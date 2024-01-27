use std::sync::Arc;

#[cfg(feature = "serialize_serde")]
use ::serde::Serialize;

use atlas_common::error::*;
use atlas_common::serialization_helper::SerType;
use atlas_communication::message::StoredMessage;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;

use crate::ordering_protocol::{DecisionMetadata, OrderingProtocol, ProtocolConsensusDecision, ProtocolMessage, ShareableConsensusMessage};
use crate::ordering_protocol::networking::serialize::{OrderingProtocolMessage, OrderProtocolProof, OrderProtocolVerificationHelper};

/// The trait definining the necessary data types for the ordering protocol to be used
/// with the decision log
pub trait PersistentOrderProtocolTypes<RQ, OPM>: Send + Sync + 'static {
    /// A proof of a given Sequence number in the consensus protocol
    /// This is used as the type to fully represent the validity of a given SeqNo in the protocol
    /// A proof with SeqNo X should mean that X has been decided correctly
    /// This should be composed of some metadata and a set of LoggableMessages
    type Proof: OrderProtocolProof + SerType + 'static;

    /// Verify the validity of the given proof
    fn verify_proof<NI, OPVH>(network_info: &Arc<NI>,
                              proof: Self::Proof) -> Result<Self::Proof>
        where NI: NetworkInformationProvider,
              OPM: OrderingProtocolMessage<RQ>,
              OPVH: OrderProtocolVerificationHelper<RQ, OPM, NI>, Self: Sized;
}

/// A trait to create a separation between these helper methods and the rest
/// of the order protocol so that we don't require generics that are not needed
pub trait OrderProtocolPersistenceHelper<RQ, OPM, POP>: Send
    where RQ: SerType,
          OPM: OrderingProtocolMessage<RQ>,
          POP: PersistentOrderProtocolTypes<RQ, OPM> {
    /// The types of messages to be stored. This is used due to the parallelization described above.
    /// Each of the names provided here will be a different KV-DB instance (in the case of RocksDB, a column family)
    fn message_types() -> Vec<&'static str>;

    /// Get the message type for a given message, must correspond to a string returned by
    /// [PersistableOrderProtocol::message_types]
    fn get_type_for_message(msg: &ProtocolMessage<RQ, OPM>) -> Result<&'static str>;

    /// Initialize a proof from the metadata and messages stored in persistent storage
    fn init_proof_from(metadata: DecisionMetadata<RQ, OPM>,
                       messages: Vec<StoredMessage<ProtocolMessage<RQ, OPM>>>)
                       -> Result<PProof<RQ, OPM, POP>>;

    /// Initialize a proof from the metadata and messages stored by the decision log
    fn init_proof_from_scm(metadata: DecisionMetadata<RQ, OPM>,
                           messages: Vec<ShareableConsensusMessage<RQ, OPM>>)
                           -> Result<PProof<RQ, OPM, POP>>;

    /// Decompose a given proof into it's metadata and messages, ready to be persisted
    fn decompose_proof(proof: &PProof<RQ, OPM, POP>)
                       -> (&DecisionMetadata<RQ, OPM>, Vec<&StoredMessage<ProtocolMessage<RQ, OPM>>>);

    /// Extract the proof out of the protocol decision proof
    fn get_requests_in_proof(proof: &PProof<RQ, OPM, POP>)
                             -> Result<ProtocolConsensusDecision<RQ>>;
}

pub type PProof<RQ, OP, POP> = <POP as PersistentOrderProtocolTypes<RQ, OP>>::Proof;

/// The trait to define the necessary methods and data types for this order protocol
/// to be compatible with the decision log
pub trait LoggableOrderProtocol<RQ, NT>: OrderingProtocol<RQ, NT>
+ OrderProtocolPersistenceHelper<RQ, Self::Serialization, Self::PersistableTypes>
    where RQ: SerType, {
    /// The required data types for working with the decision log
    type PersistableTypes: PersistentOrderProtocolTypes<RQ, Self::Serialization> + 'static;
}