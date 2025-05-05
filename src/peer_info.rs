use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PeerInfo<A> {
    pub uuid: Uuid,
    pub addr: Option<A>,
    pub status: PeerStatus,
    // topics: Vec<Topic>,
}

impl<A> PeerInfo<A> {
    pub(crate) fn new(uuid: Uuid, addr: A) -> Self {
        Self {
            addr: Some(addr),
            status: PeerStatus::Healthy,
            uuid,
        }
    }

    pub(crate) fn dead() -> Self {
        Self {
            addr: None,
            status: PeerStatus::Dead,
            uuid: Uuid::from_u128(0),
        }
    }
}

impl<A: fmt::Display> fmt::Display for PeerInfo<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{{{}, {}, {}}}",
            self.uuid,
            self.status,
            if let Some(addr) = &self.addr {
                format!("{addr}")
            } else {
                "?".to_string()
            }
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
