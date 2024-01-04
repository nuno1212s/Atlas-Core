use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;

#[cfg(feature = "serialize_serde")]
use serde::{Deserialize, Serialize};

use atlas_common::crypto::hash::Digest;
use atlas_common::error::*;
use atlas_common::node_id::NodeId;
use atlas_common::ordering::{Orderable, SeqNo};
use atlas_communication::message::StoredMessage;
use atlas_communication::protocol_node::ProtocolNetworkNode;
use atlas_smr_application::serialize::ApplicationData;

use crate::timeouts::TimedOut;

pub mod signature_ver;

/// The `Message` type encompasses all the messages traded between different
/// asynchronous tasks in the system.
///
pub enum Message {
    /// We received a timeout from the timeouts layer.
    Timeout(TimedOut),
    /// Timeouts that have already been processed by the request pre processing layer.
    ProcessedTimeout(TimedOut, TimedOut),
}

impl Debug for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::Timeout(_) => {
                write!(f, "timeout")
            }
            Message::ProcessedTimeout(_, _) => {
                write!(f, "Digested Timeouts")
            }
        }
    }
}

#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
pub enum SystemMessage<D: ApplicationData, P, ST, LT, VT> {
    ///An ordered request
    OrderedRequest(RequestMessage<D::Request>),
    ///An unordered request
    UnorderedRequest(RequestMessage<D::Request>),
    ///A reply to an ordered request
    OrderedReply(ReplyMessage<D::Reply>),
    ///A reply to an unordered request
    UnorderedReply(ReplyMessage<D::Reply>),
    ///Requests forwarded from other peers
    ForwardedRequestMessage(ForwardedRequestsMessage<D::Request>),
    ///A message related to the protocol
    ProtocolMessage(Protocol<P>),
    ///A protocol message that has been forwarded by another peer
    ForwardedProtocolMessage(ForwardedProtocolMessage<P>),
    ///A state transfer protocol message
    StateTransferMessage(StateTransfer<ST>),
    ///A Log trasnfer protocol message
    LogTransferMessage(LogTransfer<LT>),
    /// View Transfer protocol message
    ViewTransferMessage(VTMessage<VT>),
}

impl<D, P, ST, LT, VT> SystemMessage<D, P, ST, LT, VT> where D: ApplicationData {
    pub fn from_protocol_message(msg: P) -> Self {
        SystemMessage::ProtocolMessage(Protocol::new(msg))
    }

    pub fn from_state_transfer_message(msg: ST) -> Self {
        SystemMessage::StateTransferMessage(StateTransfer::new(msg))
    }

    pub fn from_log_transfer_message(msg: LT) -> Self {
        SystemMessage::LogTransferMessage(LogTransfer::new(msg))
    }

    pub fn from_view_transfer_message(msg: VT) -> Self {
        SystemMessage::ViewTransferMessage(VTMessage::new(msg))
    }

    pub fn from_fwd_protocol_message(msg: StoredMessage<Protocol<P>>) -> Self {
        SystemMessage::ForwardedProtocolMessage(ForwardedProtocolMessage::new(msg))
    }

    pub fn into_protocol_message(self) -> P {
        match self {
            SystemMessage::ProtocolMessage(prot) => {
                prot.into_inner()
            }
            _ => {
                unreachable!()
            }
        }
    }

    pub fn into_state_tranfer_message(self) -> ST {
        match self {
            SystemMessage::StateTransferMessage(s) => {
                s.into_inner()
            }
            _ => { unreachable!() }
        }
    }

    pub fn into_log_transfer_message(self) -> LT {
        match self {
            SystemMessage::LogTransferMessage(l) => {
                l.into_inner()
            }
            _ => { unreachable!() }
        }
    }

    pub fn into_view_transfer_message(self) -> VT {
        match self {
            SystemMessage::ViewTransferMessage(vt) => {
                vt.into_inner()
            }
            _ => { unreachable!() }
        }
    }
}

