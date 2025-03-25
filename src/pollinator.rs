use crate::engine::EngineConnection;
use crate::*;
use std::collections::{HashMap, HashSet};
use treeclocks::{EventTree, ItcMap};
use serde::{Serialize, Deserialize};

mod identity_map;

pub use identity_map::IdentityMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct RealityToken(u64);
pub struct PeerInfo {}

pub trait Pollinator {
    type A: PollinatorInner + Sized;
    fn from_conn(conn: EngineConnection) -> (Self, Self::A)
    where
        Self: Sized;
}

pub trait PollinatorInner {}

pub(crate) struct PollinatorCore {
    timestamp: EventTree,
    reality_token: RealityToken,
    logic: Box<dyn PollinatorInner>,
    peerInfo: ItcMap<PeerInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct UpdatePacket<T> {
    inner: T
}
