use crate::ordering_protocol::networking::serialize::PermissionedOrderingProtocolMessage;
use crate::ordering_protocol::View;

/// A permissioned ordering protocol, meaning only a select few are actually part of the quorum that decides the
/// ordering of the operations.
pub trait PermissionedOrderingProtocol {
    type PermissionedSerialization: PermissionedOrderingProtocolMessage + 'static;

    /// Get the current view of the ordering protocol
    fn view(&self) -> View<Self::PermissionedSerialization>;

    /// Install a given view into the ordering protocol
    fn install_view(&mut self, view: View<Self::PermissionedSerialization>);
}