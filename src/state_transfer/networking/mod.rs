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

pub trait StateTransferSendNode<STM> where STM: StateTransferMessage {
    fn id(&self) -> NodeId;

    /// Sends a message to a given target.
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the target or err if not. No other checks are made
    /// on the success of the message dispatch
    fn send(&self, message: STM::StateTransferMessage, target: NodeId, flush: bool) -> Result<()>;

    /// Sends a signed message to a given target
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the target or err if not. No other checks are made
    /// on the success of the message dispatch
    fn send_signed(&self, message: STM::StateTransferMessage, target: NodeId, flush: bool) -> Result<()>;

    /// Broadcast a message to all of the given targets
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    fn broadcast(&self, message: STM::StateTransferMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>>;

    /// Broadcast a signed message for all of the given targets
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    fn broadcast_signed(&self, message: STM::StateTransferMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>>;

    /// Serialize a message to a given target.
    /// Creates the serialized byte buffer along with the header, so we can send it later.
    fn serialize_digest_message(&self, message: STM::StateTransferMessage) -> Result<(SerializedMessage<STM::StateTransferMessage>, Digest)>;

    /// Broadcast the serialized messages provided.
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    fn broadcast_serialized(&self, messages: BTreeMap<NodeId, StoredSerializedProtocolMessage<STM::StateTransferMessage>>) -> std::result::Result<(), Vec<NodeId>>;
}

impl<NT, D, P, S, L, VT, NI, RM> StateTransferSendNode<S> for NodeWrap<NT, D, P, S, L, VT, NI, RM>
    where D: ApplicationData + 'static,
          P: OrderingProtocolMessage<D::Request> + 'static,
          L: LogTransferMessage<D::Request, P> + 'static,
          S: StateTransferMessage + 'static,
          VT: ViewTransferProtocolMessage + 'static,
          RM: Serializable + 'static,
          NI: NetworkInformationProvider + 'static,
          NT: FullNetworkNode<NI, RM, Service<D, P, S, L, VT>>, {
    #[inline(always)]
    fn id(&self) -> NodeId {
        self.0.id()
    }

    #[inline(always)]
    fn send(&self, message: S::StateTransferMessage, target: NodeId, flush: bool) -> Result<()> {
        self.0.send(SystemMessage::from_state_transfer_message(message), target, flush)
    }

    #[inline(always)]
    fn send_signed(&self, message: S::StateTransferMessage, target: NodeId, flush: bool) -> Result<()> {
        self.0.send_signed(SystemMessage::from_state_transfer_message(message), target, flush)
    }

    #[inline(always)]
    fn broadcast(&self, message: S::StateTransferMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>> {
        self.0.broadcast(SystemMessage::from_state_transfer_message(message), targets)
    }

    #[inline(always)]
    fn broadcast_signed(&self, message: S::StateTransferMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>> {
        self.0.broadcast_signed(SystemMessage::from_state_transfer_message(message), targets)
    }

    #[inline(always)]
    fn serialize_digest_message(&self, message: S::StateTransferMessage) -> Result<(SerializedMessage<S::StateTransferMessage>, Digest)> {
        let (message, digest) = self.0.serialize_digest_message(SystemMessage::from_state_transfer_message(message))?;

        let (message, bytes) = message.into_inner();

        let message = message.into_state_tranfer_message();

        Ok((SerializedMessage::new(message, bytes), digest))
    }

    #[inline(always)]
    fn broadcast_serialized(&self, messages: BTreeMap<NodeId, StoredSerializedProtocolMessage<S::StateTransferMessage>>) -> std::result::Result<(), Vec<NodeId>> {
        let mut map = BTreeMap::new();

        for (node, message) in messages.into_iter() {
            let (header, message) = message.into_inner();

            let (message, bytes) = message.into_inner();

            let sys_msg = SystemMessage::from_state_transfer_message(message);

            let serialized_msg = SerializedMessage::new(sys_msg, bytes);

            map.insert(node, StoredMessage::new(header, serialized_msg));
        }

        self.0.broadcast_serialized(map)
    }
}