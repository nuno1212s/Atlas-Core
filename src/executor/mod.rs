use std::time::Instant;
use atlas_common::maybe_vec::MaybeVec;
use atlas_common::node_id::NodeId;
use atlas_common::ordering::SeqNo;
use crate::ordering_protocol::BatchedDecision;

pub enum UpdateInfo {
    SessionBased {
        from: NodeId,
        session_number: SeqNo,
        sequence_number: SeqNo,
    }
}

pub struct Update<O> {
    info: UpdateInfo,
    operation: O,
}

pub struct UpdateReply<R> {
    info: UpdateInfo,
    reply: R
}

/// Trait that defines the necessary behaviour of a execution handle.
///
/// Execution handles mean the channel through which the protocol should send requests to the executor (whichever executor
/// that may be).
pub trait DecisionExecutorHandle<RQ>: Send + Clone + 'static {
    /// Queues a vec of decisions for execution.
    fn catch_up_to_quorum(&self, requests: MaybeVec<BatchedDecision<RQ>>) -> atlas_common::error::Result<()>;

    /// Queues a batch of requests `batch` for execution.
    fn queue_update(&self, batch: BatchedDecision<RQ>) -> atlas_common::error::Result<()>;

    /// Queues a batch of unordered requests for execution
    fn queue_update_unordered(&self, requests: BatchedDecision<RQ>) -> atlas_common::error::Result<()>;
}

