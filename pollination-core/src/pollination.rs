use crate::{
    message::{BinaryPatch, PollinationMessage, Topic},
    peer_info::{PeerInfo, PeerStatus},
    propagativity::Propagativity,
    reality_token::RealityToken,
};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt};
use thiserror::Error;
use treeclocks::{EventTree, IdTree, ItcMap, Patch};
use uuid::Uuid;

mod recycling;

#[derive(Clone, Debug)]
pub struct PollinationNode<A> {
    uuid: Uuid,
    topic: Topic,
    propagativity: Propagativity,
    reality_token: RealityToken,
    core_map: ItcMap<PeerInfo<A>>,
    own_info: PeerInfo<A>,
}

impl<A> PollinationNode<A>
where
    A: Clone + for<'a> Deserialize<'a> + Serialize,
{
    #[allow(unused)]
    pub fn new(uuid: Uuid, topic: Topic, own_data: A) -> Self {
        let own_info = PeerInfo::new(uuid, own_data);
        let reality_token = RealityToken::new(uuid);
        let mut core_map = ItcMap::new();
        core_map.insert(IdTree::One, own_info.clone());
        Self {
            propagativity: Propagativity::Propagating(IdTree::One),
            topic,
            reality_token,
            core_map,
            uuid,
            own_info,
        }
    }

    pub fn timestamp(&self) -> &EventTree {
        self.core_map.timestamp()
    }

    pub fn id(&self) -> Option<&IdTree> {
        self.propagativity.id()
    }

    pub fn peer_count(&self) -> usize {
        self.core_map.len()
    }

    pub fn bump(&mut self) {
        // TODO: Optimized version
        self.set_raw(self.own_info.clone());
    }

    pub fn peers(&self) -> impl Iterator<Item = (&IdTree, &PeerInfo<A>)> {
        self.core_map.iter()
    }

    /*
    pub fn set_propagating(&mut self) -> bool {
        use Propagativity::*;
        self.propagativity = match self.propagativity {
            Unknown => Unknown,
            Propagating(id) | Resting(id) => Propagating(id)
        };

        !matches!(self.propagativity, Unknown)
    }
    */

    fn set_raw(&mut self, own_info: PeerInfo<A>) -> Option<()> {
        let dead = matches!(own_info.status, PeerStatus::Dead);
        let mut removals = self.core_map.insert(self.id()?.clone(), own_info);
        for (_removed_id, info) in removals.drain(..) {
            self.reality_token.push(info.uuid);
        }
        if !dead {
            self.reality_token.push(self.uuid);
        }
        Some(())
    }

    fn create_patch(&self, peer_ts: &EventTree) -> BinaryPatch {
        let itc_patch: Patch<PeerInfo<A>> = self.core_map.diff(peer_ts);
        BinaryPatch::new(itc_patch).expect("Error serializing patch")
    }

    /// NOTE: Not a clean swap; the new core has most information
    /// wiped. This is a helper function to efficiently reset.
    fn swap_cores(&mut self, mut other: PollinationNode<A>) -> PollinationNode<A> {
        self.set_raw(PeerInfo::dead());
        std::mem::swap(self, &mut other);
        self.propagativity = Propagativity::Unknown;
        self.core_map = ItcMap::new();
        self.reality_token = RealityToken::zero();
        other
    }

    fn propagate(&mut self) -> Option<IdTree> {
        let new_id = self.propagativity.propagate()?;
        self.set_raw(self.own_info.clone())?;
        Some(new_id)
    }

    /// NOTE: Doesn't return PatchApplyError::SelfRemoved
    fn apply_patch(
        &mut self,
        peer_rt: RealityToken,
        patch: BinaryPatch,
    ) -> Result<(), PatchApplyError<A>> {
        if peer_rt != self.reality_token {
            // Apply patch on a clone to detect RealitySkew
            let mut self_clone = self.clone();
            match self_clone.apply_patch_unchecked(patch) {
                Ok(()) => {
                    if self_clone.reality_token == peer_rt {
                        *self = self_clone;
                        Ok(())
                    } else {
                        Err(PatchApplyError::RealitySkew(Box::new(self_clone)))
                    }
                }
                Err(PatchApplyError::SelfRemoved) => {
                    Err(PatchApplyError::RealitySkew(Box::new(self_clone)))
                }
                err => err,
            }
        } else {
            match self.apply_patch_unchecked(patch) {
                Ok(()) => {
                    if self.reality_token != peer_rt {
                        panic!(
                            "Reality token mismatch after a clean update. This is a bug. {} != {peer_rt}",
                            self.reality_token
                        );
                    } else {
                        Ok(())
                    }
                }
                Err(PatchApplyError::SelfRemoved) => {
                    panic!("Self removal during a peer_rt match update")
                }
                Err(PatchApplyError::DeserializationError(err)) => Err(err.into()),
                _ => unreachable!(),
            }
        }
    }

    fn apply_patch_unchecked(&mut self, patch: BinaryPatch) -> Result<(), PatchApplyError<A>> {
        let patch: Patch<PeerInfo<A>> = patch.decode()?;
        let (mut additions, mut removals) = self.core_map.apply(patch);

        for (_, info) in additions.drain(..) {
            self.reality_token.push(info.uuid);
        }

        let mut self_removed = false;
        for (removed_id, info) in removals.drain(..) {
            self.reality_token.push(info.uuid);
            if let Some(own_id) = self.id() {
                if *own_id == removed_id {
                    debug!("Applied patch removes own ID");
                    self_removed = true;
                }
            }
        }

        if self_removed {
            Err(PatchApplyError::SelfRemoved)
        } else {
            Ok(())
        }
    }

    fn apply_seed_patch(&mut self, patch: BinaryPatch) -> Result<(), PatchApplyError<A>> {
        let mut new_core = self.clone();
        new_core.apply_patch_unchecked(patch)?;

        let mut id_to_delete = None;
        for (id, peer_info) in new_core.peers() {
            if peer_info.uuid == self.uuid {
                assert!(id_to_delete.is_none());
                id_to_delete = Some(id);
            }
        }

        if let Some(id) = id_to_delete {
            // TODO: Rethink this logic
            warn!("Seed patch for group already a member of; killing old ID");
            new_core.propagativity = Propagativity::Propagating(id.clone());
            new_core.set_raw(PeerInfo::dead());
        }

        *self = new_core;

        Ok(())
    }

    pub fn reap_souls(&mut self) -> bool {
        self.reap_souls_inner().is_some()
    }

    fn reap_souls_inner(&mut self) -> Option<()> {
        let dead_peers: IdTree = self
            .peers()
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
            debug!("Reclaimed Id: {new_id}");
            self.propagativity = Propagativity::Propagating(new_id);
            self.set_raw(self.own_info.clone());
            Some(())
        } else {
            debug!("No Reclaim");
            None
        }
    }

    fn check_no_dupes(&self) {
        let mut dupe_checker = std::collections::HashSet::new();
        for (_, peer) in self.peers() {
            let uuid = peer.uuid;
            if dupe_checker.contains(&uuid) && uuid != Uuid::from_u128(0) {
                panic!("DUPE with {uuid}");
            }

            dupe_checker.insert(uuid);
        }
    }

    /* Message Handling */

    pub fn handle_message(
        &mut self,
        message: PollinationMessage,
    ) -> Result<PollinationResponse<A>, PollinationError> {
        use PollinationMessage::*;
        match message {
            Heartbeat { .. } => Ok(self.handle_heartbeat(message).into()),

            Update { .. } => Ok(self.handle_update(message)?.into()),

            RealitySkew { .. } => self.handle_reality_skew(message),

            NewMember { .. } => Ok(self.handle_new_member(message).into()),

            Seed { .. } => Ok(self.handle_seed(message).into()),
        }
    }

    fn handle_heartbeat(&self, message: PollinationMessage) -> Option<PollinationMessage> {
        let PollinationMessage::Heartbeat {
            timestamp: peer_ts,
            reality_token: peer_rt,
            ..
        } = message
        else {
            unreachable!()
        };

        match self.timestamp().partial_cmp(&peer_ts) {
            Some(Ordering::Greater) | None => self.msg_update(&peer_ts),
            Some(Ordering::Less) => self.msg_heartbeat(),
            Some(Ordering::Equal) => {
                if peer_rt != self.reality_token {
                    self.msg_reality_skew(&peer_ts)
                } else {
                    None
                }
            }
        }
    }

    fn handle_update(
        &mut self,
        message: PollinationMessage,
    ) -> Result<Option<PollinationMessage>, PollinationError> {
        let PollinationMessage::Update {
            timestamp: peer_ts,
            reality_token: peer_rt,
            patch: peer_patch,
            ..
        } = message
        else {
            unreachable!()
        };

        match self.timestamp().partial_cmp(&peer_ts) {
            Some(Ordering::Greater) => Ok(self.msg_update(&peer_ts)),

            Some(Ordering::Less) | None => match self.apply_patch(peer_rt, peer_patch) {
                Ok(()) => Ok(self.msg_heartbeat()),
                Err(PatchApplyError::RealitySkew(_)) => Ok(self.msg_reality_skew(&peer_ts)),
                Err(err) => Err(err.into()),
            },

            Some(Ordering::Equal) => {
                if peer_rt != self.reality_token {
                    Ok(self.msg_reality_skew(&peer_ts))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn handle_reality_skew(
        &mut self,
        message: PollinationMessage,
    ) -> Result<PollinationResponse<A>, PollinationError> {
        let PollinationMessage::RealitySkew {
            timestamp: peer_ts,
            reality_token: peer_rt,
            peer_count,
            patch: peer_patch,
            ..
        } = message
        else {
            unreachable!()
        };

        match self.apply_patch(peer_rt, peer_patch) {
            Ok(()) => Ok(PollinationResponse::response(self.msg_heartbeat())),
            Err(PatchApplyError::RealitySkew(core)) => {
                if peer_count > self.peer_count()
                    || peer_count == self.peer_count() && peer_rt > self.reality_token
                {
                    let old_core = self.swap_cores(*core);
                    Ok(PollinationResponse::core_dump(
                        self.msg_new_member(),
                        old_core,
                    ))
                } else {
                    Ok(PollinationResponse::response(
                        self.msg_reality_skew(&peer_ts),
                    ))
                }
            }
            Err(PatchApplyError::SelfRemoved) => unreachable!(),
            Err(PatchApplyError::DeserializationError(err)) => {
                Err(PollinationError::DeserializationError(err))
            }
        }
    }

    fn handle_new_member(&mut self, _message: PollinationMessage) -> Option<PollinationMessage> {
        let new_id = self.propagate();
        self.msg_seed(new_id)
    }

    fn handle_seed(&mut self, message: PollinationMessage) -> Option<PollinationMessage> {
        let PollinationMessage::Seed {
            timestamp: peer_ts,
            reality_token: peer_rt,
            patch: peer_patch,
            new_id,
            ..
        } = message
        else {
            unreachable!()
        };

        // Only reset ourselves if we have no ID
        if self.propagativity.id().is_some() {
            return self.msg_heartbeat();
        }

        if let Some(id) = new_id {
            let _ = self.apply_seed_patch(peer_patch);
            self.propagativity = Propagativity::resting(id);
            self.set_raw(self.own_info.clone());
            self.check_no_dupes();
            self.msg_update(&peer_ts)
        } else {
            let _ = self.apply_patch_unchecked(peer_patch);
            self.reality_token = peer_rt;
            None
        }
    }

    pub fn msg_heartbeat(&self) -> Option<PollinationMessage> {
        let id = self.id()?.clone();
        Some(PollinationMessage::Heartbeat {
            uuid: self.uuid,
            topic: self.topic.clone(),
            id,
            timestamp: self.timestamp().to_owned(),
            reality_token: self.reality_token,
        })
    }

    pub fn msg_update(&self, peer_ts: &EventTree) -> Option<PollinationMessage> {
        let id = self.id()?.clone();
        let patch = self.create_patch(peer_ts);
        Some(PollinationMessage::Update {
            uuid: self.uuid,
            topic: self.topic.clone(),
            id,
            timestamp: self.timestamp().to_owned(),
            reality_token: self.reality_token,
            patch,
        })
    }

    fn msg_reality_skew(&self, peer_ts: &EventTree) -> Option<PollinationMessage> {
        let id = self.id()?.clone();
        let patch = self.create_patch(peer_ts);
        Some(PollinationMessage::RealitySkew {
            uuid: self.uuid,
            topic: self.topic.clone(),
            id,
            timestamp: self.timestamp().to_owned(),
            reality_token: self.reality_token,
            patch,
            peer_count: self.core_map.len(),
        })
    }

    fn msg_new_member(&self) -> Option<PollinationMessage> {
        Some(PollinationMessage::NewMember {
            uuid: self.uuid,
            topic: self.topic.clone(),
        })
    }

    fn msg_seed(&self, new_id: Option<IdTree>) -> Option<PollinationMessage> {
        let id = self.id()?.clone();
        let patch = self.create_patch(&EventTree::Leaf(0));
        Some(PollinationMessage::Seed {
            uuid: self.uuid,
            topic: self.topic.clone(),
            id,
            timestamp: self.timestamp().to_owned(),
            reality_token: self.reality_token,
            patch,
            peer_count: self.core_map.len(),
            new_id,
        })
    }
}

impl<A> fmt::Display for PollinationNode<A>
where
    A: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "UUID:{} TOPIC:{} ID:{} RT:{} OWN_INFO:({}) CORE_MAP:({})",
            self.uuid,
            self.topic,
            self.propagativity,
            self.reality_token,
            self.own_info,
            self.core_map
        )
    }
}

