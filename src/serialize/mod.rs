use std::fmt::Debug;
use std::sync::Arc;

#[cfg(feature = "serialize_serde")]
use serde::{Deserialize, Serialize};

use atlas_common::node_id::NodeId;
use atlas_common::ordering::{Orderable, SeqNo};
use atlas_communication::message::Header;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;
use atlas_communication::serialize::Serializable;

use crate::ordering_protocol::networking::serialize::{NetworkView, OrderingProtocolMessage, OrderProtocolProof, ViewTransferProtocolMessage};
use crate::ordering_protocol::networking::signature_ver::OrderProtocolSignatureVerificationHelper;

#[cfg(feature = "serialize_capnp")]
pub mod capnp;

/// Reconfiguration protocol messages
pub trait ReconfigurationProtocolMessage: Serializable + Send + Sync {
    #[cfg(feature = "serialize_capnp")]
    type QuorumJoinCertificate: Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type QuorumJoinCertificate: for<'a> Deserialize<'a> + Serialize + Send + Clone;
}


#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Debug)]
pub struct NoProtocol;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
pub struct NoView;

impl Orderable for NoView {
    fn sequence_number(&self) -> SeqNo {
        unimplemented!()
    }
}

impl NetworkView for NoView {
    fn primary(&self) -> NodeId {
        unimplemented!()
    }

    fn quorum(&self) -> usize {
        unimplemented!()
    }

    fn quorum_members(&self) -> &Vec<NodeId> {
        unimplemented!()
    }

    fn f(&self) -> usize {
        unimplemented!()
    }

    fn n(&self) -> usize {
        unimplemented!()
    }
}

impl<RQ> OrderingProtocolMessage<RQ> for NoProtocol {
    type ProtocolMessage = ();

    type ProofMetadata = ();

    fn verify_order_protocol_message<NI, OPVH>(network_info: &Arc<NI>, header: &Header, message: Self::ProtocolMessage)
                                               -> atlas_common::error::Result<Self::ProtocolMessage> where NI: NetworkInformationProvider,
                                                                                                           OPVH: OrderProtocolSignatureVerificationHelper<RQ, Self, NI> {
        Ok(message)
    }

    #[cfg(feature = "serialize_capnp")]
    fn serialize_capnp(_: febft_capnp::consensus_messages_capnp::protocol_message::Builder, _: &Self::ProtocolMessage) -> Result<()> {
        unimplemented!()
    }

    #[cfg(feature = "serialize_capnp")]
    fn deserialize_capnp(_: febft_capnp::consensus_messages_capnp::protocol_message::Reader) -> Result<Self::ProtocolMessage> {
        unimplemented!()
    }

    #[cfg(feature = "serialize_capnp")]
    fn serialize_view_capnp(_: febft_capnp::cst_messages_capnp::view_info::Builder, msg: &Self::ViewInfo) -> Result<()> {
        unimplemented!()
    }

    #[cfg(feature = "serialize_capnp")]
    fn deserialize_view_capnp(_: febft_capnp::cst_messages_capnp::view_info::Reader) -> Result<Self::ViewInfo> {
        unimplemented!()
    }
}


impl ViewTransferProtocolMessage for NoProtocol {
    type ProtocolMessage = ();

    fn verify_view_transfer_message<NI>(network_info: &Arc<NI>, header: &Header, message: Self::ProtocolMessage) -> atlas_common::error::Result<Self::ProtocolMessage>
        where NI: NetworkInformationProvider, Self: Sized {
        Ok(message)
    }
}

impl OrderProtocolProof for () {
    fn contained_messages(&self) -> usize {
        0
    }
}
