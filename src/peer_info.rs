use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PeerInfo<A> {
    pub addr: A,
    pub status: PeerStatus,
    // topics: Vec<Topic>,
}

impl<A> PeerInfo<A> {
    pub(crate) fn new(addr: A) -> Self {
        Self {
            addr,
            status: PeerStatus::Healthy,
        }
    }
}

impl<A: fmt::Display> fmt::Display for PeerInfo<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{{{}, {}}}", self.status, self.addr)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerStatus {
    Healthy,
    Dead,
}

impl fmt::Display for PeerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        use PeerStatus::*;
        write!(f, "{}", match self {
            Healthy => "Healthy",
            Dead => "Dead",
        })
    }
}
