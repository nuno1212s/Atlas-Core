use atlas_common::node_id::NodeId;
use atlas_common::error::*;
use crate::ordering_protocol::networking::serialize::PermissionedOrderingProtocolMessage;
use crate::ordering_protocol::View;
use crate::timeouts::RqTimeout;

/// A permissioned ordering protocol, meaning only a select few are actually part of the quorum that decides the
/// ordering of the operations.
pub trait PermissionedOrderingProtocol {
    type PermissionedSerialization: PermissionedOrderingProtocolMessage + 'static;

    /// Get the current view of the ordering protocol
    fn view(&self) -> View<Self::PermissionedSerialization>;

    /// Install a given view into the ordering protocol
    fn install_view(&mut self, view: View<Self::PermissionedSerialization>);
}

pub enum VCResult {

}

pub enum VCTimeoutResult {

}

pub trait ViewTransferProtocol<OP, NT> where OP: PermissionedOrderingProtocol {


    fn initialize_view_transfer_protocol(view: Vec<NodeId>) -> Self;

    fn request_latest_view(&mut self) -> Result<()>;

    /// Process a view transfer protocol message
    fn process_message(&mut self) -> Result<VCResult>;

    fn handle_timeout(&mut self, timeout: Vec<RqTimeout>) -> Result<VCTimeoutResult>;

}