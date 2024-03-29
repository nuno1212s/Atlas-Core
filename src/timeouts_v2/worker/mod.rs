use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::{Duration, SystemTime, SystemTimeError};
use getset::{CopyGetters, Getters};

use thiserror::Error;

use atlas_common::channel::ChannelSyncRx;
use atlas_common::collections::HashMap;
use atlas_common::node_id::NodeId;

use crate::timeouts_v2::{TimeOut, TimeoutAck, TimeoutIdentification, TimeoutRequest};

pub(super) enum WorkerMessage {
    TimeoutRequest(TimeoutRequest),
    TimeoutRequests(Vec<TimeoutRequest>),
    TimeoutAck(TimeoutAck),
    TimeoutAcks(Vec<TimeoutAck>),
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

struct TimeoutWorker {
    our_node_id: NodeId,
    worker_id: u32,
    default_timeout_duration: Duration,

    work_rx_channel: ChannelSyncRx<WorkerMessage>,

    // Requests which can receive multiple timeouts are tracked in
    // this map
    watched_requests: HashMap<TimeoutIdentification, Rc<RefCell<RegisteredTimeout>>>,

    // The timeouts we are currently waiting for
    pending_timeout_heap: BTreeMap<u64, Vec<Rc<RefCell<RegisteredTimeout>>>>,
}

impl TimeoutWorker {
    fn process_message(&mut self, message: WorkerMessage) -> Result<(), ProcessTimeoutMessageError> {
        match message {
            WorkerMessage::TimeoutRequest(request) => {
                self.handle_timeout_request(request)?;
            }
            WorkerMessage::TimeoutRequests(requests) => {
                requests.into_iter().try_for_each(|rq| {
                    self.handle_timeout_request(rq)
                })?;
            }
            WorkerMessage::TimeoutAck(ack) => {
                self.handle_timeout_ack(ack)?;
            }
            WorkerMessage::TimeoutAcks(acks) => {
                acks.into_iter().try_for_each(|ack| {
                    self.handle_timeout_ack(ack)
                })?;
            }
        }

        Ok(())
    }

    fn handle_timeout_request(&mut self, timeout_request: TimeoutRequest) -> Result<(), ProcessTimeoutError> {
        let registered_rq = Rc::new(RefCell::new(RegisteredTimeout::new(timeout_request)?));

        self.watched_requests.entry(registered_rq.borrow().request().id().clone())
            .or_insert(registered_rq.clone());

        let timeout_time = registered_rq.borrow().timeout_time();

        self.pending_timeout_heap.entry(timeout_time)
            .or_default()
            .push(registered_rq);

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

    fn remove_time_out(&mut self, timeout_id: &TimeoutIdentification) {
        if let Some(watched) = self.watched_requests.remove(timeout_id) {
            let timeout_time = watched.borrow().timeout_time();

            if let Some(vec) = self.pending_timeout_heap.get_mut(&timeout_time) {
                vec.retain(|rq| !Rc::ptr_eq(rq, &watched))
            }
        }
    }
    
    fn process_timeouts(&mut self) -> Result<(), TimeoutError> {
        
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_millis() as u64;
        
        while let Some((timeout, _))  =self.pending_timeout_heap.first_key_value() {
            if *timeout > now {
                break;
            }
            
            let (_, requests) = self.pending_timeout_heap.pop_first().unwrap();
            
            requests.iter()
                .map(|rq| rq.borrow_mut())
                .for_each(|mut rq| rq.timed_out());
            
            
        }
        
        Ok(())
    }
    
}

impl TimeoutPhase {
    fn next_timeout(&self) -> Self {
        match self {
            Self::NeverTimedOut => Self::TimedOut(1, SystemTime::now()),
            Self::TimedOut(n, _) => {
                Self::TimedOut(*n + 1, SystemTime::now())
            }
        }
    }
}

impl RegisteredTimeout {
    fn new(request: TimeoutRequest) -> Result<Self, SystemTimeError> {
        let end_timeout_time = SystemTime::now() + request.duration;

        let timeout_int_time = end_timeout_time.duration_since(SystemTime::UNIX_EPOCH)?.as_millis() as u64;

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
    
    fn timed_out(&mut self)  {
        self.timeout_phase = self.timeout_phase.next_timeout();
    }
}

#[derive(Error, Debug)]
pub enum ProcessTimeoutMessageError {
    #[error("Ack process failed: {0}")]
    AckProcessFailed(#[from] AcceptAckError),
    #[error("Failed to process timeout: {0}")]
    TimeoutProcessFailed(#[from] ProcessTimeoutError),
}

#[derive(Error, Debug)]
pub enum AcceptAckError {
    #[error("Node already ACKed {0:?}")]
    NodeAlreadyAcked(NodeId)
}

#[derive(Error, Debug)]
pub enum ProcessTimeoutError {
    #[error("Failed to calculate system time? {0}")]
    SystemTimeError(#[from] SystemTimeError)
}

#[derive(Error, Debug)]
pub enum TimeoutError {
    #[error("Failed to calculate system time? {0}")]
    SystemTimeError(#[from] SystemTimeError),
    
}