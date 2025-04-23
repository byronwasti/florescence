use crate::message::BinaryPatch;
use crate::peer_info::PeerInfo;
use crate::propagativity::Propagativity;
use crate::reality_token::RealityToken;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Instant;
use thiserror::Error;
use treeclocks::{EventTree, IdTree, ItcMap};

pub(crate) struct Nucleus<A> {
    propagativity: Propagativity,
    reality_token: RealityToken,
    core_map: ItcMap<PeerInfo<A>>,
}

impl<A> Nucleus<A>
where
    A: Clone + for<'a> Deserialize<'a> + Serialize,
{
    pub(crate) fn new() -> Self {
        Default::default()
    }

    pub(crate) fn from_parts(id: IdTree, reality_token: RealityToken, patch: BinaryPatch) -> Self {
        // TODO: Handle error
        let patch = patch.deserialize().expect("Error deserializing patch");
        let propagativity = Propagativity::Resting(id.clone(), Instant::now());

        let mut core_map = ItcMap::new();
        let _ = core_map.apply(patch);

        Self {
            propagativity,
            reality_token,
            core_map,
        }
    }

    pub(crate) fn set(&mut self, own_info: PeerInfo<A>) -> bool {
        let mut any_removed = false;
        if let Some(id) = self.propagativity.id() {
            let mut removals = self.core_map.insert(id.clone(), own_info);
            for (id, _) in removals.drain(..) {
                any_removed = true;
                self.reality_token.increment(id);
            }
        }
        any_removed
    }

    pub(crate) fn bump(&mut self) {
        if let Some(id) = self.propagativity.id() {
            self.core_map.event(id);
        }
    }

    pub(crate) fn reality_token(&self) -> RealityToken {
        self.reality_token
    }

    pub(crate) fn timestamp(&self) -> &EventTree {
        self.core_map.timestamp()
    }

    pub(crate) fn id(&self) -> Option<&IdTree> {
        self.propagativity.id()
    }

    pub(crate) fn peer_count(&self) -> usize {
        self.core_map.len()
    }

    pub(crate) fn propagate(&mut self) -> Option<IdTree> {
        let id = self.propagativity.id()?;
        let own_info = self.core_map.get(id)?.clone();

        let peer_id = self.propagativity.propagate()?;
        self.set(own_info);

        Some(peer_id)
    }

    pub(crate) fn create_patch(&self, peer_ts: &EventTree) -> BinaryPatch {
        let itc_patch = self.core_map.diff(peer_ts);
        BinaryPatch::new(itc_patch).expect("Error serializing patch")
    }

    pub(crate) fn apply(
        &mut self,
        peer_rt: RealityToken,
        patch: BinaryPatch,
    ) -> Result<(), NucleusError> {
        let patch = patch.deserialize()?;
        let mut rt = self.reality_token;
        let mut core = if peer_rt != self.reality_token {
            self.core_map.clone()
        } else {
            std::mem::take(&mut self.core_map)
        };

        let mut removals = core.apply(patch);
        for (id, _) in removals.drain(..) {
            rt.increment(id);
        }

        if rt != peer_rt {
            if peer_rt == self.reality_token {
                panic!("Corrupted update.");
            }

            Err(NucleusError::RealitySkew)
        } else {
            self.reality_token = rt;
            self.core_map = core;
            Ok(())
        }
    }
}

impl<A> Default for Nucleus<A> {
    fn default() -> Self {
        Self {
            propagativity: Propagativity::Propagating(IdTree::One),
            reality_token: RealityToken::new(),
            core_map: ItcMap::new(),
        }
    }
}

impl<A> fmt::Display for Nucleus<A>
where
    A: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{} - {} - {}",
            self.propagativity, self.reality_token, self.core_map
        )
    }
}

#[derive(Debug, Error)]
pub(crate) enum NucleusError {
    #[error("RealitySkew")]
    RealitySkew,

    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] bincode::error::DecodeError),
}
