use std::collections::BTreeMap;
use std::sync::Arc;

use atlas_common::crypto::hash::Digest;
use atlas_common::error::*;
use atlas_common::node_id::NodeId;
use atlas_common::serialization_helper::SerType;
use atlas_communication::message::{SerializedMessage, StoredSerializedMessage};
use atlas_communication::reconfiguration_node::NetworkInformationProvider;

use crate::messages::ForwardedRequestsMessage;
use crate::ordering_protocol::networking::serialize::{OrderingProtocolMessage, ViewTransferProtocolMessage};

pub mod signature_ver;
pub mod serialize;

pub trait OrderProtocolSendNode<RQ, OPM>: Send + Sync where RQ: SerType, OPM: OrderingProtocolMessage<RQ> {
    type NetworkInfoProvider: NetworkInformationProvider + 'static;

    fn id(&self) -> NodeId;

    /// The network information provider
    fn network_info_provider(&self) -> &Arc<Self::NetworkInfoProvider>;

    /// Forward requests to the given targets
    fn forward_requests(&self, fwd_requests: ForwardedRequestsMessage<RQ>, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>>;

    /// Sends a message to a given target.
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the target or err if not. No other checks are made
    /// on the success of the message dispatch
    fn send(&self, message: OPM::ProtocolMessage, target: NodeId, flush: bool) -> Result<()>;

    /// Sends a signed message to a given target
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the target or err if not. No other checks are made
    /// on the success of the message dispatch
    fn send_signed(&self, message: OPM::ProtocolMessage, target: NodeId, flush: bool) -> Result<()>;

    /// Broadcast a message to all of the given targets
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    fn broadcast(&self, message: OPM::ProtocolMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>>;

    /// Broadcast a signed message for all of the given targets
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    fn broadcast_signed(&self, message: OPM::ProtocolMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>>;

    /// Serialize a message to a given target.
    /// Creates the serialized byte buffer along with the header, so we can send it later.
    fn serialize_digest_message(&self, message: OPM::ProtocolMessage) -> Result<(SerializedMessage<OPM::ProtocolMessage>, Digest)>;

    /// Broadcast the serialized messages provided.
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    fn broadcast_serialized(&self, messages: BTreeMap<NodeId, StoredSerializedMessage<OPM::ProtocolMessage>>) -> std::result::Result<(), Vec<NodeId>>;
}

pub trait ViewTransferProtocolSendNode<VT>: Send + Sync where
    VT: ViewTransferProtocolMessage {

    type NetworkInfoProvider: NetworkInformationProvider + 'static;

    fn id(&self) -> NodeId;

    /// The network information provider
    fn network_info_provider(&self) -> &Arc<Self::NetworkInfoProvider>;

    /// Sends a message to a given target.
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the target or err if not. No other checks are made
    /// on the success of the message dispatch
    fn send(&self, message: VT::ProtocolMessage, target: NodeId, flush: bool) -> Result<()>;

    /// Sends a signed message to a given target
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the target or err if not. No other checks are made
    /// on the success of the message dispatch
    fn send_signed(&self, message: VT::ProtocolMessage, target: NodeId, flush: bool) -> Result<()>;

    /// Broadcast a message to all of the given targets
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    fn broadcast(&self, message: VT::ProtocolMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>>;

    /// Broadcast a signed message for all of the given targets
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    fn broadcast_signed(&self, message: VT::ProtocolMessage, targets: impl Iterator<Item=NodeId>) -> std::result::Result<(), Vec<NodeId>>;

    /// Serialize a message to a given target.
    /// Creates the serialized byte buffer along with the header, so we can send it later.
    fn serialize_digest_message(&self, message: VT::ProtocolMessage) -> Result<(SerializedMessage<VT::ProtocolMessage>, Digest)>;

    /// Broadcast the serialized messages provided.
    /// Does not block on the message sent. Returns a result that is
    /// Ok if there is a current connection to the targets or err if not. No other checks are made
    /// on the success of the message dispatch
    fn broadcast_serialized(&self, messages: BTreeMap<NodeId, StoredSerializedMessage<VT::ProtocolMessage>>) -> std::result::Result<(), Vec<NodeId>>;
}

