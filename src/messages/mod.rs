use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;

#[cfg(feature = "serialize_serde")]
use serde::{Deserialize, Serialize};

use atlas_common::crypto::hash::Digest;
use atlas_common::node_id::NodeId;
use atlas_common::ordering::{Orderable, SeqNo};
use atlas_communication::message::StoredMessage;

use crate::timeouts::TimedOut;

pub type StoredRequestMessage<O> = StoredMessage<RequestMessage<O>>;

/// A trait that indicates that the requests in question are separated into sessions
pub trait SessionBased: Orderable {
    /// Obtain the session number of the object
    fn session_number(&self) -> SeqNo;
}

/// Represents a request from a client.
///
/// The `O` type argument symbolizes the client operation to be performed
/// over the replicated state.
///
/// This type encapsulates the session id and the operation id, used
/// by the clients to identify the request (and respective reply).
///
/// Follows [SessionBased]
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
///
/// This type encapsulates the session id and the operation id, used
/// by the clients to identify the request (and respective reply).
///
/// Follows [SessionBased]
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
        Self {
            operation,
            operation_id: id,
            session_id: sess,
        }
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

impl<O> Debug for RequestMessage<O> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Session: {:?} Seq No: {:?}",
            self.session_id,
            self.sequence_number()
        )
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
        Self {
            payload,
            operation_id: id,
            session_id: sess,
        }
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

/// The client request information about a given request
#[derive(Eq, PartialEq, Ord, Clone, PartialOrd, Debug)]
pub struct ClientRqInfo {
    //The UNIQUE digest of the request in question
    pub digest: Digest,

    // The sender, sequence number and session number
    pub sender: NodeId,
    pub seq_no: SeqNo,
    pub session: SeqNo,
}

/// A wrapper for protocol messages
#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct Protocol<P> {
    payload: P,
}

impl<P> Protocol<P> {
    pub fn new(payload: P) -> Self {
        Self { payload }
    }

    pub fn payload(&self) -> &P {
        &self.payload
    }

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

    pub fn payload(&self) -> &VT {
        &self.payload
    }

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
    inner: Vec<StoredMessage<O>>,
}

impl<O> ForwardedRequestsMessage<O> {
    /// Creates a new `ForwardedRequestsMessage`, containing the given client requests.
    pub fn new(inner: Vec<StoredMessage<O>>) -> Self {
        Self { inner }
    }

    pub fn requests(&self) -> &Vec<StoredMessage<O>> {
        &self.inner
    }

    pub fn mut_requests(&mut self) -> &mut Vec<StoredMessage<O>> {
        &mut self.inner
    }

    /// Returns the client requests contained in this `ForwardedRequestsMessage`.
    pub fn into_inner(self) -> Vec<StoredMessage<O>> {
        self.inner
    }
}

/// A message containing a single forwarded consensus message
#[cfg_attr(feature = "serialize_serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct ForwardedProtocolMessage<P> {
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

    pub fn message(&self) -> &StoredMessage<Protocol<P>> {
        &self.message
    }

    pub fn into_inner(self) -> StoredMessage<Protocol<P>> {
        self.message
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

impl<O> From<&StoredMessage<O>> for ClientRqInfo
where
    O: SessionBased,
{
    fn from(message: &StoredMessage<O>) -> Self {
        let digest = message.header().unique_digest();
        let sender = message.header().from();

        let session = message.message().session_number();
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

impl<P> Debug for Protocol<P>
where
    P: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.payload)
    }
}

impl SessionBased for ClientRqInfo {
    fn session_number(&self) -> SeqNo {
        self.session
    }
}

impl<O> SessionBased for RequestMessage<O> {
    fn session_number(&self) -> SeqNo {
        self.session_id
    }
}

impl<RP> SessionBased for ReplyMessage<RP> {
    fn session_number(&self) -> SeqNo {
        self.session_id
    }
}
