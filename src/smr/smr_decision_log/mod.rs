use atlas_smr_application::serialize::ApplicationData;
use atlas_common::error::*;
use atlas_common::ordering::SeqNo;
use crate::ordering_protocol::{OrderingProtocol, SerProof};
use crate::persistent_log::PersistentDecisionLog;
use crate::smr::networking::serialize::DecisionLogMessage;

pub type DecLog<D, OP, LS> = <LS as DecisionLogMessage<D, OP>>::DecLog;

pub trait DecisionLog<D, OP, NT, PL> where D: ApplicationData, OP: OrderingProtocol<D, NT, PL> {
    /// The serialization type containing the serializable parts for the decision log
    type LogSerialization: DecisionLogMessage<D, OP::Serialization>;

    type Config;

    /// Initialize the decision log of the
    fn initialize_decision_log(config: Self::Config, persistent_log: PL) -> Option<DecLog<D, OP, Self::LogSerialization>>
        where PL: PersistentDecisionLog<D, OP, Self::LogSerialization>;

    /// Sequence number decided by the ordering protocol
    fn sequence_number_decided_with_full_proof(&mut self, proof: SerProof<D, OP::Serialization>) -> Result<()>
        where PL: PersistentDecisionLog<D, OP, Self::LogSerialization>;

    /// Install a log received from other replicas in the system
    /// list of all requests that should then be executed by the application.
    fn install_log(&mut self, order_protocol: &OP, dec_log: DecLog<D, OP::Serialization, Self::LogSerialization>) -> Result<Vec<D::Request>>
        where PL: PersistentDecisionLog<D, OP, Self::LogSerialization>;

    /// Take a snapshot of our current decision log.
    fn snapshot_log(&mut self) -> Result<DecLog<D, OP::Serialization, Self::LogSerialization>>
        where PL: PersistentDecisionLog<D, OP, Self::LogSerialization>;

    /// Get the reference to the current log
    fn current_log(&self) -> Result<&DecLog<D, OP::Serialization, Self::LogSerialization>>
        where PL: PersistentDecisionLog<D, OP, Self::LogSerialization>;

    /// A checkpoint has been done of the state, meaning we can effectively
    /// delete the decisions.
    fn state_checkpoint(&self, seq: SeqNo) -> Result<()>
        where PL: PersistentDecisionLog<D, OP, Self::LogSerialization>;

    /// Get the proof of decision for a given sequence number
    fn get_proof(&self, seq: SeqNo) -> Result<Option<SerProof<D, OP::Serialization>>>
        where PL: PersistentDecisionLog<D, OP, Self::LogSerialization>;
}

pub type DecLogPart<D, OP, LS> = <LS as DecisionLogMessage<D, OP>>::DecLogPart;

pub trait PartiallyWriteableDecLog<D, OP, NT, PL>: DecisionLog<D, OP, NT, PL>
    where D: ApplicationData, OP: OrderingProtocol<D, NT, PL> {
    fn start_installing_log(&mut self) -> Result<()>
        where PL: PersistentDecisionLog<D, OP, Self::LogSerialization>;

    fn install_log_part(&mut self, log_part: DecLogPart<D, OP, Self::LogSerialization>)

    fn complete_log_install(&mut self) -> Result<()>;
}