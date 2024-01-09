use atlas_communication::FullNetworkNode;
use atlas_communication::message_signing::NetworkMessageSignatureVerifier;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;
use atlas_communication::serialize::Serializable;
use atlas_smr_application::serialize::ApplicationData;
use crate::log_transfer::networking::serialize::LogTransferMessage;
use crate::messages::signature_ver::SigVerifier;
use crate::ordering_protocol::networking::serialize::{OrderingProtocolMessage, ViewTransferProtocolMessage};
use crate::ordering_protocol::networking::signature_ver::OrderProtocolSignatureVerificationHelper;
use crate::serialize::Service;
use crate::state_transfer::networking::serialize::StateTransferMessage;

pub trait LogTransferVerificationHelper<RQ, OP, NI>: OrderProtocolSignatureVerificationHelper<RQ, OP, NI>
    where OP: OrderingProtocolMessage<RQ>, NI: NetworkInformationProvider {}

impl<SV, NI, D, OP, ST, LT, VT> LogTransferVerificationHelper<D::Request, OP, NI> for SigVerifier<SV, NI, D, OP, ST, LT, VT>
    where D: ApplicationData + 'static,
          OP: OrderingProtocolMessage<D::Request> + 'static,
          ST: StateTransferMessage + 'static,
          LT: LogTransferMessage<D::Request, OP> + 'static,
          VT: ViewTransferProtocolMessage + 'static,
          NI: NetworkInformationProvider + 'static,
          SV: NetworkMessageSignatureVerifier<Service<D, OP, ST, LT, VT>, NI> {}