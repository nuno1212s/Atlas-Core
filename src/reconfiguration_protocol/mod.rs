#![allow(type_alias_bounds)]

use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use atlas_common::channel::sync::{ChannelSyncRx, ChannelSyncTx};
use atlas_common::crypto::threshold_crypto::{PrivateKeyPart, PublicKeyPart, PublicKeySet};
use atlas_common::error::*;
use atlas_common::node_id::NodeId;
use atlas_communication::reconfiguration::{
    NetworkInformationProvider, NodeInfo, ReconfigurationNetworkCommunication,
};
use atlas_communication::stub::RegularNetworkStub;

use crate::serialize::ReconfigurationProtocolMessage;
use crate::timeouts::timeout::{ModTimeout, TimeoutModHandle, TimeoutableMod};

/// Messages to be sent by the reconfiguration protocol
/// to the ordering protocol relating changes that have undergone in the
/// Quorum View.
#[derive(Debug)]
pub enum QuorumReconfigurationMessage {
    /// The reconfiguration protocol has reached stability and we can now start to execute the other
    /// as we know the current members of the quorum. This will be used to run state transfer protocols
    /// Quorum protocol, with the given base nodes
    ReconfigurationProtocolStable(Vec<NodeId>),

    /// We have received a quorum update from other nodes and as such we must update our quorum view
    /// This will only be received when we are not a part of the quorum
    QuorumUpdated(Vec<NodeId>),

    // We have been granted permission into an existing quorum, and we must
    // now indicate to the ordering protocol that he can attempt to join the quorum
    RequestQuorumJoin(NodeId),

    // We are going to attempt to join the quorum
    AttemptToJoinQuorum,
}

/// A message that the reconfiguration protocol will emit
/// on network update events
pub enum NodeConnectionUpdateMessage {
    // A node has connected to us.
    NodeConnected(NodeInfo),
    // A node has disconnected from us.
    NodeDisconnected(NodeInfo),
}

pub type ConnectionUpdateChannelHandle = ChannelSyncTx<NodeConnectionUpdateMessage>;

/// Messages sent by the ordering protocol to notify the reconfiguration protocol of changes
/// to the quorum
#[derive(Debug)]
pub enum QuorumReconfigurationResponse {
    QuorumStableResponse(bool),
    QuorumAlterationResponse(QuorumAlterationResponse),
    QuorumAttemptJoinResponse(QuorumAttemptJoinResponse),
}

#[derive(Debug)]
pub enum QuorumAttemptJoinResponse {
    Success,
    Failed,
}

/// Response destined to the ordering protocol, indicating the result of the quorum alteration
/// Requested by it
#[derive(Debug)]
pub enum QuorumAlterationResponse {
    Successful(NodeId, usize),
    Failed(NodeId, AlterationFailReason),
}

/// Reasons for a failed quorum alteration
#[derive(Debug)]
pub enum AlterationFailReason {
    /// The node failed for some reason
    Failed,
    /// The node join request failed because there was already an ongoing reconfiguration request
    OngoingReconfiguration,
    /// We are already part of the quorum, so why are we trying to join it again?
    AlreadyPartOfQuorum,
}

/// Analogous to the `QuorumReconfigurationMessage`, this is the message that the ordering protocol
/// will send to the reconfiguration protocol to notify it of changes in the quorum view
/// This is aimed for clients, which only listen to quorum updates, they don't actually participate
pub enum QuorumUpdateMessage {
    UpdatedQuorumView(Vec<NodeId>),
}

/// The communication handles for communication between the orchestrator
/// and the reconfiguration protocol
pub struct ReconfigurationCommunicationHandles {
    network_update: ChannelSyncTx<NodeConnectionUpdateMessage>,
    node_type: ReconfigurableNodeType,
}

/// The type of reconfigurable nodes.
/// Quorum nodes are nodes that partake in the quorum
/// Client nodes are nodes that only listen to quorum updates so they know who
/// to contact in order to perform operations
pub enum ReconfigurableNodeType {
    ClientNode(ChannelSyncTx<QuorumUpdateMessage>),
    QuorumNode(
        ChannelSyncTx<QuorumReconfigurationMessage>,
        ChannelSyncRx<QuorumReconfigurationResponse>,
    ),
}

pub type QuorumJoinCert<RP: ReconfigurationProtocolMessage> = RP::QuorumJoinCertificate;

pub enum ReconfigResponse {
    Running,
    Stop,
}

