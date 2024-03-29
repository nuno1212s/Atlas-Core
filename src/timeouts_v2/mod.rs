mod worker;

use std::any::Any;
use std::cmp::Ordering;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;
use getset::{CopyGetters, Getters};
use atlas_common::channel::ChannelSyncTx;
use atlas_common::node_id::NodeId;
use atlas_common::ordering::SeqNo;
use crate::timeouts_v2::worker::WorkerMessage;

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum TimeoutID {
    SeqNoBased(SeqNo),
    SessionBased {
        session: SeqNo,
        seq_no: SeqNo,
    },
}

#[derive(Hash, Ord, Eq, PartialOrd, PartialEq, Clone, Debug)]
pub struct TimeoutIdentification {
    mod_id: Arc<str>,
    timeout_id: TimeoutID,
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
    #[get]
    extra_info: Option<Box<dyn Any>>,
}

/// A timeout trait, representing the behaviour needed from a timeout request
pub trait TimeOut {
    // The identification of the timeout
    fn id(&self) -> &TimeoutIdentification;

    // How many confirmed responses do we need to extinguish the timeout?
    fn needed_acks(&self) -> usize;
}

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

impl PartialOrd for TimeoutID {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeoutID {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (TimeoutID::SeqNoBased(a), TimeoutID::SeqNoBased(b)) => a.cmp(b),
            (TimeoutID::SessionBased { session: session_1, seq_no: seq_1 },
                TimeoutID::SessionBased { session: session_2, seq_no: seq_2 }) => {
                match session_1.cmp(session_2) {
                    Ordering::Equal => seq_1.cmp(seq_2),
                    other => other,
                }
            }
            (TimeoutID::SeqNoBased(_), TimeoutID::SessionBased { .. }) => Ordering::Less,
            (TimeoutID::SessionBased { .. }, TimeoutID::SeqNoBased(_)) => Ordering::Greater,
        }
    }
}