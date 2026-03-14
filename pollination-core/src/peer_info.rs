use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo<A> {
    pub uuid: Uuid,
    pub status: PeerStatus,
    pub addr: A,
}

impl<A> PeerInfo<A> {
    pub(crate) fn new(uuid: Uuid, addr: A) -> Self {
        Self {
            addr,
            status: PeerStatus::Healthy,
            uuid,
        }
    }
}

impl<A: fmt::Display> fmt::Display for PeerInfo<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "UUID:{}, STATUS:{}, ADDR:{}",
            self.uuid, self.status, self.addr,
        )
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
        write!(
            f,
            "{}",
            match self {
                Healthy => "Healthy",
                Dead => "Dead",
            }
        )
    }
}
