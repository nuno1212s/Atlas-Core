use atlas_communication::reconfiguration_node::NetworkInformationProvider;

use crate::ordering_protocol::networking::serialize::OrderingProtocolMessage;
use crate::ordering_protocol::networking::signature_ver::OrderProtocolSignatureVerificationHelper;

pub trait LogTransferVerificationHelper<RQ, OP, NI>: OrderProtocolSignatureVerificationHelper<RQ, OP, NI>
    where OP: OrderingProtocolMessage<RQ>, NI: NetworkInformationProvider {}
