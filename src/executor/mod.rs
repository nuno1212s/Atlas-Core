use std::time::Instant;
use atlas_common::maybe_vec::MaybeVec;
use crate::ordering_protocol::BatchedDecision;


pub trait DecisionExecutorHandle<RQ> {

    fn catch_up_to_quorum(&self, requests: MaybeVec<BatchedDecision<RQ>>) -> atlas_common::error::Result<()>;

    /// Queues a batch of requests `batch` for execution.
    fn queue_update(&self, batch: BatchedDecision<RQ>) -> atlas_common::error::Result<()>;

    /// Queues a batch of unordered requests for execution
    fn queue_update_unordered(&self, requests: BatchedDecision<RQ>) -> atlas_common::error::Result<()> ;

}