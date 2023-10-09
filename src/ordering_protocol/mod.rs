use std::fmt::{Debug, Formatter};
use std::iter;
use std::sync::Arc;

use atlas_common::crypto::hash::Digest;
use atlas_common::error::*;
use atlas_common::maybe_vec::MaybeVec;
use atlas_common::node_id::NodeId;
use atlas_common::ordering::{Orderable, SeqNo};
use atlas_communication::message::StoredMessage;
use atlas_smr_application::app::UpdateBatch;
use atlas_smr_application::ExecutorHandle;
use atlas_smr_application::serialize::ApplicationData;

use crate::messages::{ClientRqInfo, Protocol};
use crate::ordering_protocol::networking::serialize::{OrderingProtocolMessage, PermissionedOrderingProtocolMessage};
use crate::persistent_log::OrderingProtocolLog;
use crate::request_pre_processing::{BatchOutput, RequestPreProcessor};
use crate::smr::smr_decision_log::{ShareableConsensusMessage, ShareableMessage};
use crate::timeouts::{RqTimeout, Timeouts};

pub mod reconfigurable_order_protocol;
pub mod loggable;
pub mod networking;

pub type View<POP: PermissionedOrderingProtocolMessage> = <POP as PermissionedOrderingProtocolMessage>::ViewInfo;

pub type ProtocolMessage<D, OP> = <OP as OrderingProtocolMessage<D>>::ProtocolMessage;
pub type DecisionMetadata<D, OP> = <OP as OrderingProtocolMessage<D>>::ProofMetadata;

pub struct OrderingProtocolArgs<D, NT>(pub ExecutorHandle<D>, pub Timeouts,
                                       pub RequestPreProcessor<D::Request>,
                                       pub BatchOutput<D::Request>, pub Arc<NT>,
                                       pub Vec<NodeId>) where D: ApplicationData;

pub trait OrderProtocolTolerance {
    fn get_n_for_f(f: usize) -> usize;
}

/// The trait for an ordering protocol to be implemented in Atlas
pub trait OrderingProtocol<D, NT>: OrderProtocolTolerance + Orderable where D: ApplicationData + 'static {
    /// The type which implements OrderingProtocolMessage, to be implemented by the developer
    type Serialization: OrderingProtocolMessage<D> + 'static;

    /// The configuration type the protocol wants to accept
    type Config;

    /// Initialize this ordering protocol with the given configuration, executor, timeouts and node
    fn initialize(config: Self::Config, args: OrderingProtocolArgs<D, NT>) -> Result<Self> where
        Self: Sized;

    /// Handle a protocol message that was received while we are executing another protocol
    fn handle_off_ctx_message(&mut self, message: ShareableConsensusMessage<D, Self::Serialization>);

    /// Handle the protocol being executed having changed (for example to the state transfer protocol)
    /// This is important for some of the protocols, which need to know when they are being executed or not
    fn handle_execution_changed(&mut self, is_executing: bool) -> Result<()>;

    /// Poll from the ordering protocol in order to know what we should do next
    /// We do this to check if there are already messages waiting to be executed that were received ahead of time and stored.
    /// Or whether we should run state transfer or wait for messages from other replicas
    fn poll(&mut self) -> OPPollResult<DecisionMetadata<D, Self::Serialization>,
        ProtocolMessage<D, Self::Serialization>, D::Request>;

    /// Process a protocol message that we have received
    /// This can be a message received from the poll() method or a message received from other replicas.
    fn process_message(&mut self, message: ShareableConsensusMessage<D, Self::Serialization>)
                       -> Result<OPExecResult<DecisionMetadata<D, Self::Serialization>, ProtocolMessage<D, Self::Serialization>, D::Request>>;

    /// Install a given sequence number
    fn install_seq_no(&mut self, seq_no: SeqNo) -> Result<()>;

    /// Handle a timeout received from the timeouts layer
    fn handle_timeout(&mut self, timeout: Vec<RqTimeout>) -> Result<OPExecResult<DecisionMetadata<D, Self::Serialization>, ProtocolMessage<D, Self::Serialization>, D::Request>>;
}


/// A permissioned ordering protocol, meaning only a select few are actually part of the quorum that decides the
/// ordering of the operations.
pub trait PermissionedOrderingProtocol {
    type PermissionedSerialization: PermissionedOrderingProtocolMessage + 'static;

    /// Get the current view of the ordering protocol
    fn view(&self) -> View<Self::PermissionedSerialization>;

