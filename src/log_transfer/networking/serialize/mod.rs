use std::sync::Arc;

use atlas_common::error::*;
use atlas_common::serialization_helper::SerType;
use atlas_communication::message::Header;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;

use crate::log_transfer::networking::signature_ver::LogTransferVerificationHelper;
use crate::ordering_protocol::networking::serialize::OrderingProtocolMessage;

/// The abstraction for log transfer protocol messages.
/// This allows us to have any log transfer protocol work with the same backbone
pub trait LogTransferMessage<RQ, OP>: Send + Sync {
    
    /// The message type for the log transfer protocol
    type LogTransferMessage: SerType;

    /// Verify the message and return the message if it is valid
    fn verify_log_message<NI, LVH>(network_info: &Arc<NI>,
                                   header: &Header,
                                   message: Self::LogTransferMessage) -> Result<Self::LogTransferMessage>
        where NI: NetworkInformationProvider,
              LVH: LogTransferVerificationHelper<RQ, OP, NI>,
              OP: OrderingProtocolMessage<RQ>;

    #[cfg(feature = "serialize_capnp")]
    fn serialize_capnp(builder: febft_capnp::cst_messages_capnp::cst_message::Builder, msg: &Self::LogTransferMessage) -> Result<()>;

    #[cfg(feature = "serialize_capnp")]
    fn deserialize_capnp(reader: febft_capnp::cst_messages_capnp::cst_message::Reader) -> Result<Self::LogTransferMessage>;
}