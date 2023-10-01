use std::sync::Arc;
use atlas_smr_application::serialize::ApplicationData;
use atlas_common::error::*;
use atlas_common::globals::ReadOnly;
use atlas_common::ordering::{Orderable, SeqNo};
use atlas_communication::message::StoredMessage;
use crate::ordering_protocol::{DecisionMetadata, OrderingProtocol};
use crate::ordering_protocol::loggable::{LMessage, LoggableOrderProtocol, PersistentOrderProtocolTypes, PProof};
use crate::persistent_log::PersistentDecisionLog;
use crate::smr::networking::serialize::DecisionLogMessage;

pub type DecLog<D, OP, POP, LS> = <LS as DecisionLogMessage<D, OP, POP>>::DecLog;

pub type StoredConsensusMessage<D, OP, POP> = Arc<ReadOnly<StoredMessage<<POP as PersistentOrderProtocolTypes<D, OP>>::LoggableMessage>>>;

pub trait DecisionLog<D, OP, NT, PL>: Orderable where D: ApplicationData,
                                                      OP: LoggableOrderProtocol<D, NT, PL> {
    /// The serialization type containing the serializable parts for the decision log
    type LogSerialization: DecisionLogMessage<D, OP::Serialization, OP::PersistableTypes>;

    type Config;

    /// Initialize the decision log of the
    fn initialize_decision_log(config: Self::Config, persistent_log: PL) -> Result<Self>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Clear the sequence number in the decision log
    fn clear_sequence_number(&mut self, seq: SeqNo) -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Clear all decisions forward of the provided one (inclusive)
    fn clear_decisions_forward(&mut self, seq: SeqNo) -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// The given sequence number was advanced in state with the given
    fn sequence_number_advanced(&mut self, seq: SeqNo, message: StoredConsensusMessage<D, OP::Serialization, OP::PersistableTypes>) -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    fn sequence_number_decided(&mut self, seq: SeqNo, metadata: DecisionMetadata<D, OP::Serialization>) -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Sequence number decided by the ordering protocol
    fn sequence_number_decided_with_full_proof(&mut self, proof: PProof<D, OP::Serialization, OP::PersistableTypes>) -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Install a log received from other replicas in the system
    /// list of all requests that should then be executed by the application.
    fn install_log(&mut self, order_protocol: &OP, dec_log: DecLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>) -> Result<Vec<D::Request>>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Take a snapshot of our current decision log.
    fn snapshot_log(&mut self) -> Result<DecLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// Get the reference to the current log
    fn current_log(&self) -> Result<&DecLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    /// A checkpoint has been done of the state, meaning we can effectively
    /// delete the decisions up until the given sequence number.
    fn state_checkpoint(&self, seq: SeqNo) -> Result<()>
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

pub type DecLogPart<D, OP, POP, LS> = <LS as DecisionLogMessage<D, OP, POP>>::DecLogPart;

pub trait PartiallyWriteableDecLog<D, OP, NT, PL>: DecisionLog<D, OP, NT, PL>
    where D: ApplicationData, OP: LoggableOrderProtocol<D, NT, PL> {
    fn start_installing_log(&mut self) -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>;

    fn install_log_part(&mut self, log_part: DecLogPart<D, OP::Serialization, OP::PersistableTypes, Self::LogSerialization>) -> Result<()>;

    fn complete_log_install(&mut self) -> Result<()>;
}