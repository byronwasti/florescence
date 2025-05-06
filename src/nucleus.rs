use crate::message::BinaryPatch;
use crate::peer_info::{PeerInfo, PeerStatus};
use crate::propagativity::Propagativity;
use crate::reality_token::RealityToken;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Instant;
use thiserror::Error;
use treeclocks::{EventTree, IdTree, ItcMap, Patch};
use uuid::Uuid;

mod recycling;

#[derive(Clone, Debug)]
pub struct Nucleus<A> {
    propagativity: Propagativity,
    reality_token: RealityToken,
    core_map: ItcMap<PeerInfo<A>>,
    uuid: Uuid,
}

impl<A> Nucleus<A>
where
    A: Clone + for<'a> Deserialize<'a> + Serialize,
{
    pub(crate) fn new() -> Self {
        Default::default()
    }

    #[allow(unused)]
    pub(crate) fn own_info(&self) -> Option<&PeerInfo<A>> {
        let id = self.propagativity.id()?;
        self.core_map.get(id)
    }

    pub(crate) fn reset(&mut self, id: IdTree, patch: BinaryPatch) {
        // TODO: Handle error
        let patch: Patch<PeerInfo<A>> = patch.decode().expect("Error deserializing patch");
        self.propagativity = Propagativity::Resting(id.clone(), Instant::now());

        let mut core_map = ItcMap::new();
        let _ = core_map.apply(patch);
        self.core_map = core_map;
        self.reality_token = self.recalculate_reality_token();
    }

    pub(crate) fn from_parts(
        id: IdTree,
        mut reality_token: RealityToken,
        patch: BinaryPatch,
    ) -> Self {
        // TODO: Handle error
        let patch: Patch<PeerInfo<A>> = patch.decode().expect("Error deserializing patch");
        let propagativity = Propagativity::Resting(id.clone(), Instant::now());

        let mut core_map = ItcMap::new();
        let _ = core_map.apply(patch);

        let uuid = Uuid::new_v4();
        reality_token.push(uuid);

        Self {
            propagativity,
            reality_token,
            core_map,
            uuid,
        }
    }

    pub(crate) fn set(&mut self, addr: A) -> bool {
        if let Some(id) = self.propagativity.id() {
            let own_info = PeerInfo::new(self.uuid, addr);
            self.insert(id.clone(), own_info)
        } else {
            false
        }
    }

    fn insert(&mut self, id: IdTree, info: PeerInfo<A>) -> bool {
        self.reality_token.push(info.uuid);
        let mut any_removed = false;
        let mut removals = self.core_map.insert(id, info);
        for (_, info) in removals.drain(..) {
            any_removed = true;
            self.reality_token.push(info.uuid);
        }
        any_removed
    }

    pub(crate) fn mark_dead(&mut self, dead_id: IdTree) {
        self.insert(dead_id, PeerInfo::dead());
    }

    pub(crate) fn bump(&mut self) {
        if let Some(id) = self.propagativity.id() {
            self.core_map.event(id);
        }
    }

    pub(crate) fn reality_token(&self) -> RealityToken {
        self.reality_token
    }

    #[allow(unused)]
    pub(crate) fn recalculate_reality_token(&self) -> RealityToken {
        let mut reality_token = RealityToken::default();
        for (_, val) in self.core_map.iter() {
            reality_token.push(val.uuid);
        }
        reality_token
    }

    #[allow(unused)]
    pub(crate) fn reap_souls(&mut self) -> Option<()> {
        let dead_peers: IdTree = self
            .core_map
            .iter()
            .filter_map(|(peer_id, peer_info)| {
                // TODO: How to calculate timed-out peers?
                if peer_info.status == PeerStatus::Dead {
                    Some(peer_id.to_owned())
                } else {
                    None
                }
            })
            .reduce(|acc, id| acc.join(id))?;

        let new_id = recycling::claim_ids(self.id()?.clone(), dead_peers);

        if &new_id != self.id()? {
            info!("BYRON =>> Reclaimed Id: {new_id}");
            let own_addr = self.own_info()?.clone().addr?;
            self.propagativity = Propagativity::Resting(new_id, Instant::now());
            self.set(own_addr);
            Some(())
        } else {
            info!("BYRON =>> No Reclaim");
            None
        }
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
        let own_addr = self.core_map.get(id)?.clone().addr?;
        let peer_id = self.propagativity.propagate()?;
        self.set(own_addr);

        Some(peer_id)
    }

    pub(crate) fn create_patch(&self, peer_ts: &EventTree) -> BinaryPatch {
        let itc_patch: Patch<PeerInfo<A>> = self.core_map.diff(peer_ts);
        BinaryPatch::new(itc_patch).expect("Error serializing patch")
    }

    pub(crate) fn apply(
        &mut self,
        peer_rt: RealityToken,
        patch: BinaryPatch,
    ) -> Result<(), NucleusError> {
        let patch: Patch<PeerInfo<A>> = patch.decode()?;
        let mut new_rt = self.reality_token;
        let mut new_core = if peer_rt != self.reality_token {
            self.core_map.clone()
        } else {
            std::mem::take(&mut self.core_map)
        };

        let (mut additions, mut removals) = new_core.apply(patch);
        for (_, info) in removals.drain(..) {
            if info.uuid == self.uuid {
                return Err(NucleusError::RealitySkew);
            }
            new_rt.push(info.uuid);
        }
        for (_, info) in additions.drain(..) {
            new_rt.push(info.uuid);
        }

        if new_rt != peer_rt {
            if peer_rt == self.reality_token {
                panic!("Corrupted update.");
            }

            Err(NucleusError::RealitySkew)
        } else {
            self.reality_token = new_rt;
            self.core_map = new_core;
            Ok(())
        }
    }
}

