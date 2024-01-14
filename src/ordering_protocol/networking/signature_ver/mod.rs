use std::sync::Arc;

use atlas_common::error::*;
use atlas_communication::message::Header;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;

use crate::ordering_protocol::networking::serialize::{OrderingProtocolMessage};

/// This is a helper trait to verify signatures of messages for the ordering protocol
pub trait OrderProtocolSignatureVerificationHelper<RQ, OP, NI> where OP: OrderingProtocolMessage<RQ>,
                                                                     NI: NetworkInformationProvider {
    /// This is a helper to verify internal player requests
    fn verify_request_message(network_info: &Arc<NI>, header: &Header, request: RQ) -> Result<RQ>;

    /// helper mostly to verify forwarded consensus messages, for example
    fn verify_protocol_message(network_info: &Arc<NI>, header: &Header, message: OP::ProtocolMessage) -> Result<OP::ProtocolMessage>;
}