    /// Install a given view into the ordering protocol
    fn install_view(&mut self, view: View<Self::PermissionedSerialization>);
}


#[derive(Debug)]
/// A given decision and information about it
/// To be taken by the replica and processed accordingly
pub struct Decision<MD, P, O> {
    // The seq no of the decision
    seq: SeqNo,
    // Information about the decision progression.
    // Multiple instances can be bundled here in case we want to include various updates to
    // The same decision
    decision_info: MaybeVec<DecisionInfo<MD, P, O>>,
}

#[derive(Debug, PartialOrd, Ord)]
/// Information about a given decision
pub enum DecisionInfo<MD, P, O> {
    // The decision metadata, does not indicate that the decision is made
    DecisionMetadata(MD),
    // Partial information about the decision (composing messages)
    PartialDecisionInformation(MaybeVec<ShareableMessage<P>>),
    // The decision has been completed
    DecisionDone(ProtocolConsensusDecision<O>),
}

#[derive(Debug)]
/// The information about a node having joined the quorum
pub struct JoinInfo {
    node: NodeId,
    new_quorum: Vec<NodeId>,
}

/// The return enum of polling the ordering protocol
#[derive(Debug)]
pub enum OPPollResult<MD, P, D> {
    RunCst,
    ReceiveMsg,
    Exec(ShareableMessage<P>),
    Decided(MaybeVec<Decision<MD, P, D>>),
    QuorumJoined(Option<MaybeVec<Decision<MD, P, D>>>, JoinInfo),
    RePoll,
}

#[derive(Debug)]
/// The result of the ordering protocol executing a message
pub enum OPExecResult<MD, P, D> {
    /// The message we have passed onto the order protocol was dropped
    MessageDropped,
    /// The message we have passed onto the order protocol was queued for later use
    MessageQueued,
    /// The input was processed but there are no new updates to take from it
    MessageProcessedNoUpdate,
    /// The given decisions have been progressed (containing the progress information)
    ProgressedDecision(MaybeVec<Decision<MD, P, D>>),
    /// The given decisions have been decided
    Decided(MaybeVec<Decision<MD, P, D>>),
    /// The quorum has been joined by a given node
    QuorumJoined(Option<MaybeVec<Decision<MD, P, D>>>, JoinInfo),
    /// The order protocol requires the protocol to update its state to be inline with
    /// the rest of replicas in the system
    RunCst,
    //TODO: clear the deciding log from a seq number onwards in order to handle
    // view changes or such protocols
}

/// Information reported after a logging operation.
pub enum ExecutionResult {
    /// Nothing to report.
    Nil,
    /// The log became full. We are waiting for the execution layer
    /// to provide the current serialized application state, so we can
    /// complete the log's garbage collection and eventually its
    /// checkpoint.
    BeginCheckpoint,
}

/// The struct representing a consensus decision
///
/// Executable Batch: All of the requests that should be executed, in the correct order
/// Execution Result: Whether we need to ask the executor for a checkpoint in order to reset the current message log
/// Batch info: The information collected by the [DecidingLog], if applicable. (We can receive a batch
/// via a complete proof which means this will be [None] or we can process a batch normally, which means
/// this will be [Some(CompletedBatch<D>)])
pub struct ProtocolConsensusDecision<O> {
    seq: SeqNo,
    // The digest of the batch
    batch_digest: Digest,
    // The client requests information contained in the batch.
    contained_requests: Vec<ClientRqInfo>,
    // The batch of client requests to execute as a result of this protocol
    executable_batch: UpdateBatch<O>,
}

impl<MD, P, O> Decision<MD, P, O> {
    /// Create a decision information object from a stored message
    pub fn decision_info_from_message(seq: SeqNo, decision: ShareableMessage<P>) -> Self {
        Decision {
            seq,
            decision_info: MaybeVec::One(DecisionInfo::PartialDecisionInformation(MaybeVec::One(decision))),
        }
    }

    /// Create a decision information object from a metadata object
    pub fn decision_info_from_metadata(seq: SeqNo, metadata: MD) -> Self {
        Decision {
            seq,
            decision_info: MaybeVec::One(DecisionInfo::DecisionMetadata(metadata)),
        }
    }

    /// Create a decision information object from a group of messages
    pub fn decision_info_from_messages(seq: SeqNo, messages: Vec<ShareableMessage<P>>) -> Self {
        Decision {
            seq,
            decision_info: MaybeVec::One(DecisionInfo::PartialDecisionInformation(MaybeVec::Vec(messages))),
        }
    }

