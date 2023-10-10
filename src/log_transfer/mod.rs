use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use atlas_common::error::*;
use atlas_common::maybe_vec::MaybeVec;
use atlas_common::ordering::SeqNo;
use atlas_communication::message::StoredMessage;
use atlas_communication::protocol_node::ProtocolNetworkNode;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;
use atlas_smr_application::serialize::ApplicationData;

use crate::log_transfer::networking::serialize::LogTransferMessage;
use crate::ordering_protocol::loggable::{LoggableOrderProtocol, PersistentOrderProtocolTypes};
use crate::ordering_protocol::networking::serialize::{NetworkView, OrderingProtocolMessage};
use crate::ordering_protocol::{OrderingProtocol, PermissionedOrderingProtocol};
use crate::persistent_log::{PersistentDecisionLog};
use crate::smr::smr_decision_log::{DecisionLog, LoggedDecision};
use crate::timeouts::{RqTimeout, Timeouts};

pub mod networking;

pub type LogTM<D, OP, M: LogTransferMessage<D, OP>> = <M as LogTransferMessage<D, OP>>::LogTransferMessage;

/// The result of processing a message in the log transfer protocol
pub enum LTResult<D: ApplicationData> {
    RunLTP,
    NotNeeded,
    Running,
    InstallSeq(SeqNo),
    /// The log transfer protocol has finished and the ordering protocol should now
    /// be proceeded. The requests contained are requests that must be executed by the application
    /// in order to reach the state that corresponds to the decision log
    /// FirstSeq and LastSeq of the installed log downloaded from other replicas and the requests that should be executed
    LTPFinished(SeqNo, SeqNo, MaybeVec<LoggedDecision<D::Request>>),
}

pub enum LTTimeoutResult {
    RunLTP,
    NotNeeded,
}

pub trait LogTransferProtocol<D, OP, DL, NT, PL> where D: ApplicationData + 'static,
                                                       OP: LoggableOrderProtocol<D, NT>,
                                                       DL: DecisionLog<D, OP, NT, PL> + 'static {
    /// The type which implements StateTransferMessage, to be implemented by the developer
    type Serialization: LogTransferMessage<D, OP::Serialization> + 'static;

    /// The configuration type the protocol wants to accept
    type Config;

    /// Initialize the log transferring protocol
    fn initialize(config: Self::Config, timeout: Timeouts, node: Arc<NT>, log: PL) -> Result<Self>
        where Self: Sized;

    /// Request the latest logs from the rest of the replicas
    fn request_latest_log<V>(&mut self, decision_log: &mut DL, view: V) -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, DL::LogSerialization>,
              V: NetworkView;

    /// Handle a state transfer protocol message that was received while executing the ordering protocol
    fn handle_off_ctx_message<V>(&mut self, decision_log: &mut DL,
                                 view: V,
                                 message: StoredMessage<LogTM<D, OP, Self::Serialization>>)
                                 -> Result<()>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, DL::LogSerialization>,
              V: NetworkView;

    /// Process a log transfer protocol message, received from other replicas
    ///
    fn process_message<V>(&mut self, decision_log: &mut DL, view: V,
                          message: StoredMessage<LogTM<D, OP, Self::Serialization>>) -> Result<LTResult<D>>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, DL::LogSerialization>,
              V: NetworkView;

    /// Handle a timeout received from the timeout layer
    fn handle_timeout<V>(&mut self, view: V, timeout: Vec<RqTimeout>) -> Result<LTTimeoutResult>
        where PL: PersistentDecisionLog<D, OP::Serialization, OP::PersistableTypes, DL::LogSerialization>,
              V: NetworkView;
}

impl<D: ApplicationData> Debug for LTResult<D> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LTResult::RunLTP => {
                write!(f, "RunLTP")
            }
            LTResult::NotNeeded => {
                write!(f, "NotNeeded")
            }
            LTResult::Running => {
                write!(f, "Running")
            }
            LTResult::LTPFinished(first, last, _) => {
                write!(f, "LTPFinished({:?}, {:?})", first, last)
            }
        }
    }
}