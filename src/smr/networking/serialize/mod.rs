use std::sync::Arc;
use atlas_common::ordering::{Orderable, SeqNo};

use serde::{Deserialize, Serialize};
use atlas_communication::reconfiguration_node::NetworkInformationProvider;
use atlas_smr_application::serialize::ApplicationData;
use atlas_common::error::*;
use crate::ordering_protocol::networking::serialize::OrderingProtocolMessage;
use crate::ordering_protocol::networking::signature_ver::OrderProtocolSignatureVerificationHelper;
use crate::ordering_protocol::loggable::PersistentOrderProtocolTypes;

pub trait OrderProtocolLog: Orderable {
    // At the moment I only need orderable, but I might need more in the future
    fn first_seq(&self) -> Option<SeqNo>;
}

/// A trait defining what we need in order to verify parts of the decision log
pub trait OrderProtocolLogPart: Orderable {
    // We only need to add the first sequence number, since we already know the last
    // From the orderable implementation
    fn first_seq(&self) -> Option<SeqNo>;
}

pub trait DecisionLogMessage<D, OPM, POP>: Send + Sync {
    /// A metadata type to allow for decision logs to include some
    /// more specific information into their decision log, apart from
    /// the list of proofs
    #[cfg(feature = "serialize_capnp")]
    type DecLogMetadata: Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type DecLogMetadata: for<'a> Deserialize<'a> + Serialize + Send + Clone;

    /// A type that defines the log of decisions made since the last garbage collection
    /// (In the case of BFT SMR the log is GCed after a checkpoint of the application)
    #[cfg(feature = "serialize_capnp")]
    type DecLog: OrderProtocolLog + Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type DecLog: OrderProtocolLog + for<'a> Deserialize<'a> + Serialize + Send + Clone;

    /// A type that defines the log of decisions made since the last garbage collection
    /// (In the case of BFT SMR the log is GCed after a checkpoint of the application)
    #[cfg(feature = "serialize_capnp")]
    type DecLogPart: OrderProtocolLogPart + Send + Clone;

    #[cfg(feature = "serialize_serde")]
    type DecLogPart: OrderProtocolLogPart + for<'a> Deserialize<'a> + Serialize + Send + Clone;

    fn verify_decision_log<NI, OPVH>(network_info: &Arc<NI>, dec_log: Self::DecLog)
                                     -> Result<Self::DecLog>
        where NI: NetworkInformationProvider,
              D: ApplicationData,
              OPM: OrderingProtocolMessage<D>,
              POP: PersistentOrderProtocolTypes<D, OPM>,
              OPVH: OrderProtocolSignatureVerificationHelper<D, OPM, NI>,;
}