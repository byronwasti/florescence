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

#[derive(Clone, Debug)]
pub struct Nucleus<A> {
    uuid: Uuid,
    propagativity: Propagativity,
    reality_token: RealityToken,
    core_map: ItcMap<PeerInfo<A>>,
}

impl<A> Nucleus<A>
where
    A: Clone + for<'a> Deserialize<'a> + Serialize,
{
    pub(crate) fn new(uuid: Uuid, own_addr: A) -> Self {
        let own_info = PeerInfo::new(uuid, own_addr);
        let reality_token = RealityToken::new(uuid);
        let mut core_map = ItcMap::new();
        core_map.insert(IdTree::One, own_info);
        Self {
            propagativity: Propagativity::Resting(IdTree::One, Instant::now()),
            reality_token,
            core_map,
            uuid,
        }
    }

    pub(crate) fn timestamp(&self) -> &EventTree {
        self.core_map.timestamp()
    }

    pub(crate) fn id(&self) -> Option<&IdTree> {
        self.propagativity.id()
    }

    fn create_patch(&self, peer_ts: &EventTree) -> BinaryPatch {
        let itc_patch: Patch<PeerInfo<A>> = self.core_map.diff(peer_ts);
        BinaryPatch::new(itc_patch).expect("Error serializing patch")
    }

    fn apply_patch(&mut self, patch: BinaryPatch) -> Result<(), PatchApplyError> {
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

    /* Message Handling */

    //pub(crate) fn handle_message(&mut self, message: PollinationMessage) -> Result<(Option<PollinationMessage>, Option<Self>), NucleusError> {
    pub(crate) fn handle_message(
        &mut self,
        message: PollinationMessage,
    ) -> Result<HandleMessageRes<A>, NucleusError> {
        use PollinationMessage::*;
        match message {
            Heartbeat { .. } => Ok(self.handle_heartbeat(message).into()),

            Update { .. } => Ok(self.handle_update(message)?.into()),

            RealitySkew { .. } => {
                todo!()
            }

            NewMember { .. } => {
                todo!()
            }

            Seed { .. } => {
                todo!()
            }

            // TODO: Get rid of SeeOther
            SeeOther { .. } => {
                todo!()
            }
        }
    }

    fn handle_heartbeat(&self, message: PollinationMessage) -> Option<PollinationMessage> {
        let PollinationMessage::Heartbeat {
            timestamp: peer_ts,
            reality_token: peer_rt,
            ..
        } = message else {
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
            patch,
            ..
        } = message else {
            unreachable!()
        };

        match self.timestamp().partial_cmp(&peer_ts) {
            Some(Ordering::Greater) => {
                let patch = self.create_patch(&peer_ts);
                Ok(self.msg_update(patch))
            }

            Some(Ordering::Less) | None => {
                if peer_rt != self.reality_token {
                    // Apply patch on a clone to detect RealitySkew
                    let mut self_clone = self.clone();
                    match self_clone.apply_patch(patch) {
                        Ok(()) => {
                            if self_clone.reality_token == peer_rt {
                                *self = self_clone;
                                Ok(self.msg_heartbeat())
                            } else {
                                let patch = self.create_patch(&peer_ts);
                                Ok(self.msg_reality_skew(patch))
                            }
                        }
                        Err(PatchApplyError::SelfRemoved) => {
                            let patch = self.create_patch(&peer_ts);
                            Ok(self.msg_reality_skew(patch))
                        }
                        Err(PatchApplyError::DeserializationError(err)) => Err(err.into()),
                    }
                } else {
                    match self.apply_patch(patch) {
                        Ok(()) => {
                            if self.reality_token != peer_rt {
                                panic!("Reality token mismatch after a clean update");
                            } else {
                                Ok(self.msg_heartbeat())
                            }
                        }
                        Err(PatchApplyError::SelfRemoved) => {
                            panic!("Self removal during a peer_rt match update")
                        }
                        Err(PatchApplyError::DeserializationError(err)) => Err(err.into()),
                    }
                }
            }

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
            ..
        } = message else {
            unreachable!()
        };

        todo!()
    }

    fn msg_heartbeat(&self) -> Option<PollinationMessage> {
        let id = self.id()?.clone();
        Some(PollinationMessage::Heartbeat {
            uuid: self.uuid,
            id,
            timestamp: self.timestamp().to_owned(),
            reality_token: self.reality_token,
        })
    }

    fn msg_update(&self, patch: BinaryPatch) -> Option<PollinationMessage> {
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

    fn msg_seed(&self, new_id: IdTree) -> Option<PollinationMessage> {
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

    fn msg_see_other(&self) -> Option<PollinationMessage> {
        let id = self.id()?.clone();
        let patch = self.create_patch(&EventTree::Leaf(0));
        Some(PollinationMessage::SeeOther {
            uuid: self.uuid,
            id,
            timestamp: self.timestamp().to_owned(),
            reality_token: self.reality_token,
            patch,
        })
    }
}

pub struct HandleMessageRes<A> {
    response: Option<PollinationMessage>,
    old_core: Option<Nucleus<A>>,
}

impl<A> HandleMessageRes<A> {
    fn response(response: PollinationMessage) -> Self {
        Self {
            response: Some(response),
            old_core: None,
        }
    }

    fn core_dump(response: PollinationMessage, core: Nucleus<A>) -> Self {
        Self {
            response: Some(response),
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
enum NucleusError {
    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] bincode::error::DecodeError),
}

#[derive(Error, Debug)]
enum PatchApplyError {
    #[error("Update patch removed own ID")]
    SelfRemoved,

    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] bincode::error::DecodeError),
}
