use std::sync::Arc;

use atlas_common::channel::{ChannelSyncRx, ChannelSyncTx};
use atlas_common::error::*;
use atlas_smr_application::state::divisible_state::{DivisibleState, InstallStateMessage};
use crate::ordering_protocol::networking::serialize::NetworkView;

use crate::persistent_log::DivisibleStateLog;
use crate::state_transfer::StateTransferProtocol;
use crate::timeouts::Timeouts;

pub trait DivisibleStateTransfer<S, NT, PL>: StateTransferProtocol<S, NT, PL>
    where S: DivisibleState + 'static,
          PL: DivisibleStateLog<S> {
    /// The configuration type the state transfer protocol wants to accept
    type Config;

    /// Initialize the state transferring protocol with the given configuration, timeouts and communication layer
    fn initialize(config: Self::Config, timeouts: Timeouts, node: Arc<NT>, log: PL,
                  executor_state_handle: ChannelSyncTx<InstallStateMessage<S>>) -> Result<Self>
        where Self: Sized;

    /// Handle having received a state from the application
    /// you should also notify the ordering protocol that the state has been received
    /// and processed, so he is now safe to delete the state (Maybe this should be handled by the replica?)
    fn handle_state_desc_received_from_app<V>(&mut self, view: V, descriptor: S::StateDescriptor)
                                              -> Result<()> where V: NetworkView;


    fn handle_state_part_received_from_app<V>(&mut self, view: V, parts: Vec<S::StatePart>)
                                              -> Result<()> where V: NetworkView;

    fn handle_state_finished_reception<V>(&mut self, view: V)
                                          -> Result<()> where V: NetworkView;

}