pub struct PollinationResponse<A> {
    pub response: Option<PollinationMessage>,
    pub old_core: Option<PollinationNode<A>>,
}

impl<A> PollinationResponse<A> {
    fn response(response: Option<PollinationMessage>) -> Self {
        Self {
            response,
            old_core: None,
        }
    }

    fn core_dump(response: Option<PollinationMessage>, core: PollinationNode<A>) -> Self {
        Self {
            response,
            old_core: Some(core),
        }
    }
}

impl<A> From<Option<PollinationMessage>> for PollinationResponse<A> {
    fn from(response: Option<PollinationMessage>) -> Self {
        Self {
            response,
            old_core: None,
        }
    }
}

#[derive(Error, Debug)]
pub enum PollinationError {
    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] crate::serialization::DeserializeError),

    #[error("Patch application error")]
    PatchApplyError,
}

impl<A> From<PatchApplyError<A>> for PollinationError {
    fn from(value: PatchApplyError<A>) -> PollinationError {
        match value {
            PatchApplyError::RealitySkew(_) => unreachable!(),
            PatchApplyError::SelfRemoved => PollinationError::PatchApplyError,
            PatchApplyError::DeserializationError(err) => {
                PollinationError::DeserializationError(err)
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum PatchApplyError<A> {
    #[error("Reality skew")]
    RealitySkew(Box<PollinationNode<A>>),

    #[error("Update patch removed own ID")]
    SelfRemoved,

    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] crate::serialization::DeserializeError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_nucleus() {
        // BAD SEEDS = [
        //      16254458854126421037  w/ 3
        // ]
        for _ in 0..40 {
            let seed = rand::thread_rng().random();
            println!(
                "\n\n================== NEW SIMULATION WITH SEED {seed} =======================\n"
            );
            let id_tree = test_nucleus_inner(seed);
            if id_tree != IdTree::one() {
                println!("FAILED TO CLEAN UP WITH SEED {seed}, GOT TO {id_tree}");
                //panic!("Test failure");
            }
        }
    }

    fn test_nucleus_inner(seed: u64) -> IdTree {
        let mut rng = StdRng::seed_from_u64(seed);

        let count = 5;
        let mut nuclei = (0..count)
            .map(|i| {
                PollinationNode::new(
                    Uuid::from_u128(rng.random()),
                    Topic::new("test".to_string()),
                    i,
                )
            })
            .collect::<Vec<_>>();

        for _ in 0..1000 {
            let mut idxs = (0..count).collect::<Vec<_>>();
            idxs.shuffle(&mut rng);
            for i in idxs {
                // Includes Self but that is okay and we should test that fact.
                let j = rng.gen_range(0..count);

                println!("SYNC {i} -> {j}");
                let sync_res = sync_two(&mut rng, &mut nuclei, i, j);
                println!("END\n");
                if let Some(old_core) = sync_res {
                    println!("NOTIFYING KIN");
                    notify_of_death(&mut rng, &mut nuclei, old_core);
                    println!("END\n");
                }

                for i in 0..count {
                    println!("STATE {i}: {}", nuclei[i]);
                    // Reset propagativity so we don't have to wait
                    nuclei[i].propagativity.force_propagating();

                    // Also do any cleanup
                    // TODO: Test delayed cleanup
                    if nuclei[i].reap_souls() {
                        println!("{i} REAPED SOULS");
                    }
                }

                println!("");
            }

            let mut iter = nuclei.iter().map(|n| n.reality_token);
            let first = iter.next().unwrap();
            if iter.all(|rt| rt == first) {
                println!("CONVERGED");
                break;
            }
        }

        println!("END SIMULATION");
        let mut summed_ids = IdTree::zero();
        for i in 0..count {
            println!("END STATE {i}: {}", nuclei[i]);
            if let Some(id) = nuclei[i].id() {
                summed_ids = summed_ids.join(id.to_owned());
            }
        }

        println!("TOTAL ID SPACE: {summed_ids}");
        summed_ids
    }

    fn notify_of_death<R: Rng>(
        rng: &mut R,
        nuclei: &mut [PollinationNode<usize>],
        old_core: PollinationNode<usize>,
    ) -> Option<()> {
        for (_, peer) in old_core.peers() {
            // 10% chance of failure
            if rng.random_bool(0.10) {
                //println!("DEATH NOTIFICATION LOOP ENDED EARLY");
                //break
            }

            if peer.addr.is_none() {
                println!("NO PEER ADDR FOR NOTIFICATION OF DEATH: {peer}");
                continue;
            }
            let peer_idx = peer.addr.unwrap();
            let peer_nucl = &mut nuclei[peer_idx];
            let msg = old_core.msg_update(peer_nucl.timestamp())?;
            println!("{peer_idx} <- {msg}");
            let _ = peer_nucl
                .handle_message(msg)
                .expect("Pollination hit an error");
        }
        None
    }

    fn sync_two<R: Rng>(
        rng: &mut R,
        nuclei: &mut [PollinationNode<usize>],
        i: usize,
        j: usize,
    ) -> Option<PollinationNode<usize>> {
        nuclei[i].bump();
        let mut msg = nuclei[i].msg_heartbeat();
        if msg.is_none() {
            msg = nuclei[i].msg_new_member();
        }

        let mut old_core = None;
        let mut iters = 0;
        loop {
            // 10% chance of failure
            if rng.random_bool(0.10) {
                println!("SYNC LOOP ENDED EARLY");
                break old_core;
            }

            iters += 1;
            if iters >= 10 {
                panic!("Too many iterations during sync");
            }
            for idx in [j, i] {
                let res = match msg.take() {
                    Some(msg) => {
                        println!("{idx} <- {msg}");
                        nuclei[idx].handle_message(msg)
                    }
                    None => return old_core,
                }
                .expect("Pollination hit an error.");

                msg = res.response;
                if let Some(new_old_core) = res.old_core {
                    assert!(old_core.is_none());
                    old_core = Some(new_old_core);
                }
            }
        }
    }
}
