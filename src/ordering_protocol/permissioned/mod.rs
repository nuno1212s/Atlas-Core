use std::sync::Arc;

use atlas_common::error::*;
use atlas_common::node_id::NodeId;
use atlas_communication::message::StoredMessage;

use crate::ordering_protocol::{OrderingProtocol, PermissionedOrderingProtocol};
use crate::ordering_protocol::networking::serialize::ViewTransferProtocolMessage;
use crate::ordering_protocol::networking::ViewTransferProtocolSendNode;
use crate::timeouts::RqTimeout;

/// The result of processing a message with the view transfer protocol
pub enum VTResult {
    // Run the view transfer protocol
    RunVTP,
    // View transfer is not necessary
    VTransferNotNeeded,
    // View Transfer is running
    VTransferRunning,
    // View transfer finished
    VTransferFinished,
}

/// Result of Polling the view transfer protocol
pub enum VTPollResult<VT> {
    ReceiveMsg,
    RePoll,
    Exec(StoredMessage<VT>),
    VTResult(VTResult),
}

/// Result of the view transfer protocol
pub enum VTTimeoutResult {
    RunVTP,
    VTPNotNeeded,
}

pub type VTMsg<VT> = <VT as ViewTransferProtocolMessage>::ProtocolMessage;

/// View Transfer protocol abstraction
/// This is not to be confused with any type of view change protocol,
/// it is merely a protocol to transfer the view of a given node which is trying to join
/// the protocol from the currently available replicas
/// The view change protocol (if applicable) is to be implemented by the
/// [OrderingProtocol]
pub trait ViewTransferProtocol<OP, NT> {
    type Serialization: ViewTransferProtocolMessage + 'static;

    type Config;

    /// Initialize the view transfer protocol
    fn initialize_view_transfer_protocol(config: Self::Config, net: Arc<NT>, view: Vec<NodeId>) -> Result<Self>
        where NT: ViewTransferProtocolSendNode<Self::Serialization>,
              Self: Sized;

    /// View transfer poll result
    fn poll(&mut self) -> Result<VTPollResult<VTMsg<Self::Serialization>>>
        where NT: ViewTransferProtocolSendNode<Self::Serialization>;

    /// Request a view transfer
    fn request_latest_view(&mut self, op: &OP) -> Result<()>
        where NT: ViewTransferProtocolSendNode<Self::Serialization>,
              OP: PermissionedOrderingProtocol;

    fn handle_off_context_msg(&mut self, op: &OP, message: StoredMessage<VTMsg<Self::Serialization>>) -> Result<VTResult>
        where NT: ViewTransferProtocolSendNode<Self::Serialization>,
              OP: PermissionedOrderingProtocol;

    /// Process a view transfer protocol message
    fn process_message(&mut self, op: &mut OP, message: StoredMessage<VTMsg<Self::Serialization>>) -> Result<VTResult>
        where NT: ViewTransferProtocolSendNode<Self::Serialization>,
              OP: PermissionedOrderingProtocol;

    /// Handle a timeout
    fn handle_timeout(&mut self, timeout: Vec<RqTimeout>) -> Result<VTTimeoutResult>
        where NT: ViewTransferProtocolSendNode<Self::Serialization>,
              OP: PermissionedOrderingProtocol;
}