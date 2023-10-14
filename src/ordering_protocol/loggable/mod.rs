use std::fmt::Debug;
use std::sync::Arc;
#[cfg(feature = "serialize_serde")]
use ::serde::{Deserialize, Serialize};
use atlas_common::ordering::{Orderable, SeqNo};
use atlas_common::error::*;
use atlas_communication::message::StoredMessage;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;
use atlas_smr_application::app::UpdateBatch;
use atlas_smr_application::serialize::ApplicationData;
use crate::messages::{ClientRqInfo, StoredRequestMessage};
use crate::ordering_protocol::networking::serialize::{OrderingProtocolMessage, OrderProtocolProof};
use crate::ordering_protocol::networking::signature_ver::OrderProtocolSignatureVerificationHelper;
use crate::ordering_protocol::{DecisionMetadata, OrderingProtocol, ProtocolConsensusDecision, ProtocolMessage};
use crate::smr::smr_decision_log::ShareableConsensusMessage;

/// The trait definining the necessary data types for the ordering protocol to be used
/// with the decision log
pub trait PersistentOrderProtocolTypes<D, OPM> {
    /// A proof of a given Sequence number in the consensus protocol
    /// This is used as the type to fully represent the validity of a given SeqNo in the protocol
    /// A proof with SeqNo X should mean that X has been decided correctly
    /// This should be composed of some metadata and a set of LoggableMessages
    #[cfg(feature = "serialize_capnp")]
    type Proof: OrderProtocolProof + Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type Proof: OrderProtocolProof + for<'a> Deserialize<'a> + Serialize + Send + Clone;

    /// Verify the validity of the given proof
    fn verify_proof<NI, OPVH>(network_info: &Arc<NI>,
                              proof: Self::Proof) -> Result<(bool, Self::Proof)>
        where NI: NetworkInformationProvider,
              D: ApplicationData,
              OPM: OrderingProtocolMessage<D>,
              OPVH: OrderProtocolSignatureVerificationHelper<D, OPM, NI>, Self: Sized;
}

/// A trait to create a separation between these helper methods and the rest
/// of the order protocol so that we don't require generics that are not needed
pub trait OrderProtocolPersistenceHelper<D, OPM, POP> where D: ApplicationData,
                                                            OPM: OrderingProtocolMessage<D>,
                                                            POP: PersistentOrderProtocolTypes<D, OPM> {
    /// The types of messages to be stored. This is used due to the parallelization described above.
    /// Each of the names provided here will be a different KV-DB instance (in the case of RocksDB, a column family)
    fn message_types() -> Vec<&'static str>;

    /// Get the message type for a given message, must correspond to a string returned by
    /// [PersistableOrderProtocol::message_types]
    fn get_type_for_message(msg: &ProtocolMessage<D, OPM>) -> Result<&'static str>;

    /// Initialize a proof from the metadata and messages stored in persistent storage
    fn init_proof_from(metadata: DecisionMetadata<D, OPM>,
                       messages: Vec<StoredMessage<ProtocolMessage<D, OPM>>>)
                       -> PProof<D, OPM, POP>;

    /// Initialize a proof from the metadata and messages stored by the decision log
    fn init_proof_from_scm(metadata: DecisionMetadata<D, OPM>,
                           messages: Vec<ShareableConsensusMessage<D, OPM>>)
                           -> PProof<D, OPM, POP>;

    /// Decompose a given proof into it's metadata and messages, ready to be persisted
    fn decompose_proof(proof: &PProof<D, OPM, POP>)
                       -> (&DecisionMetadata<D, OPM>, Vec<&StoredMessage<ProtocolMessage<D, OPM>>>);

    /// Extract the proof out of the protocol decision proof
    fn get_requests_in_proof(proof: &PProof<D, OPM, POP>)
                             -> Result<ProtocolConsensusDecision<D::Request>>;
}

pub type PProof<D, OP, POP> = <POP as PersistentOrderProtocolTypes<D, OP>>::Proof;

/// The trait to define the necessary methods and data types for this order protocol
/// to be compatible with the decision log
pub trait LoggableOrderProtocol<D, NT>: OrderingProtocol<D, NT> +
OrderProtocolPersistenceHelper<D, Self::Serialization, Self::PersistableTypes>
    where D: ApplicationData + 'static {

    /// The required data types for working with the decision log
    type PersistableTypes: PersistentOrderProtocolTypes<D, Self::Serialization>;
}