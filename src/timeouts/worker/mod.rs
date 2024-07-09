#![allow(dead_code)]

use getset::{CopyGetters, Getters};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Add;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, SystemTime, SystemTimeError};

use thiserror::Error;
use tracing::error;
use tracing::instrument;
use tracing::Level;

use crate::request_pre_processing::{operation_key, operation_key_raw};
use atlas_common::channel::{ChannelSyncRx, TryRecvError};
use atlas_common::collections::HashMap;
use atlas_common::node_id::NodeId;

use crate::timeouts::{
    Timeout, TimeoutAck, TimeoutIdentification, TimeoutRequest, TimeoutWorkerResponder,
};

#[derive(Debug)]
pub(super) enum WorkerMessage {
    Request(TimeoutRequest),
    Requests(Vec<TimeoutRequest>),
    Ack(TimeoutAck),
    Acks(Vec<TimeoutAck>),
    Cancel(TimeoutIdentification),
    CancelMultiple(Vec<TimeoutIdentification>),
    CancelAll(Arc<str>),
    ResetAll(Arc<str>),
}

enum TimeoutPhase {
    // This request has never timed out
    NeverTimedOut,
    TimedOut(usize, SystemTime),
}

#[derive(Getters, CopyGetters)]
struct RegisteredTimeout {
    #[get_copy]
    timeout_time: u64,
    #[get]
    timeout_phase: TimeoutPhase,
    #[get]
    time_made: SystemTime,
    #[get]
    acks_received: BTreeSet<NodeId>,
    #[get]
    request: TimeoutRequest,
}

struct TimeoutWorker<WR> {
    our_node_id: NodeId,
    worker_id: u32,
    default_timeout_duration: Duration,

    work_rx_channel: ChannelSyncRx<WorkerMessage>,

    // Requests which can receive multiple timeouts are tracked in
    // this map
    watched_requests: HashMap<TimeoutIdentification, Rc<RefCell<RegisteredTimeout>>>,

    // The timeouts we are currently waiting for
    pending_timeout_heap: BTreeMap<u64, Vec<Rc<RefCell<RegisteredTimeout>>>>,
    // The notifier to send timeouts to
    timeout_notifier: WR,
}

pub(super) fn initialize_worker_thread<WR>(
    our_id: NodeId,
    worker_id: usize,
    work_receiver: ChannelSyncRx<WorkerMessage>,
    notifier: WR,
) where
    WR: TimeoutWorkerResponder + 'static,
{
    std::thread::Builder::new()
        .name(format!("TimeoutWorker-{}", worker_id))
        .spawn(move || {
            let mut worker = TimeoutWorker {
                our_node_id: our_id,
                worker_id: worker_id as u32,
                default_timeout_duration: Default::default(),
                work_rx_channel: work_receiver,
                watched_requests: Default::default(),
                pending_timeout_heap: Default::default(),
                timeout_notifier: notifier,
            };

            loop {
                if let Err(err) = worker.run() {
                    error!("Timeout worker error: {:?}", err);
                }
            }
        })
        .expect("Failed to spawn timeout worker thread");
}

