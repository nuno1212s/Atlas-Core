use std::fmt::Debug;
use std::sync::Arc;
#[cfg(feature = "serialize_serde")]
use ::serde::{Deserialize, Serialize};
use atlas_common::ordering::Orderable;
use atlas_common::error::*;
use atlas_communication::message::StoredMessage;
use atlas_communication::reconfiguration_node::NetworkInformationProvider;
use atlas_smr_application::serialize::ApplicationData;
use crate::ordering_protocol::networking::serialize::{OrderingProtocolMessage, OrderProtocolProof};
use crate::ordering_protocol::networking::signature_ver::OrderProtocolSignatureVerificationHelper;

pub trait PersistentOrderProtocolTypes<D, OPM> {
    /// A shortcut type to messages that are going to be logged. (this is useful for situations
    /// where we don't log all message types that we send)
    #[cfg(feature = "serialize_capnp")]
    type LoggableMessage: Orderable + Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type LoggableMessage: Orderable + for<'a> Deserialize<'a> + Serialize + Send + Clone + Debug;

    /// The metadata type for storing the proof in the persistent storage
    /// Since the message will be stored in the persistent storage, it needs to be serializable
    /// This should provide all the necessary final information to assemble the proof from the messages
    #[cfg(feature = "serialize_capnp")]
    type ProofMetadata: Orderable + Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type ProofMetadata: Orderable + for<'a> Deserialize<'a> + Serialize + Send + Clone;

    /// A proof of a given Sequence number in the consensus protocol
    /// This is used as the type to fully represent the validity of a given SeqNo in the protocol
    /// A proof with SeqNo X should mean that X has been decided correctly
    /// This should be composed of some metadata and a set of LoggableMessages
    #[cfg(feature = "serialize_capnp")]
    type Proof: OrderProtocolProof + Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type Proof: OrderProtocolProof + for<'a> Deserialize<'a> + Serialize + Send + Clone;

    fn verify_proof<NI, OPVH>(network_info: &Arc<NI>,
                              proof: Self::Proof) -> Result<(bool, Self::Proof)>
        where NI: NetworkInformationProvider,
              D: ApplicationData,
              OPM: OrderingProtocolMessage<D>,
              OPVH: OrderProtocolSignatureVerificationHelper<D, OPM, NI>, Self: Sized;
}

type PProof<D, OP, POP> = <POP as PersistentOrderProtocolTypes<D, OP>>::Proof;
type LMessage<D, OP, POP> = <POP as PersistentOrderProtocolTypes<D, OP>>::LoggableMessage;
type PProofMeta<D, OP, POP> = <POP as PersistentOrderProtocolTypes<D, OP>>::ProofMetadata;

pub trait PersistableOrderProtocolV2<D, OPM> where D: ApplicationData,
                                                   OPM: OrderingProtocolMessage<D> {
    type PersistableTypes: PersistentOrderProtocolTypes<D, OPM>;

    /// The types of messages to be stored. This is used due to the parallelization described above.
    /// Each of the names provided here will be a different KV-DB instance (in the case of RocksDB, a column family)
    fn message_types() -> Vec<&'static str>;

    /// Get the message type for a given message, must correspond to a string returned by
    /// [PersistableOrderProtocol::message_types]
    fn get_type_for_message(msg: &LMessage<D, OPM, Self::PersistableTypes>) -> Result<&'static str>;

    /// Initialize a proof from the metadata and messages stored in persistent storage
    fn init_proof_from(metadata: PProofMeta<D, OPM, Self::PersistableTypes>, messages: Vec<StoredMessage<LMessage<D, OPM, Self::PersistableTypes>>>) -> PProof<D, OPM, Self::PersistableTypes>;

    /// Decompose a given proof into it's metadata and messages, ready to be persisted
    fn decompose_proof(proof: &PProof<D, OPM, Self::PersistableTypes>) -> (&PProofMeta<D, OPM, Self::PersistableTypes>, Vec<&StoredMessage<LMessage<D, OPM, Self::PersistableTypes>>>);
}