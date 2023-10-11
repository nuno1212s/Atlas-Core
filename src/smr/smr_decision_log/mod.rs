use std::sync::Arc;
use atlas_common::crypto::hash::Digest;
use atlas_smr_application::serialize::ApplicationData;
use atlas_common::error::*;
use atlas_common::globals::ReadOnly;
use atlas_common::maybe_vec::MaybeVec;
use atlas_common::node_id::NodeId;
use atlas_common::ordering::{Orderable, SeqNo};
use atlas_communication::message::StoredMessage;
use atlas_smr_application::app::UpdateBatch;
use atlas_smr_application::ExecutorHandle;
use crate::messages::{ClientRqInfo, StoredRequestMessage};
use crate::ordering_protocol::{Decision, DecisionMetadata, OrderingProtocol, ProtocolMessage};
use crate::ordering_protocol::loggable::{LoggableOrderProtocol, OrderProtocolPersistenceHelper, PersistentOrderProtocolTypes, PProof};
use crate::ordering_protocol::networking::serialize::OrderingProtocolMessage;
use crate::persistent_log::PersistentDecisionLog;
use crate::smr::networking::serialize::DecisionLogMessage;

pub type DecLog<D, OP, POP, LS> = <LS as DecisionLogMessage<D, OP, POP>>::DecLog;
pub type DecLogMetadata<D, OP, POP, LS> = <LS as DecisionLogMessage<D, OP, POP>>::DecLogMetadata;
pub type DecLogPart<D, OP, POP, LS> = <LS as DecisionLogMessage<D, OP, POP>>::DecLogPart;

pub type ShareableConsensusMessage<D, OP> = Arc<ReadOnly<StoredMessage<<OP as OrderingProtocolMessage<D>>::ProtocolMessage>>>;
pub type ShareableMessage<P> = Arc<ReadOnly<StoredMessage<P>>>;

/// The record of the decision that has been made.
pub struct LoggedDecision<O> {
    // The sequence number
    seq: SeqNo,
    // The client requests that were contained in the decision
    contained_client_requests: Vec<ClientRqInfo>,
    decision_value: LoggedDecisionValue<O>,
}

/// Contains the requests that were in the logged decision,
/// in the case we want the replica to handle the execution
/// If we return [LoggedDecisionValue<O>::ExecutionNotNeeded],
/// we assume that the execution handling of the requests will be done
/// by the decision log
pub enum LoggedDecisionValue<O> {
    Execute(UpdateBatch<O>),
    ExecutionNotNeeded,
}

/// The information about a decision that is part of the decision log.
/// Namely, the sequence number and the messages that must be stored
/// for that sequence number proof to be completely stored.
/// This is what is used to handle the Strict persistency mode,
/// among other necessary
pub enum LoggingDecision {
    Proof(SeqNo),
    PartialDecision(SeqNo, Vec<(NodeId, Digest)>),
}

/// The abstraction for the SMR decision log
/// All SMR systems require this decision log since they function
/// on a Checkpoint based approach so it naturally requires
/// the knowledge of all decisions since the last checkpoint in order
/// to both recover replicas or integrate new replicas into the system
///
/// IMPORTANT: Refer to [LoggedDecision<O>] in order to better understand
/// how to handle logged decision execution
///
pub trait DecisionLog<D, OP, NT, PL>: Orderable where D: ApplicationData,
                                                      OP: LoggableOrderProtocol<D, NT> {
    /// The serialization type containing the serializable parts for the decision log
    type LogSerialization: DecisionLogMessage<D, OP::Serialization, OP::PersistableTypes>;

    type Config;

    /// Initialize the decision log of the
    fn initialize_decision_log(config: Self::Config, persistent_log: PL, executor_handle: ExecutorHandle<D>) -> Result<Self>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Clear the sequence number in the decision log
    fn clear_sequence_number(&mut self, seq: SeqNo) -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Clear all decisions forward of the provided one (inclusive)
    fn clear_decisions_forward(&mut self, seq: SeqNo) -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// The given sequence number was advanced in state with the given
    /// All the decisions that have been logged should be returned in the results of this
    /// function, so the replica can keep track of which sequence number we are currently in
    /// and when we need to check point the state or other related procedures.
    /// The results returned in this function should never
    fn decision_information_received(&mut self,
                                     decision_info: Decision<DecisionMetadata<D, OP::Serialization>, ProtocolMessage<D, OP::Serialization>, D::Request>)
                                     -> Result<MaybeVec<LoggedDecision<D::Request>>>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Install an entire proof into the decision log.
    /// Similarly to the [decision_information_received()] the decisions added to the decision log
    /// should be returned in the return object, following the total order of the order protocol
    fn install_proof(&mut self, proof: PProof<D, OP::Serialization, OP::PersistableTypes>)
                     -> Result<MaybeVec<LoggedDecision<D::Request>>>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Install a log received from other replicas in the system
    /// returns a list of all requests that should then be executed by the application.
    /// as well as the last execution contained in the sequence number
    fn install_log(&mut self, dec_log: DecLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>)
                   -> Result<MaybeVec<LoggedDecision<D::Request>>>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Take a snapshot of our current decision log.
    fn snapshot_log(&mut self)
                    -> Result<DecLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Get the reference to the current log
    fn current_log(&self)
                   -> Result<&DecLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// A checkpoint has been done of the state, meaning we can effectively
    /// delete the decisions up until the given sequence number.
    fn state_checkpoint(&mut self, seq: SeqNo)
                        -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Verify the sequence number sent by another replica. This doesn't pass a mutable reference since we don't want to
    /// make any changes to the state of the protocol here (or allow the implementer to do so). Instead, we want to
    /// just verify this sequence number
    fn verify_sequence_number(&self, seq_no: SeqNo, proof: &PProof<D, OP::Serialization, OP::PersistableTypes>) -> Result<bool>;

    /// Get the current sequence number of the protocol, combined with a proof of it so we can send it to other replicas
    fn sequence_number_with_proof(&self) -> Result<Option<(SeqNo, PProof<D, OP::Serialization, OP::PersistableTypes>)>>;

    /// Get the proof of decision for a given sequence number
    fn get_proof(&self, seq: SeqNo) -> Result<Option<PProof<D, OP::Serialization, OP::PersistableTypes>>>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;
}