#[allow(async_fn_in_trait)]
/// The trait defining the necessary functionality for a reconfiguration protocol (at least at the moment)
///
/// This is different from the other protocols like ordering, state and log transfer since we actually
/// Run this protocol independently from the rest of the system, only sending and receiving updates
/// through message passing (unlike the state and log transfer, which run in the same thread since
/// we cannot execute the ordering protocol while we are executing log and state transfers)
///
/// This is done since the messaging of the reconfiguration protocol is actually separate from the messaging of
/// the other protocols, since we can only establish secure communication with the nodes.
/// Therefore, for a given node to be able to deliver protocol messages, he must first successfully authenticate
/// with the reconfiguration protocol and be allowed into the known network.
///
/// The reconfiguration protocol acts as the network information acquirer, acquiring new nodes,
/// verifying their integrity and correctness
pub trait ReconfigurationProtocol:
    TimeoutableMod<ReconfigResponse> + Send + Sync + 'static
{
    // The configuration type the protocol wants to receive
    type Config;

    /// Type of the information provider that the protocol will provide
    type InformationProvider: NetworkInformationProvider;

    /// Type of the message that the protocol will use, to be used by the networking layer
    type Serialization: ReconfigurationProtocolMessage + 'static;

    /// Initialize a default information object from the provided configuration.
    /// This object will be used to initialize the networking protocol.
    fn init_default_information(config: Self::Config) -> Result<Arc<Self::InformationProvider>>;

    /// After initializing the networking protocol with the necessary information provider,
    /// we can then start to initialize the reconfiguration protocol. At the moment, differently from
    /// the ordering, state transfer and log transfer protocols, the reconfiguration protocol
    /// is meant to run completely independently from the rest of the system, only sending and receiving
    /// updates
    async fn initialize_protocol<NT>(
        information: Arc<Self::InformationProvider>,
        node: Arc<NT>,
        timeouts: TimeoutModHandle,
        node_type: ReconfigurationCommunicationHandles,
        reconfig: ReconfigurationNetworkCommunication,
        min_stable_node_count: usize,
    ) -> Result<Self>
    where
        NT: RegularNetworkStub<Self::Serialization> + 'static,
        Self: Sized;

    /// Get the current quorum members of the system
    fn get_quorum_members(&self) -> Vec<NodeId>;

    /// Get the current amount of faults the quorum can tolerate
    fn get_current_f(&self) -> usize;

    /// Get the current quorum view
    fn quorum_state(&self) -> (Vec<NodeId>, usize);

    /// Check if a given join certificate is valid
    fn is_join_certificate_valid(&self, certificate: &QuorumJoinCert<Self::Serialization>) -> bool;

    fn handle_timeouts_safe(&self, timeouts: Vec<ModTimeout>) -> Result<ReconfigResponse>;
}

/// Threshold crypto information about the current network
pub trait QuorumThresholdCrypto: Send + Sync {
    /// Get our own public key part
    fn own_pub_key(&self) -> PublicKeyPart;

    /// Get the public key part that corresponds to a given node
    fn pub_key_for_node(&self, node: NodeId) -> PublicKeyPart;

    /// The set of public key, generated collaboratively by the quorum
    fn pub_key_set(&self) -> Result<&PublicKeySet>;

    /// Get our own private key part, which we will
    /// use to sign / encrypt information which will
    /// then be combined in order to assure at least threshold
    /// nodes signed/encrypted the same piece of information
    fn get_priv_key_part(&self) -> Result<&PrivateKeyPart>;
}

impl Debug for ReconfigurableNodeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReconfigurableNodeType::ClientNode(_) => {
                write!(f, "Client Node")
            }
            ReconfigurableNodeType::QuorumNode(_, _) => {
                write!(f, "Replica Node")
            }
        }
    }
}

impl From<(ConnectionUpdateChannelHandle, ReconfigurableNodeType)>
    for ReconfigurationCommunicationHandles
{
    fn from(value: (ConnectionUpdateChannelHandle, ReconfigurableNodeType)) -> Self {
        Self {
            network_update: value.0,
            node_type: value.1,
        }
    }
}

impl From<ReconfigurationCommunicationHandles>
    for (ConnectionUpdateChannelHandle, ReconfigurableNodeType)
{
    fn from(value: ReconfigurationCommunicationHandles) -> Self {
        (value.network_update, value.node_type)
    }
}
