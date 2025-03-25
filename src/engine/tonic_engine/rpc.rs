use serde::{Deserialize, Serialize};
use treeclocks::EventTree;
use crate::pollinator::RealityToken;
use crate::message::PollinationMessage;

#[derive(Debug, Deserialize, Serialize)]
pub struct TonicReqWrapper {
    pub raw: Vec<u8>,
}

include!(concat!(env!("OUT_DIR"), "/bincode.gossip.Gossip.rs"));