impl<D, P, ST, LT, VT> Clone for SystemMessage<D, P, ST, LT, VT> where D: ApplicationData, P: Clone, ST: Clone, LT: Clone, VT: Clone {
    fn clone(&self) -> Self {
        match self {
            SystemMessage::OrderedRequest(req) => {
                SystemMessage::OrderedRequest(req.clone())
            }
            SystemMessage::UnorderedRequest(req) => {
                SystemMessage::UnorderedRequest(req.clone())
            }
            SystemMessage::OrderedReply(rep) => {
                SystemMessage::OrderedReply(rep.clone())
            }
            SystemMessage::UnorderedReply(rep) => {
                SystemMessage::UnorderedReply(rep.clone())
            }
            SystemMessage::ForwardedRequestMessage(fwd_req) => {
                SystemMessage::ForwardedRequestMessage(fwd_req.clone())
            }
            SystemMessage::ProtocolMessage(prot) => {
                SystemMessage::ProtocolMessage(prot.clone())
            }
            SystemMessage::ForwardedProtocolMessage(prot) => {
                SystemMessage::ForwardedProtocolMessage(prot.clone())
            }
            SystemMessage::StateTransferMessage(state_transfer) => {
                SystemMessage::StateTransferMessage(state_transfer.clone())
            }
            SystemMessage::LogTransferMessage(log_transfer) => {
                SystemMessage::LogTransferMessage(log_transfer.clone())
            }
            SystemMessage::ViewTransferMessage(view_transfer) => {
                SystemMessage::ViewTransferMessage(view_transfer.clone())
            }
        }
    }
}

impl<D, P, ST, LT, VT> Debug for SystemMessage<D, P, ST, LT, VT>
    where D: ApplicationData,
          P: Clone, ST: Clone,
          LT: Clone, VT: Clone {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SystemMessage::OrderedRequest(_) => {
                write!(f, "Ordered Request")
            }
            SystemMessage::UnorderedRequest(_) => {
                write!(f, "Unordered Request")
            }
            SystemMessage::OrderedReply(_) => {
                write!(f, "Ordered Reply")
            }
            SystemMessage::UnorderedReply(_) => {
                write!(f, "Unordered Reply")
            }
            SystemMessage::ForwardedRequestMessage(_) => {
                write!(f, "Forwarded Request")
            }
            SystemMessage::ProtocolMessage(_) => {
                write!(f, "Protocol Message")
            }
            SystemMessage::ForwardedProtocolMessage(_) => {
                write!(f, "Forwarded Protocol Message")
            }
            SystemMessage::StateTransferMessage(_) => {
                write!(f, "State Transfer Message")
            }
            SystemMessage::LogTransferMessage(_) => {
                write!(f, "Log transfer message")
            }
            SystemMessage::ViewTransferMessage(_) => {
                write!(f, "View Transfer message")
            }
        }
    }
}

#[derive(Eq, PartialEq, Ord, Clone, PartialOrd, Debug)]
pub struct ClientRqInfo {
    //The UNIQUE digest of the request in question
    pub digest: Digest,

    // The sender, sequence number and session number
    pub sender: NodeId,
    pub seq_no: SeqNo,
    pub session: SeqNo,
}

pub type StoredRequestMessage<O> = StoredMessage<RequestMessage<O>>;

/// Represents a request from a client.
///
/// The `O` type argument symbolizes the client operation to be performed
/// over the replicated state.
#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct RequestMessage<O> {
    session_id: SeqNo,
    operation_id: SeqNo,
    operation: O,
}

/// Represents a reply to a client.
///
/// The `P` type argument symbolizes the response payload.
#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct ReplyMessage<P> {
    session_id: SeqNo,
    operation_id: SeqNo,
    payload: P,
}

impl<O> Orderable for RequestMessage<O> {
    fn sequence_number(&self) -> SeqNo {
        self.operation_id
    }
}

impl<O> RequestMessage<O> {
    /// Creates a new `RequestMessage`.
    pub fn new(sess: SeqNo, id: SeqNo, operation: O) -> Self {
        Self { operation, operation_id: id, session_id: sess }
    }

    /// Returns a reference to the operation of type `O`.
    pub fn operation(&self) -> &O {
        &self.operation
    }

    pub fn session_id(&self) -> SeqNo {
        self.session_id
    }

    /// Unwraps this `RequestMessage`.
    pub fn into_inner_operation(self) -> O {
        self.operation
    }
}

impl<P> Orderable for ReplyMessage<P> {
    fn sequence_number(&self) -> SeqNo {
        self.operation_id
    }
}

impl<P> ReplyMessage<P> {
    /// Creates a new `ReplyMessage`.
    pub fn new(sess: SeqNo, id: SeqNo, payload: P) -> Self {
        Self { payload, operation_id: id, session_id: sess }
    }

    /// Returns a reference to the payload of type `P`.
    pub fn payload(&self) -> &P {
        &self.payload
    }

    pub fn session_id(&self) -> SeqNo {
        self.session_id
    }

