use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::HashSet};
use thiserror::Error;
use treeclocks::{EventTree, IdTree, ItcMap, Patch};
use uuid::Uuid;

pub struct PollinationCore<A> {
    id: IdTree,
    core_map: ItcMap<NodeInfo<A>>,
    own_info: NodeInfo<A>,
}

impl<A> PollinationCore<A>
where
    A: Clone + for<'a> Deserialize<'a> + Serialize,
{
    pub fn new(uuid: Uuid, addr: A) -> Self {
        let own_info = NodeInfo::new(uuid, addr);
        let id = IdTree::One;
        let mut core_map = ItcMap::new();
        core_map.insert(id.clone(), own_info.clone());

        Self {
            id,
            core_map,
            own_info,
        }
    }

    pub fn timestamp(&self) -> &EventTree {
        self.core_map.timestamp()
    }

    pub fn uuid(&self) -> Uuid {
        self.own_info.uuid
    }

    pub fn unique_count(&self) -> usize {
        // TODO: Efficiency
        let unique_count = self
            .core_map
            .iter()
            .map(|(_, n)| n.uuid)
            .collect::<HashSet<_>>();
        unique_count.len()
    }

    pub fn membership_hash(&self) -> MembershipHash {
        // TODO: Efficiency
        MembershipHash::new(&self.core_map)
    }

    pub fn heartbeat_message(&self) -> PollinationMessage<A> {
        let membership_hash = MembershipHash::new(&self.core_map);
        let unique_count = self.unique_count();

        PollinationMessage {
            uuid: self.own_info.uuid.clone(),
            id: self.id.clone(),
            timestamp: self.core_map.timestamp().clone(),
            membership_hash,
            unique_count,
            patch: None,
            new_membership: NewMembership::None,
        }
    }

    // Insert value at IdTree location, returning removed IdTrees and their values.
    fn insert(&mut self, id: IdTree, value: NodeInfo<A>) -> Vec<(IdTree, NodeInfo<A>)> {
        self.core_map.insert(id, value)
    }

    // NOTE: Degrades to be heartbeat_message() if the timestamps are equal
    fn update_message(&self, timestamp: &EventTree) -> PollinationMessage<A> {
        let mut msg = self.heartbeat_message();
        msg.patch = self.core_map.diff(timestamp);
        msg
    }

    fn new_member_message(&self) -> PollinationMessage<A> {
        // Include all of our peers to be included as well
        let mut msg = self.update_message(&EventTree::new());
        msg.new_membership = NewMembership::Request;
        msg
    }

    fn provide_member_message(&self) -> PollinationMessage<A> {
        let mut msg = self.update_message(&EventTree::new());
        msg.new_membership = NewMembership::Provide;
        msg
    }

    pub fn recycle(&mut self) -> bool {
        let dead_peers: Option<IdTree> = self
            .core_map
            .iter()
            .filter_map(|(id, info)| {
                if info.status == NodeStatus::Dead {
                    Some(id.to_owned())
                } else {
                    None
                }
            })
            .reduce(|acc, id| acc.join(id));

        let Some(dead_peers) = dead_peers else {
            return false;
        };

        let new_id = crate::recycling::recycle_ids(self.id.clone(), dead_peers);
        if self.id != new_id {
            self.id = new_id;
            true
        } else {
            false
        }
    }

    fn handle_skew(&self, message: PollinationMessage<A>) -> PollinationMessage<A> {
        match message.unique_count.cmp(&self.unique_count()) {
            Ordering::Greater => self.new_member_message(),
            Ordering::Less => self.heartbeat_message(),
            Ordering::Equal => {
                if message.membership_hash > self.membership_hash() {
                    self.new_member_message()
                } else {
                    self.heartbeat_message()
                }
            }
        }
    }

    fn handle_new_members(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Result<Option<PollinationMessage<A>>> {
        if self.membership_hash() == message.membership_hash {
            return Ok(None);
        }

        if self.unique_count() < message.unique_count {
            // TODO: I _think_ this can actually end up in a live-lock situation. So, probably fix
            // that.
            return Ok(None);
        }

        // A PollinationMessage with NewMembership::Request can be assumed to have the full ITCMap
        // as part of the request.
        // TODO: Proper error handling
        let peer_map: ItcMap<NodeInfo<A>> =
            ItcMap::from_patch(message.patch.ok_or(PollinationError::NoPatch)?);

        let mut peers = vec![];
        for (_, node) in peer_map.iter() {
            // TODO: Horribly inefficient; does it matter?
            if find_id(&self.core_map, node.uuid).is_some() {
                continue;
            }

            peers.push(node.clone());
        }

        if peers.is_empty() {
            Ok(None)
        } else {
            self.add_peers(peers);
            Ok(Some(self.provide_member_message()))
        }
    }

    fn handle_provided_membership(
        &mut self,
        message: PollinationMessage<A>,
    ) -> PollinationMessage<A> {
        todo!()
    }

    pub fn handle_message(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Option<PollinationMessage<A>> {
        if message.new_membership.is_request() {
            match self.handle_new_members(message.clone()) {
                Ok(Some(msg)) => return Some(msg),
                Ok(None) => debug!("Nothing to do for handling new membership"),
                Err(PollinationError::NoPatch) => {
                    debug!("No Patch present in NewMembership::Request message")
                }
            }
        }

        if message.new_membership.is_provide() {
            return Some(self.handle_provided_membership(message));
        }

        if let Some(patch) = message.patch.clone() {
            let mut updated_core = self.core_map.clone();
            let (added, removed) = updated_core.apply(patch); // TODO: Use these

            if find_id(&updated_core, self.uuid()).is_some() {
                if MembershipHash::new(&updated_core) != message.membership_hash {
                    // Definitely unclean update; memberhsip hash mismatch
                    Some(self.handle_skew(message))
                } else {
                    // Assume clean
                    // TODO: Are there edge cases?
                    self.core_map = updated_core;
                    Some(self.update_message(&message.timestamp))
                }
            } else {
                // Definitely unclean update; removed self
                Some(self.handle_skew(message))
            }
        } else {
            if &message.timestamp == self.timestamp() {
                if self.membership_hash() == message.membership_hash {
                    None
                } else {
                    Some(self.handle_skew(message))
                }
            } else {
                Some(self.update_message(&message.timestamp))
            }
        }
    }

    // Evenly divide the Self address space and distribute to peers. Then, assign every nodes
    // information to the map.
    fn add_peers(&mut self, mut peers: Vec<NodeInfo<A>>) {
        let mut new_ids = self.id.clone().fork_many(peers.len() + 1);
        self.id = new_ids[0].clone();
        self.insert(new_ids[0].clone(), self.own_info.clone());

        assert_eq!(new_ids.len(), peers.len() + 1);

        for (new_id, info) in new_ids.drain(1..).zip(peers.drain(..)) {
            let removals = self.insert(new_id, info);
            // There should be no removals, as inserting our own info should have removed ourselves
            // already.
            assert!(removals.is_empty());
        }
    }
}

fn find_id<A>(map: &ItcMap<NodeInfo<A>>, uuid: Uuid) -> Option<IdTree> {
    map.iter()
        .find(|(_, info)| info.uuid == uuid)
        .map(|(id, _)| id)
        .cloned()
}

// PollinationMessage

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollinationMessage<A> {
    uuid: Uuid,
    id: IdTree,
    timestamp: EventTree,
    membership_hash: MembershipHash,
    unique_count: usize,
    patch: Option<Patch<NodeInfo<A>>>,
    new_membership: NewMembership,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum NewMembership {
    None,
    Request,
    Provide,
}

impl NewMembership {
    fn is_none(&self) -> bool {
        matches!(self, NewMembership::None)
    }

    fn is_request(&self) -> bool {
        matches!(self, NewMembership::Request)
    }

    fn is_provide(&self) -> bool {
        matches!(self, NewMembership::Provide)
    }
}

// Error & Result

#[derive(Debug, Error)]
pub enum PollinationError {
    #[error("No patch present when one was expected")]
    NoPatch,
}

pub type Result<T> = std::result::Result<T, PollinationError>;

// NodeInfo

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeInfo<A> {
    uuid: Uuid,
    addr: A,
    timestamp: u64,
    status: NodeStatus,
}

impl<A> NodeInfo<A> {
    pub fn new(uuid: Uuid, addr: A) -> Self {
        NodeInfo {
            uuid,
            addr,
            timestamp: 1,
            status: NodeStatus::Healthy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum NodeStatus {
    Healthy,
    Dead,
}

// MembershipHash
use simplehash::fnv1a_64;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy, Serialize, Deserialize)]
struct MembershipHash(u64);

impl MembershipHash {
    fn new<A>(itc_map: &ItcMap<NodeInfo<A>>) -> Self {
        let hash = itc_map.map_recursive(
            &|node: &NodeInfo<A>| fnv1a_64(node.uuid.as_bytes()),
            &|l, r| {
                let mut bytes = [0u8; 16];
                bytes[..8].copy_from_slice(&l.to_le_bytes());
                bytes[8..].copy_from_slice(&r.to_le_bytes());
                fnv1a_64(&bytes)
            },
        );

        Self(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_membership_hashing() {
        let mut m0 = ItcMap::new();
        let mut m1 = ItcMap::new();

        let mut i0 = IdTree::one();
        let (i0, i1) = i0.fork();

        let n0: NodeInfo<_> = NodeInfo::new(Uuid::from_u128(1030), 0);
        let n1: NodeInfo<_> = NodeInfo::new(Uuid::from_u128(2313), 1);

        m0.insert(i0.clone(), n0.clone());
        m0.insert(i1.clone(), n1.clone());

        // Different tree
        m1.insert(i1.clone(), n0.clone());
        m1.insert(i0.clone(), n1.clone());

        let h0 = MembershipHash::new(&m0);
        let h1 = MembershipHash::new(&m1);

        assert_ne!(h0, h1);
    }
}
