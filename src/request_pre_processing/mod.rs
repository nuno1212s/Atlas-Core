use std::ops::Deref;
use std::time::{Duration, Instant};
use atlas_common::channel;
use atlas_common::channel::{ChannelSyncRx, ChannelSyncTx, OneShotTx, RecvError, TryRecvError};
use atlas_common::node_id::NodeId;
use atlas_common::ordering::SeqNo;
use atlas_communication::message::{Header, StoredMessage};
use atlas_metrics::metrics::metric_duration;
use crate::messages::{ClientRqInfo, ForwardedRequestsMessage, SessionBased};
use crate::metric::{RQ_PP_CLONE_PENDING_TIME_ID, RQ_PP_COLLECT_PENDING_TIME_ID, RQ_PP_WORKER_PROPOSER_PASSING_TIME_ID};
use crate::timeouts::RqTimeout;

pub mod work_dividers;

/// The work partitioner is responsible for deciding which worker should process a given request
/// This should sign a contract to maintain all client sessions in the same worker, never changing
/// A session is defined by the client ID and the session ID.
///
pub trait WorkPartitioner<O>: Send where O: SessionBased {
    /// Get the worker that should process this request
    fn get_worker_for(rq_info: &Header, message: &O, worker_count: usize) -> usize;

    /// Get the worker that should process this request
    fn get_worker_for_processed(rq_info: &ClientRqInfo, worker_count: usize) -> usize;
}

pub type PreProcessorOutput<O> = (PreProcessorOutputMessage<O>, Instant);

#[derive(Clone)]
pub struct BatchOutput<O>(ChannelSyncRx<PreProcessorOutput<O>>);

/// Message to the request pre processor
pub enum PreProcessorMessage<O> {
    /// We have received forwarded requests from other replicas.
    ForwardedRequests(StoredMessage<ForwardedRequestsMessage<O>>),
    /// We have received requests that are already decided by the system
    StoppedRequests(Vec<StoredMessage<O>>),
    /// Analyse timeout requests. Returns only timeouts that have not yet been executed
    TimeoutsReceived(Vec<RqTimeout>, ChannelSyncTx<(Vec<RqTimeout>, Vec<RqTimeout>)>),
    /// A batch of requests that has been decided by the system
    DecidedBatch(Vec<ClientRqInfo>),
    /// Collect all pending messages from all workers.
    CollectAllPendingMessages(OneShotTx<Vec<StoredMessage<O>>>),
    /// Clone a vec of requests to be used
    CloneRequests(Vec<ClientRqInfo>, OneShotTx<Vec<StoredMessage<O>>>),
}

/// Output messages of the preprocessor
pub enum PreProcessorOutputMessage<O> {
    /// A de duped batch of ordered requests that should be proposed
    DeDupedOrderedRequests(Vec<StoredMessage<O>>),
    /// A de duped batch of unordered requests that should be proposed
    DeDupedUnorderedRequests(Vec<StoredMessage<O>>),
}

/// Request pre processor handle
#[derive(Clone)]
pub struct RequestPreProcessor<O>(ChannelSyncTx<PreProcessorMessage<O>>);

impl<O> RequestPreProcessor<O> {

    pub fn clone_pending_rqs(&self, client_rqs: Vec<ClientRqInfo>) -> Vec<StoredMessage<O>> {
        let start = Instant::now();

        let (tx, rx) = channel::new_oneshot_channel();

        self.0.send_return(PreProcessorMessage::CloneRequests(client_rqs, tx)).unwrap();

        let result = rx.recv().unwrap();

        metric_duration(RQ_PP_CLONE_PENDING_TIME_ID, start.elapsed());

        result
    }

    pub fn collect_all_pending_rqs(&self) -> Vec<StoredMessage<O>> {
        let start = Instant::now();

        let (tx, rx) = channel::new_oneshot_channel();

        self.0.send_return(PreProcessorMessage::CollectAllPendingMessages(tx)).unwrap();

        let result = rx.recv().unwrap();

        metric_duration(RQ_PP_COLLECT_PENDING_TIME_ID, start.elapsed());

        result
    }

    pub fn process_timeouts(&self, timeouts: Vec<RqTimeout>, response: ChannelSyncTx<(Vec<RqTimeout>, Vec<RqTimeout>)>) {
        self.0.send_return(PreProcessorMessage::TimeoutsReceived(timeouts, response)).unwrap();
    }
}

impl<O> From<ChannelSyncTx<PreProcessorMessage<O>>> for RequestPreProcessor<O> {
    fn from(value: ChannelSyncTx<PreProcessorMessage<O>>) -> Self {
        Self(value)
    }
}

impl<O> Deref for RequestPreProcessor<O> {
    type Target = ChannelSyncTx<PreProcessorMessage<O>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<O> From<ChannelSyncRx<PreProcessorOutput<O>>> for BatchOutput<O> {
    fn from(value: ChannelSyncRx<PreProcessorOutput<O>>) -> Self {
        Self(value)
    }
}

impl<O> BatchOutput<O> {
    pub fn recv(&self) -> Result<PreProcessorOutputMessage<O>, RecvError> {
        let (message, instant) = self.0.recv().unwrap();

        metric_duration(RQ_PP_WORKER_PROPOSER_PASSING_TIME_ID, instant.elapsed());

        Ok(message)
    }

    pub fn try_recv(&self) -> Result<PreProcessorOutputMessage<O>, TryRecvError> {
        let (message, instant) = self.0.try_recv()?;

        metric_duration(RQ_PP_WORKER_PROPOSER_PASSING_TIME_ID, instant.elapsed());

        Ok(message)
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<PreProcessorOutputMessage<O>, TryRecvError> {
        let (message, instant) = self.0.recv_timeout(timeout)?;

        metric_duration(RQ_PP_WORKER_PROPOSER_PASSING_TIME_ID, instant.elapsed());

        Ok(message)
    }
}

impl<O> Deref for PreProcessorOutputMessage<O> {
    type Target = Vec<StoredMessage<O>>;

    fn deref(&self) -> &Self::Target {
        match self {
            PreProcessorOutputMessage::DeDupedOrderedRequests(cls) => {
                cls
            }
            PreProcessorOutputMessage::DeDupedUnorderedRequests(cls) => {
                cls
            }
        }
    }
}

#[inline]
pub fn operation_key<O>(header: &Header, message: &O) -> u64
    where O: SessionBased {
    operation_key_raw(header.from(), message.session_number())
}

#[inline]
pub fn operation_key_raw(from: NodeId, session: SeqNo) -> u64 {
    // both of these values are 32-bit in width
    let client_id: u64 = from.into();
    let session_id: u64 = session.into();

    // therefore this is safe, and will not delete any bits
    client_id | (session_id << 32)
}