    /// Unwraps this `ReplyMessage`.
    pub fn into_inner(self) -> (SeqNo, SeqNo, P) {
        (self.session_id, self.operation_id, self.payload)
    }
}

#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct Protocol<P> {
    payload: P,
}

impl<P> Protocol<P> {
    pub fn new(payload: P) -> Self {
        Self { payload }
    }

    pub fn payload(&self) -> &P { &self.payload }

    pub fn into_inner(self) -> P {
        self.payload
    }
}

impl<P> Deref for Protocol<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}

///
/// State transfer messages
///
#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct StateTransfer<P> {
    payload: P,
}

impl<P> StateTransfer<P> {
    pub fn new(payload: P) -> Self {
        Self { payload }
    }

    pub fn payload(&self) -> &P { &self.payload }

    pub fn into_inner(self) -> P {
        self.payload
    }
}

impl<P> Deref for StateTransfer<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}

///
/// Log transfer messages
///
#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct LogTransfer<P> {
    payload: P,
}

impl<P> LogTransfer<P> {
    pub fn new(payload: P) -> Self {
        Self { payload }
    }

    pub fn payload(&self) -> &P { &self.payload }

    pub fn into_inner(self) -> P {
        self.payload
    }
}

impl<P> Deref for LogTransfer<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}

///
/// View Transfer messages
///
#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct VTMessage<VT> {
    payload: VT,
}

impl<VT> VTMessage<VT> {
    pub fn new(payload: VT) -> Self {
        Self { payload }
    }

    pub fn payload(&self) -> &VT { &self.payload }

    pub fn into_inner(self) -> VT {
        self.payload
    }
}

impl<VT> Deref for VTMessage<VT> {
    type Target = VT;

    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}


///
/// A message containing a number of forwarded requests
///
#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct ForwardedRequestsMessage<O> {
    inner: Vec<StoredRequestMessage<O>>,
}

impl<O> ForwardedRequestsMessage<O> {
    /// Creates a new `ForwardedRequestsMessage`, containing the given client requests.
    pub fn new(inner: Vec<StoredRequestMessage<O>>) -> Self {
        Self { inner }
    }

    pub fn requests(&self) -> &Vec<StoredRequestMessage<O>> { &self.inner }

    pub fn mut_requests(&mut self) -> &mut Vec<StoredRequestMessage<O>> { &mut self.inner }

    /// Returns the client requests contained in this `ForwardedRequestsMessage`.
    pub fn into_inner(self) -> Vec<StoredRequestMessage<O>> {
        self.inner
    }
}

/// A message containing a single forwarded consensus message
#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct ForwardedProtocolMessage<P> where {
    message: StoredMessage<Protocol<P>>,
}

impl<P> Deref for ForwardedProtocolMessage<P> {
    type Target = StoredMessage<Protocol<P>>;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}

impl<P> ForwardedProtocolMessage<P> {
    pub fn new(message: StoredMessage<Protocol<P>>) -> Self {
        Self { message }
    }

    pub fn message(&self) -> &StoredMessage<Protocol<P>> { &self.message }

    pub fn into_inner(self) -> StoredMessage<Protocol<P>> {
        self.message
    }
}

impl<O> Debug for RequestMessage<O> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Session: {:?} Seq No: {:?}", self.session_id, self.sequence_number())
    }
}

impl Orderable for ClientRqInfo {
    fn sequence_number(&self) -> SeqNo {
        self.seq_no
    }
}

impl ClientRqInfo {
    pub fn new(digest: Digest, sender: NodeId, seqno: SeqNo, session: SeqNo) -> Self {
        Self {
            digest,
            sender,
            seq_no: seqno,
            session,
        }
    }

    pub fn digest(&self) -> Digest {
        self.digest
    }

    pub fn sender(&self) -> NodeId {
        self.sender
    }

    pub fn session(&self) -> SeqNo {
        self.session
    }
}

impl<O> From<&StoredRequestMessage<O>> for ClientRqInfo {
    fn from(message: &StoredRequestMessage<O>) -> Self {
        let digest = message.header().unique_digest();
        let sender = message.header().from();

        let session = message.message().session_id();
        let seq_no = message.message().sequence_number();

        Self {
            digest,
            sender,
            seq_no,
            session,
        }
    }
}

impl Hash for ClientRqInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.digest.hash(state);
    }
}

impl<P> Debug for Protocol<P> where P: Debug {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.payload)
    }
}