    /// Create a decision done object
    pub fn completed_decision(seq: SeqNo, update: ProtocolConsensusDecision<O>) -> Self {
        Decision {
            seq,
            decision_info: MaybeVec::One(DecisionInfo::DecisionDone(update)),
        }
    }

    /// Create a full decision info, from all of the components
    pub fn full_decision_info(seq: SeqNo, metadata: MD,
                              messages: Vec<ShareableMessage<P>>,
                              requests: ProtocolConsensusDecision<O>) -> Self {
        let mut decision_info = Vec::with_capacity(3);

        decision_info.push(DecisionInfo::DecisionMetadata(metadata));
        decision_info.push(DecisionInfo::PartialDecisionInformation(MaybeVec::Vec(messages)));
        decision_info.push(DecisionInfo::DecisionDone(requests));

        Decision {
            seq,
            decision_info: MaybeVec::Vec(decision_info),
        }
    }

    pub fn decision_info(&self) -> &MaybeVec<DecisionInfo<MD, P, O>> {
        &self.decision_info
    }

    pub fn into_decision_info(self) -> MaybeVec<DecisionInfo<MD, P, O>> {
        self.decision_info
    }
}

impl<MD, P, O> Orderable for Decision<MD, P, O> {
    fn sequence_number(&self) -> SeqNo {
        self.seq
    }
}

impl<MD, P, O> DecisionInfo<MD, P, O> {
    
    pub fn decision_info_from_message(message: MaybeVec<ShareableMessage<P>>) -> MaybeVec<Self> {
        MaybeVec::from_one(Self::PartialDecisionInformation(message))
    }
    
    pub fn decision_info_from_message_and_metadata(message: MaybeVec<ShareableMessage<P>>, metadata: MD) -> MaybeVec<Self> {
        let partial = Self::PartialDecisionInformation(message);
        let metadata = Self::DecisionMetadata(metadata);
        
        MaybeVec::Vec(vec![partial, metadata])
    }
    
}

impl JoinInfo {
    pub fn new(joined: NodeId, current_quorum: Vec<NodeId>) -> Self{
        Self {
            node: joined,
            new_quorum: current_quorum,
        }
    }

    pub fn into_inner(self) -> (NodeId, Vec<NodeId>) {
        (self.node, self.new_quorum)
    }
}

impl<MD, P, D> Orderable for Decision<MD, P, D> {
    fn sequence_number(&self) -> SeqNo {
        self.seq
    }
}

impl<MD, P, D> Debug for OPPollResult<MD, P, D> where P: Debug {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            OPPollResult::RunCst => {
                write!(f, "RunCst")
            }
            OPPollResult::ReceiveMsg => {
                write!(f, "Receive From Replicas")
            }
            OPPollResult::Exec(message) => {
                write!(f, "Exec message {:?}", message.message())
            }
            OPPollResult::RePoll => {
                write!(f, "RePoll")
            }
            OPPollResult::Decided(rqs) => {
                write!(f, "{} committed decisions", rqs.len())
            }
            OPPollResult::QuorumJoined(decs, node) => {
                write!(f, "Join information: {:?}. Contained Decisions {}", node, decs.map(|dec| dec.len()).unwrap_or(0))
            }
        }
    }
}

/// Constructor for the ProtocolConsensusDecision struct
impl<O> ProtocolConsensusDecision<O> {
    pub fn new(seq: SeqNo,
               executable_batch: UpdateBatch<O>,
               client_rqs: Vec<ClientRqInfo>,
               batch_digest: Digest) -> Self {
        ProtocolConsensusDecision {
            seq,
            batch_digest,
            contained_requests: client_rqs,
            executable_batch,
        }
    }

    pub fn into(self) -> (SeqNo, UpdateBatch<O>, Vec<ClientRqInfo>, Digest) {
        (self.seq, self.executable_batch, self.contained_requests, self.batch_digest)
    }

    pub fn update_batch(&self) -> &UpdateBatch<O> {
        &self.executable_batch
    }
}

impl<O> Orderable for ProtocolConsensusDecision<O> {
    fn sequence_number(&self) -> SeqNo {
        self.seq
    }
}

impl<O> Debug for ProtocolConsensusDecision<O> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProtocolConsensusDecision {{ seq: {:?}, executable_batch: {:?}, batch_info: {:?} }}", self.seq, self.executable_batch.len(), self.batch_digest)
    }
}