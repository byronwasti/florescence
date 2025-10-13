use crate::reality_token::RealityToken;
use treeclocks::{EventTree, ItcMap};

mod identity_map;

pub use identity_map::IdentityMap;

pub struct PeerInfo {}

pub trait Pollinator {
    type A: PollinatorInner + Sized;
    fn from_conn() -> (Self, Self::A)
    where
        Self: Sized;
}

pub trait PollinatorInner {}

#[allow(dead_code)]
pub(crate) struct PollinatorCore {
    timestamp: EventTree,
    reality_token: RealityToken,
    logic: Box<dyn PollinatorInner>,
    peer_info: ItcMap<PeerInfo>,
}