impl<WR> TimeoutWorker<WR>
where
    WR: TimeoutWorkerResponder,
{
    fn run(&mut self) -> Result<(), TimeoutError> {
        let duration = Duration::from_millis(1000);

        loop {
            match self.work_rx_channel.recv_timeout(duration) {
                Ok(message) => {
                    self.process_message(message)?;
                }
                Err(e) => {
                    if let TryRecvError::Timeout = e {
                    } else {
                        error!("Error receiving message: {:?}", e);
                    }
                }
            }

            self.process_timeouts()?;
        }
    }

    #[instrument(skip(self), level = Level::DEBUG)]
    fn process_message(
        &mut self,
        message: WorkerMessage,
    ) -> Result<(), ProcessTimeoutMessageError> {
        match message {
            WorkerMessage::Request(request) => {
                self.handle_timeout_request(request)?;
            }
            WorkerMessage::Requests(requests) => {
                requests
                    .into_iter()
                    .try_for_each(|rq| self.handle_timeout_request(rq))?;
            }
            WorkerMessage::Ack(ack) => {
                self.handle_timeout_ack(ack)?;
            }
            WorkerMessage::Acks(acks) => {
                acks.into_iter()
                    .try_for_each(|ack| self.handle_timeout_ack(ack))?;
            }
            WorkerMessage::Cancel(timeout_id) => {
                self.remove_time_out(&timeout_id);
            }
            WorkerMessage::CancelMultiple(timeout_ids) => {
                timeout_ids
                    .into_iter()
                    .for_each(|id| self.remove_time_out(&id));
            }
            WorkerMessage::CancelAll(module_name) => {
                self.cancel_all_rqs_for_mod(module_name)?;
            }
            WorkerMessage::ResetAll(mod_name) => {
                self.reset_all_rqs_for_mod(mod_name)?;
            }
        }

        Ok(())
    }

    fn handle_timeout_request(
        &mut self,
        timeout_request: TimeoutRequest,
    ) -> Result<(), ProcessTimeoutError> {
        let registered_rq = Rc::new(RefCell::new(RegisteredTimeout::new(timeout_request)?));

        self.watched_requests
            .entry(registered_rq.borrow().request().id().clone())
            .or_insert(registered_rq.clone());

        let timeout_time = registered_rq.borrow().timeout_time();

        self.pending_timeout_heap
            .entry(timeout_time)
            .or_default()
            .push(registered_rq);

        Ok(())
    }

    fn re_register_timeout(
        &mut self,
        timeout: Rc<RefCell<RegisteredTimeout>>,
    ) -> Result<(), ProcessTimeoutError> {
        let timeout_time = SystemTime::now()
            .add(*timeout.borrow().request().duration())
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as u64;

        timeout.borrow_mut().timeout_time = timeout_time;

        self.pending_timeout_heap
            .entry(timeout_time)
            .or_default()
            .push(timeout);

        Ok(())
    }

    fn handle_timeout_ack(&mut self, timeout_ack: TimeoutAck) -> Result<(), AcceptAckError> {
        let should_remove = if let Some(watched) = self.watched_requests.get(timeout_ack.id()) {
            let mut mut_guard = watched.borrow_mut();

            mut_guard.register_ack(timeout_ack.from())?;

            mut_guard.acks_received().len() >= mut_guard.request().needed_acks()
        } else {
            false
        };

        if should_remove {
            self.remove_time_out(timeout_ack.id());
        }

        Ok(())
    }

    #[instrument(skip(self), level = Level::DEBUG)]
    fn remove_time_out(&mut self, timeout_id: &TimeoutIdentification) {
        if let Some(watched) = self.watched_requests.remove(timeout_id) {
            let timeout_time = watched.borrow().timeout_time();

            if let Some(vec) = self.pending_timeout_heap.get_mut(&timeout_time) {
                let removed_position = vec
                    .iter()
                    .position(|rq| Rc::ptr_eq(rq, &watched))
                    .map(|pos| vec.swap_remove(pos));

                if removed_position.is_none() {
                    error!("Failed to remove timeout from heap {:?}", timeout_id);
                };
            } else {
                error!(
                    "Removed timeout that was no longer in the pending timeout heap? {:?}",
                    timeout_id
                );
            }
        } else {
            error!(
                "Provided timeout that is not currently being tracked {:?}",
                timeout_id
            );
        }
    }

    #[instrument(skip(self), level = Level::DEBUG)]
    fn process_timeouts(&mut self) -> Result<(), TimeoutError> {
        let current_sys_time = SystemTime::now();

        let now = current_sys_time
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as u64;

        let mut timeouts = Vec::new();

        while let Some((timeout, _)) = self.pending_timeout_heap.first_key_value() {
            if *timeout > now {
                break;
            }

            let (_, requests) = self.pending_timeout_heap.pop_first().unwrap();

            requests.iter().for_each(|rq| rq.borrow_mut().timed_out());

            let (cumulative_stream, non_cumulative_stream): (Vec<_>, Vec<_>) = requests
                .iter()
                .partition(|rq| rq.borrow().request().is_cumulative());

            // Re register the cumulative timeouts for the next timeout
            cumulative_stream
                .into_iter()
                .try_for_each(|rq| self.re_register_timeout(rq.clone()))?;

            // Remove the non-cumulative ones from the watched list
            non_cumulative_stream.into_iter().for_each(|rq| {
                self.watched_requests.remove(rq.borrow().request().id());
            });

            let mut partial_timeouts = requests
                .into_iter()
                .map(|rq| {
                    let rq_borrow = rq.borrow();

                    let extra_info = rq_borrow
                        .request()
                        .extra_info
                        .as_ref()
                        .map(|ex| dyn_clone::clone_box(&**ex));

                    Timeout {
                        id: rq_borrow.request().id().clone(),
                        timeout_count: rq_borrow.timeout_phase().timeout_count(),
                        timeout_time: current_sys_time,
                        extra_info,
                    }
                })
                .collect::<Vec<_>>();

            timeouts.append(&mut partial_timeouts);
        }

        if !timeouts.is_empty() {
            self.timeout_notifier.report_timeouts(timeouts)?;
        }

        Ok(())
    }

    fn cancel_all_rqs_for_mod(
        &mut self,
        mod_name: Arc<str>,
    ) -> Result<(), ClearAllOcurrencesError> {
        self.watched_requests
            .retain(|k, _v| !Arc::ptr_eq(k.mod_id(), &mod_name));

        self.pending_timeout_heap.iter_mut().for_each(|(_, v)| {
            v.retain(|rq| !Arc::ptr_eq(rq.borrow().request().id().mod_id(), &mod_name))
        });

        Ok(())
    }

    fn reset_all_rqs_for_mod(&mut self, mod_name: Arc<str>) -> Result<(), ResetTimeoutError> {
        let mut removed_timeouts: Vec<_> = self
            .pending_timeout_heap
            .values_mut()
            .flat_map(|timeouts| {
                timeouts
                    .extract_if(|rq| Arc::ptr_eq(rq.borrow().request().id().mod_id(), &mod_name))
            })
            .collect();

        removed_timeouts.iter_mut().for_each(|rq| {
            let mut timeout_ref = rq.borrow_mut();

            timeout_ref.timeout_phase = TimeoutPhase::NeverTimedOut;
            timeout_ref.acks_received.clear();
        });

        let timeout_time = SystemTime::now()
            .add(self.default_timeout_duration)
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as u64;

        self.pending_timeout_heap
            .entry(timeout_time)
            .or_default()
            .append(&mut removed_timeouts);

        Ok(())
    }
}

