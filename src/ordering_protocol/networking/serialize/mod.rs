use std::fmt::{Debug, Formatter};
use std::sync::Arc;
#[cfg(feature = "serialize_serde")]
use serde::{Deserialize, Serialize};
use atlas_common::node_id::NodeId;
use atlas_common::error::*;
use atlas_common::ordering::{Orderable, SeqNo};
use atlas_communication::message::Header;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;
use atlas_smr_application::serialize::ApplicationData;
use crate::ordering_protocol::networking::signature_ver::OrderProtocolSignatureVerificationHelper;

/// The basic methods needed for a view
pub trait NetworkView: Orderable + Clone + Debug {
    fn primary(&self) -> NodeId;

    fn quorum(&self) -> usize;

    fn quorum_members(&self) -> &Vec<NodeId>;

    fn f(&self) -> usize;

    fn n(&self) -> usize;
}

pub trait OrderProtocolLog: Orderable {
    // At the moment I only need orderable, but I might need more in the future
    fn first_seq(&self) -> Option<SeqNo>;
}

/// Proof of the order protocol message
pub trait OrderProtocolProof: Orderable {
    // At the moment I only need orderable, but I might need more in the future

    fn contained_messages(&self) -> usize;
}

pub trait PermissionedOrderingProtocolMessage: Send + Sync {
    #[cfg(feature = "serialize_capnp")]
    type ViewInfo: NetworkView + Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type ViewInfo: NetworkView + for<'a> Deserialize<'a> + Serialize + Send + Clone + Debug;
}

pub trait ViewTransferProtocolMessage: Send + Sync {
    /// The general protocol type for all messages in the View Transfer protocol
    #[cfg(feature = "serialize_capnp")]
    type ProtocolMessage: Orderable + Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type ProtocolMessage: for<'a> Deserialize<'a> + Serialize + Send + Clone + Debug;

    fn verify_view_transfer_message<NI, D, OPM, OPVH>(network_info: &Arc<NI>,
                                                       header: &Header,
                                                       message: Self::ProtocolMessage) -> Result<Self::ProtocolMessage>
        where NI: NetworkInformationProvider,
              D: ApplicationData,
              OPM: OrderingProtocolMessage<D>,
              OPVH: OrderProtocolSignatureVerificationHelper<D, OPM, NI>, Self: Sized;
}

/// We do not need a serde module since serde serialization is just done on the network level.
/// The abstraction for ordering protocol messages.
pub trait OrderingProtocolMessage<D>: Send + Sync {
    /// The general protocol type for all messages in the ordering protocol
    #[cfg(feature = "serialize_capnp")]
    type ProtocolMessage: Orderable + Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type ProtocolMessage: Orderable + for<'a> Deserialize<'a> + Serialize + Send + Clone + Debug;

    /// The metadata type for storing the proof in the persistent storage
    /// Since the message will be stored in the persistent storage, it needs to be serializable
    /// This should provide all the necessary final information to assemble the proof from the messages
    #[cfg(feature = "serialize_capnp")]
    type ProofMetadata: Orderable + Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type ProofMetadata: Orderable + for<'a> Deserialize<'a> + Serialize + Send + Clone;

    fn verify_order_protocol_message<NI, OPVH>(network_info: &Arc<NI>,
                                               header: &Header,
                                               message: Self::ProtocolMessage) -> Result<Self::ProtocolMessage>
        where NI: NetworkInformationProvider,
              OPVH: OrderProtocolSignatureVerificationHelper<D, Self, NI>,
              D: ApplicationData, Self: Sized;

    #[cfg(feature = "serialize_capnp")]
    fn serialize_capnp(builder: febft_capnp::consensus_messages_capnp::protocol_message::Builder, msg: &Self::ProtocolMessage) -> Result<()>;

    #[cfg(feature = "serialize_capnp")]
    fn deserialize_capnp(reader: febft_capnp::consensus_messages_capnp::protocol_message::Reader) -> Result<Self::ProtocolMessage>;

    #[cfg(feature = "serialize_capnp")]
    fn serialize_view_capnp(builder: febft_capnp::cst_messages_capnp::view_info::Builder, msg: &Self::ViewInfo) -> Result<()>;

    #[cfg(feature = "serialize_capnp")]
    fn deserialize_view_capnp(reader: febft_capnp::cst_messages_capnp::view_info::Reader) -> Result<Self::ViewInfo>;

    #[cfg(feature = "serialize_capnp")]
    fn serialize_proof_capnp(builder: febft_capnp::cst_messages_capnp::proof::Builder, msg: &Self::Proof) -> Result<()>;

    #[cfg(feature = "serialize_capnp")]
    fn deserialize_proof_capnp(reader: febft_capnp::cst_messages_capnp::proof::Reader) -> Result<Self::Proof>;
}