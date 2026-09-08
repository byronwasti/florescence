use crate::NodeIndex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config<C> {
    pub node_count: usize,
    pub seed: u64,
    pub message_queue_size: usize,
    pub custom: C,
}