impl TimeoutPhase {
    fn next_timeout(&self) -> Self {
        match self {
            Self::NeverTimedOut => Self::TimedOut(1, SystemTime::now()),
            Self::TimedOut(n, _) => Self::TimedOut(*n + 1, SystemTime::now()),
        }
    }

    fn timeout_count(&self) -> usize {
        match self {
            Self::NeverTimedOut => 0,
            Self::TimedOut(n, _) => *n,
        }
    }

    fn last_timeout_time(&self) -> Option<SystemTime> {
        match self {
            Self::NeverTimedOut => None,
            Self::TimedOut(_, t) => Some(*t),
        }
    }
}

impl RegisteredTimeout {
    fn new(request: TimeoutRequest) -> Result<Self, SystemTimeError> {
        let end_timeout_time = SystemTime::now() + request.duration;

        let timeout_int_time = end_timeout_time
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as u64;

        Ok(Self {
            timeout_time: timeout_int_time,
            timeout_phase: TimeoutPhase::NeverTimedOut,
            time_made: SystemTime::now(),
            acks_received: BTreeSet::new(),
            request,
        })
    }

    fn register_ack(&mut self, from: NodeId) -> Result<(), AcceptAckError> {
        if !self.acks_received.insert(from) {
            return Err(AcceptAckError::NodeAlreadyAcked(from));
        }

        Ok(())
    }

    fn timed_out(&mut self) {
        self.timeout_phase = self.timeout_phase.next_timeout();
    }
}

#[derive(Error, Debug)]
pub enum ProcessTimeoutMessageError {
    #[error("Ack process failed: {0}")]
    AckProcessFailed(#[from] AcceptAckError),
    #[error("Failed to process timeout: {0}")]
    TimeoutProcessFailed(#[from] ProcessTimeoutError),
    #[error("Failed to clear all occurrences {0}")]
    ClearingAllOccurrences(#[from] ClearAllOcurrencesError),
    #[error("Failed to reset timeouts: {0}")]
    ResetTimeouts(#[from] ResetTimeoutError),
}

#[derive(Error, Debug)]
pub enum ClearAllOcurrencesError {}

#[derive(Error, Debug)]
pub enum ResetTimeoutError {
    #[error("Failed to calculate system time? {0}")]
    SystemTime(#[from] SystemTimeError),
}

#[derive(Error, Debug)]
pub enum AcceptAckError {
    #[error("Node already ACKed {0:?}")]
    NodeAlreadyAcked(NodeId),
}

#[derive(Error, Debug)]
pub enum ProcessTimeoutError {
    #[error("Failed to calculate system time? {0}")]
    SystemTimeError(#[from] SystemTimeError),
}

#[derive(Error, Debug)]
pub enum TimeoutError {
    #[error("Failed to calculate system time? {0}")]
    SystemTime(#[from] SystemTimeError),
    #[error("Re-Register cumulative timeout error {0}")]
    RegisterTimeout(#[from] ProcessTimeoutError),
    #[error("Failed to process message timeouts {0}")]
    MessageProcess(#[from] ProcessTimeoutMessageError),
    #[error("Failed to notify of timeouts {0}")]
    Notifier(#[from] anyhow::Error),
}
