use std::sync::Arc;

use atlas_common::error::*;
use atlas_communication::message::Header;
use atlas_communication::reconfiguration::NetworkInformationProvider;

use crate::ordering_protocol::networking::serialize::OrderingProtocolMessage;
