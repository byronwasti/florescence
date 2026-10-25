use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInfo<A> {
    pub uuid: Uuid,
    pub addr: A,
    pub timestamp: u64,
    pub status: NodeStatus,
}

impl<A> NodeInfo<A> {
    pub fn new(uuid: Uuid, addr: A) -> Self {
        NodeInfo {
            uuid,
            addr,
            timestamp: 1,
            status: NodeStatus::Healthy,
        }
    }
}

impl<A: std::fmt::Debug> std::fmt::Display for NodeInfo<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(
            f,
            "{{ {:?}::{} timestamp={} status={:?} }}",
            self.addr, self.uuid, self.timestamp, self.status
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Healthy,
    Dead,
}