impl<A: fmt::Display> Nucleus<A> {
    pub fn beautiful(&self) -> String {
        let map: Vec<String> = self
            .core_map
            .iter()
            .map(|(id, val)| format!("{id}: {val}"))
            .collect();
        let map = map.join(", ");
        format!(
            "{}\n\tuuid:{}\n\trt:{}\n\tts:{}\n\t{}",
            self.propagativity,
            self.uuid,
            self.reality_token,
            self.core_map.timestamp(),
            map,
        )
    }
}

impl<A> Default for Nucleus<A> {
    fn default() -> Self {
        let uuid = Uuid::new_v4();
        let reality_token = RealityToken::default();
        Self {
            propagativity: Propagativity::Propagating(IdTree::One),
            reality_token,
            core_map: ItcMap::new(),
            uuid,
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
            "id:{} - rt:{} - uuid:{} - map:{}",
            self.propagativity, self.reality_token, self.uuid, self.core_map
        )
    }
}

#[derive(Debug, Error)]
pub(crate) enum NucleusError {
    #[error("RealitySkew")]
    RealitySkew,

    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] bincode::error::DecodeError),

    #[error("Other: {0}")]
    Other(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nucleus() {
        let n0_uuid = Uuid::new_v4();
        let mut n0 = Nucleus::new();
        n0.set(10);
        assert_eq!(n0.timestamp().to_string(), "1".to_string());

        let peer_id = n0.propagate().unwrap();
        assert_eq!(n0.timestamp().to_string(), "(1, 1, 0)".to_string());

        let patch = n0.create_patch(&EventTree::new());
        n0.set(12);
        assert_eq!(n0.timestamp().to_string(), "(1, 2, 0)".to_string());

        let n1_uuid = Uuid::new_v4();
        let mut n1 = Nucleus::<usize>::from_parts(peer_id, n0.reality_token(), patch);
        n1.set(0);
        n1.set(1);

        assert_eq!(n1.timestamp().to_string(), "(2, 0, 1)".to_string());
        assert_eq!(n0.timestamp().to_string(), "(1, 2, 0)".to_string());
    }
}
