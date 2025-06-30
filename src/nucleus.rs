use crate::{
    message::{BinaryPatch, PollinationMessage},
    peer_info::{PeerInfo, PeerStatus},
    propagativity::Propagativity,
    reality_token::RealityToken,
};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt, time::Instant};
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
    pub(crate) fn new(uuid: Uuid, own_data: A) -> Self {
        let own_info = PeerInfo::new(uuid, own_data);
        let reality_token = RealityToken::new(uuid);
        let mut core_map = ItcMap::new();
        core_map.insert(IdTree::One, own_info.clone());
        Self {
            propagativity: Propagativity::Resting(IdTree::One, Instant::now()),
            reality_token,
            core_map,
            uuid,
            own_info,
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

    pub(crate) fn bump(&mut self) {
        // TODO: Optimized version
        self.set_raw(self.own_info.clone());
    }

    pub(crate) fn reap_souls(&mut self) -> bool {
        self.reap_souls_inner().is_some()
    }

    fn reap_souls_inner(&mut self) -> Option<()> {
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
            debug!("Reclaimed Id: {new_id}");
            self.propagativity = Propagativity::Resting(new_id, Instant::now());
            self.set_raw(self.own_info.clone());
            Some(())
        } else {
            debug!("No Reclaim");
            None
        }
    }

    fn set_raw(&mut self, own_info: PeerInfo<A>) -> Option<()> {
        let mut removals = self.core_map.insert(self.id()?.clone(), own_info);
        for (_removed_id, info) in removals.drain(..) {
            self.reality_token.push(info.uuid);
        }
        self.reality_token.push(self.uuid);
        Some(())
    }

    pub(crate) fn create_patch(&self, peer_ts: &EventTree) -> BinaryPatch {
        let itc_patch: Patch<PeerInfo<A>> = self.core_map.diff(peer_ts);
        BinaryPatch::new(itc_patch).expect("Error serializing patch")
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
                        Err(PatchApplyError::RealitySkew(self_clone))
                    }
                }
                Err(PatchApplyError::SelfRemoved) => Err(PatchApplyError::RealitySkew(self_clone)),
                err => err,
            }
        } else {
            match self.apply_patch_unchecked(patch) {
                Ok(()) => {
                    if self.reality_token != peer_rt {
                        panic!("Reality token mismatch after a clean update");
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

    /// NOTE: Not a clean swap; the new core has most information
    /// wiped. This is a helper function to efficiently reset.
    fn swap_cores(&mut self, mut other: Nucleus<A>) -> Nucleus<A> {
        std::mem::swap(self, &mut other);
        self.propagativity = Propagativity::Unknown;
        self.core_map = ItcMap::new();
        self.reality_token = RealityToken::new(self.uuid);
        other
    }

    fn propagate(&mut self) -> Option<IdTree> {
        let new_id = self.propagativity.propagate()?;
        self.set_raw(self.own_info.clone())?;
        Some(new_id)
    }

    /* Message Handling */

    pub(crate) fn handle_message(
        &mut self,
        message: PollinationMessage,
    ) -> Result<HandleMessageRes<A>, NucleusError> {
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
            Some(Ordering::Greater) | None => {
                let patch = self.create_patch(&peer_ts);
                self.msg_update(patch)
            }
            Some(Ordering::Less) => self.msg_heartbeat(),
            Some(Ordering::Equal) => {
                if peer_rt != self.reality_token {
                    let patch = self.create_patch(&peer_ts);
                    self.msg_reality_skew(patch)
                } else {
                    None
                }
            }
        }
    }

    fn handle_update(
        &mut self,
        message: PollinationMessage,
    ) -> Result<Option<PollinationMessage>, NucleusError> {
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
            Some(Ordering::Greater) => {
                let patch = self.create_patch(&peer_ts);
                Ok(self.msg_update(patch))
            }

            Some(Ordering::Less) | None => match self.apply_patch(peer_rt, peer_patch) {
                Ok(()) => Ok(self.msg_heartbeat()),
                Err(PatchApplyError::RealitySkew(_)) => {
                    let patch = self.create_patch(&peer_ts);
                    Ok(self.msg_reality_skew(patch))
                }
                Err(err) => Err(err.into()),
            },

            Some(Ordering::Equal) => {
                if peer_rt != self.reality_token {
                    let patch = self.create_patch(&peer_ts);
                    Ok(self.msg_reality_skew(patch))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn handle_reality_skew(
        &mut self,
        message: PollinationMessage,
    ) -> Result<HandleMessageRes<A>, NucleusError> {
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
            Ok(()) => Ok(HandleMessageRes::response(self.msg_heartbeat())),
            Err(PatchApplyError::RealitySkew(core)) => {
                if peer_count > self.peer_count()
                    || peer_count == self.peer_count() && peer_rt > self.reality_token
                {
                    let old_core = self.swap_cores(core);
                    Ok(HandleMessageRes::core_dump(self.msg_new_member(), old_core))
                } else {
                    let patch = self.create_patch(&peer_ts);
                    Ok(HandleMessageRes::response(self.msg_reality_skew(patch)))
                }
            }
            Err(PatchApplyError::SelfRemoved) => unreachable!(),
            Err(PatchApplyError::DeserializationError(err)) => {
                Err(NucleusError::DeserializationError(err))
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
            let _ = self.apply_patch_unchecked(peer_patch);
            self.propagativity = Propagativity::Resting(id, Instant::now());
            self.set_raw(self.own_info.clone());
            let patch = self.create_patch(&peer_ts);
            self.msg_update(patch)
        } else {
            let _ = self.apply_patch_unchecked(peer_patch);
            self.reality_token = peer_rt;
            None
        }
    }

    pub(crate) fn msg_heartbeat(&self) -> Option<PollinationMessage> {
        let id = self.id()?.clone();
        Some(PollinationMessage::Heartbeat {
            uuid: self.uuid,
            id,
            timestamp: self.timestamp().to_owned(),
            reality_token: self.reality_token,
        })
    }

    pub(crate) fn msg_update(&self, patch: BinaryPatch) -> Option<PollinationMessage> {
        let id = self.id()?.clone();
        Some(PollinationMessage::Update {
            uuid: self.uuid,
            id,
            timestamp: self.timestamp().to_owned(),
            reality_token: self.reality_token,
            patch,
        })
    }

    fn msg_reality_skew(&self, patch: BinaryPatch) -> Option<PollinationMessage> {
        let id = self.id()?.clone();
        Some(PollinationMessage::RealitySkew {
            uuid: self.uuid,
            id,
            timestamp: self.timestamp().to_owned(),
            reality_token: self.reality_token,
            patch,
            peer_count: self.core_map.len(),
        })
    }

    fn msg_new_member(&self) -> Option<PollinationMessage> {
        Some(PollinationMessage::NewMember { uuid: self.uuid })
    }

    fn msg_seed(&self, new_id: Option<IdTree>) -> Option<PollinationMessage> {
        let id = self.id()?.clone();
        let patch = self.create_patch(&EventTree::Leaf(0));
        Some(PollinationMessage::Seed {
            uuid: self.uuid,
            id,
            timestamp: self.timestamp().to_owned(),
            reality_token: self.reality_token,
            patch,
            peer_count: self.core_map.len(),
            new_id,
        })
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

pub struct HandleMessageRes<A> {
    response: Option<PollinationMessage>,
    old_core: Option<Nucleus<A>>,
}

impl<A> HandleMessageRes<A> {
    fn response(response: Option<PollinationMessage>) -> Self {
        Self {
            response,
            old_core: None,
        }
    }

    fn core_dump(response: Option<PollinationMessage>, core: Nucleus<A>) -> Self {
        Self {
            response,
            old_core: Some(core),
        }
    }
}

impl<A> From<Option<PollinationMessage>> for HandleMessageRes<A> {
    fn from(response: Option<PollinationMessage>) -> Self {
        Self {
            response,
            old_core: None,
        }
    }
}

#[derive(Error, Debug)]
pub enum NucleusError {
    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] bincode::error::DecodeError),

    #[error("Patch application error")]
    PatchApplyError,
}

impl<A> From<PatchApplyError<A>> for NucleusError {
    fn from(value: PatchApplyError<A>) -> NucleusError {
        match value {
            PatchApplyError::RealitySkew(_) => unreachable!(),
            PatchApplyError::SelfRemoved => NucleusError::PatchApplyError,
            PatchApplyError::DeserializationError(err) => NucleusError::DeserializationError(err),
        }
    }
}

#[derive(Error, Debug)]
enum PatchApplyError<A> {
    #[error("Reality skew")]
    RealitySkew(Nucleus<A>),

    #[error("Update patch removed own ID")]
    SelfRemoved,

    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] bincode::error::DecodeError),
}
