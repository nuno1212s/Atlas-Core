pub mod signature_ver;
pub mod serialize;

use std::collections::BTreeMap;

use atlas_common::crypto::hash::Digest;
use atlas_common::error::*;
use atlas_common::node_id::NodeId;
use atlas_communication::FullNetworkNode;
use atlas_communication::message::{SerializedMessage, StoredMessage, StoredSerializedProtocolMessage};
use atlas_communication::protocol_node::ProtocolNetworkNode;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;
use atlas_communication::serialize::Serializable;
use atlas_smr_application::serialize::ApplicationData;
use crate::log_transfer::networking::serialize::LogTransferMessage;

use crate::messages::SystemMessage;
use crate::ordering_protocol::networking::serialize::{OrderingProtocolMessage, ViewTransferProtocolMessage};
use crate::serialize::Service;
use crate::smr::networking::NodeWrap;
use crate::state_transfer::networking::serialize::StateTransferMessage;

pub trait LogTransferSendNode<RQ, OP, LPM> : Send + Sync where LPM: LogTransferMessage<RQ, OP> {

    /// Our own ID
    #[inline(always)]
    fn id(&self) -> NodeId;

    /// Sends a message to a given target.
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the target or err if not. No other checks are made
    /// on the success of the message dispatch
    #[inline(always)]
    fn send(&self, message: LPM::LogTransferMessage, target: NodeId, flush: bool) -> Result<()>;

    /// Sends a signed message to a given target
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the target or err if not. No other checks are made
    /// on the success of the message dispatch
    #[inline(always)]
    fn send_signed(&self, message: LPM::LogTransferMessage, target: NodeId, flush: bool) -> Result<()>;

    /// Broadcast a message to all of the given targets
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    #[inline(always)]
    fn broadcast(&self, message: LPM::LogTransferMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>>;

    /// Broadcast a signed message for all of the given targets
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    #[inline(always)]
    fn broadcast_signed(&self, message: LPM::LogTransferMessage, target: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>>;

    /// Serialize a message to a given target.
    /// Creates the serialized byte buffer along with the header, so we can send it later.
    #[inline(always)]
    fn serialize_digest_message(&self, message: LPM::LogTransferMessage) -> Result<(SerializedMessage<LPM::LogTransferMessage>, Digest)>;

    /// Broadcast the serialized messages provided.
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    #[inline(always)]
    fn broadcast_serialized(&self, messages: BTreeMap<NodeId, StoredSerializedProtocolMessage<LPM::LogTransferMessage>>) -> std::result::Result<(), Vec<NodeId>>;
}

impl<NT, D, P, S, L, VT, NI, RM> LogTransferSendNode<D::Request, P, L> for NodeWrap<NT, D, P, S, L,VT, NI, RM>
    where D: ApplicationData + 'static,
          P: OrderingProtocolMessage<D::Request> + 'static,
          S: StateTransferMessage + 'static,
          L: LogTransferMessage<D::Request, P> + 'static,
          VT: ViewTransferProtocolMessage + 'static,
          RM: Serializable + 'static,
          NI: NetworkInformationProvider + 'static,
          NT: FullNetworkNode<NI, RM, Service<D, P, S, L, VT>>, {
    #[inline(always)]
    fn id(&self) -> NodeId {
        self.0.id()
    }

    #[inline(always)]
    fn send(&self, message: L::LogTransferMessage, target: NodeId, flush: bool) -> Result<()> {
        self.0.send(SystemMessage::from_log_transfer_message(message), target, flush)
    }

    #[inline(always)]
    fn send_signed(&self, message: L::LogTransferMessage, target: NodeId, flush: bool) -> Result<()> {
        self.0.send_signed(SystemMessage::from_log_transfer_message(message), target, flush)
    }

    fn broadcast(&self, message: L::LogTransferMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>> {
        self.0.broadcast(SystemMessage::from_log_transfer_message(message), targets)
    }

    fn broadcast_signed(&self, message: L::LogTransferMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>> {
        self.0.broadcast_signed(SystemMessage::from_log_transfer_message(message), targets)
    }

    /// Why do we do this wrapping/unwrapping? Well, since we want to avoid having to store all of the
    /// generics that are used at the replica level (with all message types), we can't
    /// just return a system message type.
    /// This way, we can still keep this working well with just very small memory changes (to the stack)
    /// and avoid having to store all those unnecessary types in generics
    #[inline(always)]
    fn serialize_digest_message(&self, message: L::LogTransferMessage) -> Result<(SerializedMessage<L::LogTransferMessage>, Digest)> {
        let (message, digest) = self.0.serialize_digest_message(SystemMessage::from_log_transfer_message(message))?;

        let (message, bytes) = message.into_inner();

        let message = message.into_log_transfer_message();

        Ok((SerializedMessage::new(message, bytes), digest))
    }

    /// Read comment above
    #[inline(always)]
    fn broadcast_serialized(&self, messages: BTreeMap<NodeId, StoredSerializedProtocolMessage<L::LogTransferMessage>>) -> std::result::Result<(), Vec<NodeId>> {
        let mut map = BTreeMap::new();

        for (node, message) in messages.into_iter() {
            let (header, message) = message.into_inner();

            let (message, bytes) = message.into_inner();

            let sys_msg = SystemMessage::from_log_transfer_message(message);

            let serialized_msg = SerializedMessage::new(sys_msg, bytes);

            map.insert(node, StoredMessage::new(header, serialized_msg));
        }

        self.0.broadcast_serialized(map)
    }
}