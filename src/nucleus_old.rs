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
    uuid: Uuid,
    propagativity: Propagativity,
    reality_token: RealityToken,
    core_map: ItcMap<PeerInfo<A>>,
    own_info: PeerInfo<A>,
}

impl<A> Nucleus<A>
where
    A: Clone + for<'a> Deserialize<'a> + Serialize,
{
    pub(crate) fn new(uuid: Uuid, own_addr: A) -> Self {
        let reality_token = RealityToken::default();
        let own = PeerInfo::new(uuid, own_addr);
        let mut s = Self {
            propagativity: Propagativity::Propagating(IdTree::One),
            reality_token,
            core_map: ItcMap::new(),
            uuid,
            own_info: own.clone(),
        };
        assert!(!s.set_self());
        s
    }

    #[allow(unused)]
    pub(crate) fn own_info(&self) -> &PeerInfo<A> {
        &self.own_info
    }

    pub(crate) fn reset(&mut self, id: IdTree, patch: BinaryPatch) {
        // TODO: Handle error
        let patch: Patch<PeerInfo<A>> = patch.decode().expect("Error deserializing patch");
        self.propagativity = Propagativity::Resting(id.clone(), Instant::now());

        let mut core_map = ItcMap::new();
        let _ = core_map.apply(patch);
        self.core_map = core_map;
        self.reality_token = self.recalculate_reality_token();
        self.set_self();
        debug!("RESET");
    }

    pub(crate) fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub(crate) fn set(&mut self, addr: A) -> bool {
        self.own_info = PeerInfo::new(self.uuid, addr);
        self.set_self()
    }

    fn set_self(&mut self) -> bool {
        if let Some(id) = self.propagativity.id() {
            self.insert(id.clone(), self.own_info.clone())
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

        assert_eq!(self.reality_token(), self.recalculate_reality_token());
        any_removed
    }

    pub(crate) fn mark_dead(&mut self, dead_id: IdTree) {
        debug!("Mark Dead: {dead_id}");
        self.insert(dead_id, PeerInfo::dead());
    }

    pub(crate) fn bump(&mut self) {
        if let Some(id) = self.propagativity.id() {
            self.core_map.event(id);
        }
    }

    pub(crate) fn force_propagating(&mut self) {
        self.propagativity.force_propagating()
    }

    pub fn reality_token(&self) -> RealityToken {
        self.reality_token
    }

    pub(crate) fn recalculate_reality_token(&self) -> RealityToken {
        let mut reality_token = RealityToken::default();
        for (_, val) in self.core_map.iter() {
            reality_token.push(val.uuid);
        }
        reality_token
    }

    // TODO: Fix bugs in how reality token is handled to avoid needing this
    #[instrument(skip_all)]
    pub(crate) fn check_and_reset_reality_token(&mut self) {
        let recalculated = self.recalculate_reality_token();

        if recalculated != self.reality_token {
            error!(
                "Reality token incorrect: has={} expected={recalculated}",
                self.reality_token
            );
            self.reality_token = recalculated
        }
    }

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
            debug!("CHECK =>> Reclaimed Id: {new_id}");
            let own_addr = self.own_info().clone().addr?;
            self.propagativity = Propagativity::Resting(new_id, Instant::now());
            self.set(own_addr);
            Some(())
        } else {
            debug!("CHECK =>> No Reclaim");
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
        self.core_map
            .iter()
            .filter(|(_, info)| info.status == PeerStatus::Healthy)
            .count()
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

    pub(crate) fn contains_self(&self) -> bool {
        if let Some(id) = self.id() {
            if let Some(info) = self.core_map.get(id) {
                info.uuid == self.uuid
            } else {
                false
            }
        } else {
            false
        }
    }

    pub(crate) fn apply(&mut self, patch: BinaryPatch) -> Result<(), NucleusError> {
        let patch: Patch<PeerInfo<A>> = patch.decode()?;
        let (mut additions, mut removals) = self.core_map.apply(patch);

        for (_, info) in removals.drain(..) {
            self.reality_token.push(info.uuid);
        }
        for (_, info) in additions.drain(..) {
            self.reality_token.push(info.uuid);
        }

        Ok(())
    }

    /*
        pub(crate) fn apply_old(
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
                    debug!("CHECK =>> Self-UUID removal; RealitySkew");
                    return Err(NucleusError::RealitySkew);
                }
                debug!("CHECK =>> REMOVE UUID: {}", info.uuid);
                new_rt.push(info.uuid);
                debug!("CHECK =>> new_rt: {new_rt}");
            }
            for (_, info) in additions.drain(..) {
                debug!("CHECK =>> ADD UUID: {}", info.uuid);
                new_rt.push(info.uuid);
                debug!("CHECK =>> new_rt: {new_rt}");
            }

            if new_rt != peer_rt {
                debug!("CHECK =>> rt !=");
                /*
                if peer_rt == self.reality_token {
                    panic!("Corrupted update.");
                }
                */

                Err(NucleusError::RealitySkew)
            } else {
                debug!("CHECK =>> OK");
                self.reality_token = new_rt;
                self.core_map = new_core;
                self.check_and_reset_reality_token();
                Ok(())
            }
        }
    */
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

impl<A> fmt::Display for Nucleus<A>
where
    A: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "id:{} - rt:{} - uuid:{} - own:{} - map:{}",
            self.propagativity, self.reality_token, self.uuid, self.own_info, self.core_map
        )
    }
}

#[derive(Debug, Error)]
pub(crate) enum NucleusError {
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
        let mut n0 = Nucleus::new(n0_uuid, 0);
        assert_eq!(n0.timestamp().to_string(), "1".to_string());

        let peer_id = n0.propagate().unwrap();
        assert_eq!(n0.timestamp().to_string(), "(1, 1, 0)".to_string());

        let patch = n0.create_patch(&EventTree::new());
        n0.set(12);
        assert_eq!(n0.timestamp().to_string(), "(1, 2, 0)".to_string());

        let n1_uuid = Uuid::new_v4();
        let mut n1 = Nucleus::new(n1_uuid, 0);
        n1.reset(peer_id, patch);
        n1.set(1);

        assert_eq!(n1.timestamp().to_string(), "(2, 0, 1)".to_string());
        assert_eq!(n0.timestamp().to_string(), "(1, 2, 0)".to_string());
    }
}
