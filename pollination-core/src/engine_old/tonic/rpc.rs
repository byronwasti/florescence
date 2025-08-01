use serde::{Deserialize, Serialize};

// TODO: AFAICT Tonic doesn't support generic RPCs
// so this is a gross workaround for now.
#[derive(Debug, Deserialize, Serialize)]
pub struct TonicReqWrapper {
    pub raw: Vec<u8>,
}

include!(concat!(env!("OUT_DIR"), "/bincode.gossip.Gossip.rs"));
