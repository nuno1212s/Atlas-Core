use atlas_communication::message_signing::NetworkMessageSignatureVerifier;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;
use atlas_smr_application::serialize::ApplicationData;
use crate::log_transfer::networking::serialize::LogTransferMessage;
use crate::messages::signature_ver::SigVerifier;
use crate::ordering_protocol::networking::serialize::{OrderingProtocolMessage, ViewTransferProtocolMessage};
use crate::serialize::Service;
use crate::state_transfer::networking::serialize::StateTransferMessage;

/// State transfer messages don't really need internal verifications, since the entire message is signed
/// and the signature is verified by the network layer (by verifying the entire image)
pub trait StateTransferVerificationHelper {}

impl<SV, NI, D, OP, LT, ST, VT> StateTransferVerificationHelper for SigVerifier<SV, NI, D, OP, ST, LT, VT>
    where D: ApplicationData + 'static,
          OP: OrderingProtocolMessage<D::Request> + 'static,
          LT: LogTransferMessage<D::Request, OP> + 'static,
          ST: StateTransferMessage + 'static,
          VT: ViewTransferProtocolMessage + 'static,
          NI: NetworkInformationProvider + 'static,
          SV: NetworkMessageSignatureVerifier<Service<D, OP, ST, LT, VT>, NI> {}