use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GossipRequest {
    pub timestamp: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GossipResponse {
    pub timestamp: String,
}

include!(concat!(env!("OUT_DIR"), "/bincode.gossip.Gossip.rs"));

