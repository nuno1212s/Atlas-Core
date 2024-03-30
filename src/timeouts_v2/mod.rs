use dyn_clone::DynClone;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use getset::{CopyGetters, Getters};
use itertools::Itertools;

use crate::request_pre_processing::operation_key_raw;
use atlas_common::channel::{new_bounded_sync, ChannelSyncTx};
use atlas_common::node_id::NodeId;
use atlas_common::ordering::SeqNo;

use crate::timeouts_v2::worker::WorkerMessage;

mod tests;
mod worker;

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum TimeoutID {
    SeqNoBased(SeqNo),
    SessionBased {
        session: SeqNo,
        seq_no: SeqNo,
        from: NodeId,
    },
}

#[derive(Hash, Ord, Eq, PartialOrd, PartialEq, Clone, Debug)]
pub struct TimeoutIdentification {
    mod_id: Arc<str>,
    timeout_id: TimeoutID,
}

/// A time-out that has occurred
#[derive(Debug, CopyGetters, Getters)]
pub struct Timeout {
    #[get = "pub"]
    id: TimeoutIdentification,
    #[get_copy = "pub"]
    timeout_count: usize,
    #[get_copy = "pub"]
    timeout_time: SystemTime,
    #[get = "pub"]
    extra_info: Option<Box<dyn TimeOutable>>,
}

#[derive(CopyGetters, Getters)]
struct TimeoutRequest {
    #[get]
    id: TimeoutIdentification,
    #[get]
    duration: Duration,
    #[get_copy]
    needed_acks: usize,
    #[get_copy]
    is_cumulative: bool,
    extra_info: Option<Box<dyn TimeOutable>>,
}

/// A timeout trait, representing the behaviour needed from a timeout request
pub trait TimeOutable: DynClone + Debug + Send {}

#[derive(Getters, CopyGetters)]
struct TimeoutAck {
    #[get = "pub"]
    id: TimeoutIdentification,
    #[get_copy]
    from: NodeId,
}

#[derive(Clone)]
pub struct TimeoutsHandle {
    worker_handles: Vec<ChannelSyncTx<WorkerMessage>>,
}

pub fn initialize_timeouts<WR>(
    our_id: NodeId,
    num_workers: usize,
    channel_size: usize,
    timeout_output: WR,
) -> TimeoutsHandle
where
    WR: TimeoutWorkerResponder + 'static,
{
    let mut handles = Vec::with_capacity(num_workers);

    for worker in 0..num_workers {
        let (tx, rx) = new_bounded_sync(
            channel_size,
            Some(format!("TimeoutWorker Thread {}", worker)),
        );

        worker::initialize_worker_thread(our_id, worker, rx, timeout_output.clone());

        handles.push(tx)
    }

    TimeoutsHandle {
        worker_handles: handles,
    }
}

impl TimeoutsHandle {
    fn worker_for_timeout(
        &self,
        timeout_id: &TimeoutIdentification,
    ) -> &ChannelSyncTx<WorkerMessage> {
        &self.worker_handles[self.worker_id_for_timeout(timeout_id)]
    }

    fn worker_id_for_timeout(&self, timeout_id: &TimeoutIdentification) -> usize {
        match timeout_id.timeout_id {
            TimeoutID::SeqNoBased(_) => 0,
            TimeoutID::SessionBased { session, from, .. } => {
                operation_key_raw(from, session) as usize % self.worker_handles.len()
            }
        }
    }

    pub fn request_timeout(
        &self,
        timeout_id: TimeoutIdentification,
        extra_info: Option<Box<dyn TimeOutable>>,
        duration: Duration,
        needed_acks: usize,
        cumulative: bool,
    ) -> atlas_common::error::Result<()> {
        self.worker_for_timeout(&timeout_id)
            .send(WorkerMessage::Request(TimeoutRequest {
                id: timeout_id,
                duration,
                needed_acks,
                is_cumulative: cumulative,
                extra_info,
            }))
    }

    pub fn request_timeouts(
        &self,
        timeout_id: Vec<(TimeoutIdentification, Option<Box<dyn TimeOutable>>)>,
        duration: Duration,
        needed_acks: usize,
        cumulative: bool,
    ) -> atlas_common::error::Result<()> {
        timeout_id
            .into_iter()
            .map(|(id, extra_info)| TimeoutRequest {
                id,
                duration,
                needed_acks,
                is_cumulative: cumulative,
                extra_info,
            })
            .group_by(|rq| self.worker_id_for_timeout(rq.id()))
            .into_iter()
            .try_for_each(|(worker_id, group)| {
                self.worker_handles[worker_id].send(WorkerMessage::Requests(group.collect()))
            })
    }

    pub fn ack_received(
        &self,
        timeout_id: TimeoutIdentification,
        from: NodeId,
    ) -> atlas_common::error::Result<()> {
        self.worker_for_timeout(&timeout_id)
            .send(WorkerMessage::Ack(TimeoutAck {
                id: timeout_id,
                from,
            }))
    }

    pub fn acks_received(
        &self,
        acks: Vec<(TimeoutIdentification, NodeId)>,
    ) -> atlas_common::error::Result<()> {
        acks.into_iter()
            .map(|(id, from)| TimeoutAck { id, from })
            .group_by(|ack| self.worker_id_for_timeout(ack.id()))
            .into_iter()
            .try_for_each(|(worker_id, group)| {
                self.worker_handles[worker_id].send(WorkerMessage::Acks(group.collect()))
            })
    }
}

impl PartialOrd for TimeoutID {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeoutID {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (TimeoutID::SeqNoBased(a), TimeoutID::SeqNoBased(b)) => a.cmp(b),
            (
                TimeoutID::SessionBased {
                    session: session_1,
                    seq_no: seq_1,
                    from: from_1,
                },
                TimeoutID::SessionBased {
                    session: session_2,
                    seq_no: seq_2,
                    from: from_2,
                },
            ) => match from_1.cmp(from_2) {
                Ordering::Equal => match session_1.cmp(session_2) {
                    Ordering::Equal => seq_1.cmp(seq_2),
                    other => other,
                },
                other => other,
            },
            (TimeoutID::SeqNoBased(_), TimeoutID::SessionBased { .. }) => Ordering::Less,
            (TimeoutID::SessionBased { .. }, TimeoutID::SeqNoBased(_)) => Ordering::Greater,
        }
    }
}

/// The trait detailing the worker behaviour we require
pub trait TimeoutWorkerResponder: Send + Clone {
    /// Report a timeout to the rest of the system
    fn report_timeouts(&self, timeouts: Vec<Timeout>) -> atlas_common::error::Result<()>;
}

impl TimeoutRequest {
    fn extra_info(&self) -> Option<&dyn TimeOutable> {
        self.extra_info.as_deref()
    }
}