/// Wrap a loggable message
pub fn wrap_loggable_message<D, OP, POP>(message: StoredMessage<ProtocolMessage<D, OP>>) -> ShareableConsensusMessage<D, OP> {
    Arc::new(ReadOnly::new(message))
}

pub trait PartiallyWriteableDecLog<D, OP, NT, PL>: DecisionLog<D, OP, NT, PL>
    where D: ApplicationData, OP: LoggableOrderProtocol<D, NT> {
    fn start_installing_log(&mut self) -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    fn install_log_part(&mut self, log_part: DecLogPart<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>) -> Result<()>;

    fn complete_log_install(&mut self) -> Result<()>;
}

/// Persistence helper for the decision log
pub trait DecisionLogPersistenceHelper<D, OPM, POP, LS>
    where D: ApplicationData,
          OPM: OrderingProtocolMessage<D>,
          POP: PersistentOrderProtocolTypes<D, OPM>,
          LS: DecisionLogMessage<D, OPM, POP> {

    /// Initialize the decision log
    fn init_decision_log(metadata: DecLogMetadata<D, OPM, POP, LS>, proofs: Vec<PProof<D, OPM, POP>>) -> DecLog<D, OPM, POP, LS>;

    /// Take a decision log and decompose it into parts in order to store them more quickly and easily
    /// This is also so we can support
    fn decompose_decision_log(dec_log: DecLog<D, OPM, POP, LS>) -> (DecLogMetadata<D, OPM, POP, LS>, Vec<PProof<D, OPM, POP>>);
}

impl<O> LoggedDecision<O> {
    pub fn from_decision(seq: SeqNo, client_rqs: Vec<ClientRqInfo>) -> Self {
        Self {
            seq,
            contained_client_requests: client_rqs,
            decision_value: LoggedDecisionValue::ExecutionNotNeeded,
        }
    }

    pub fn from_decision_with_execution(seq: SeqNo, client_rqs: Vec<ClientRqInfo>, update: UpdateBatch<O>) -> Self {
        Self {
            seq,
            contained_client_requests: client_rqs,
            decision_value: LoggedDecisionValue::Execute(update),
        }
    }

    pub fn into_inner(self) -> (SeqNo, Vec<ClientRqInfo>, LoggedDecisionValue<O>) {
        (self.seq, self.contained_client_requests, self.decision_value)
    }
}

impl LoggingDecision {
    pub fn init_empty(seq: SeqNo) -> Self {
        Self::PartialDecision(seq, Vec::new())
    }

    pub fn init_from_proof(seq: SeqNo) -> Self {
        Self::Proof(seq)
    }

    pub fn insert_message<D, OP>(&mut self, message: &ShareableConsensusMessage<D, OP>) {
        match self {
            LoggingDecision::PartialDecision(_, messages) => {
                messages.push((message.header().from(), message.header().digest().clone()))
            }
            LoggingDecision::Proof(_) => unreachable!(),
        }
    }
}