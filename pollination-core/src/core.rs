use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::HashSet};
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
        let unique_count = self
            .core_map
            .iter()
            .map(|(_, n)| n.uuid)
            .collect::<HashSet<_>>();
        unique_count.len()
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
            request_membership: false,
        }
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
        msg.request_membership = true;
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

    pub fn handle_message(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Option<PollinationMessage<A>> {
        // TODO: request_membership handling

        if let Some(patch) = message.patch {
            let membership_hash = MembershipHash::new(&self.core_map);
            let mut updated_core = self.core_map.clone();
            let (added, removed) = updated_core.apply(patch); // TODO: Use these

            if find_id(&updated_core, self.uuid()).is_some() {
                if MembershipHash::new(&updated_core) != message.membership_hash {
                    // Definitely unclean update; memberhsip hash mismatch
                    todo!()
                } else {
                    // Assume clean
                    // TODO: Are there edge cases?
                    self.core_map = updated_core;
                    Some(self.update_message(&message.timestamp))
                }
            } else {
                // Definitely unclean update; removed self
                todo!()
            }
        } else {
            if &message.timestamp == self.timestamp() {
                let membership_hash = MembershipHash::new(&self.core_map);
                if membership_hash == message.membership_hash {
                    None
                } else {
                    match message.unique_count.cmp(&self.unique_count()) {
                        Ordering::Greater => Some(self.new_member_message()),
                        Ordering::Less => Some(self.heartbeat_message()),
                        Ordering::Equal => {
                            if message.membership_hash > membership_hash {
                                Some(self.new_member_message())
                            } else {
                                Some(self.heartbeat_message())
                            }
                        }
                    }
                }
            } else {
                Some(self.update_message(&message.timestamp))
            }
        }

        /*
        if message.membership_hash == membership_hash {
            match message.timestamp.partial_cmp(&self.timestamp()) {
                Some(Ordering::Greater) => {


                    Some(self.heartbeat_message())
                }
                Some(Ordering::Less) => {
                    Some(self.update_message(&message.timestamp))
                }
                Some(Ordering::Equal) => {
                    None
                }
                None => {


                    Some(self.update_message(&message.timestamp))
                }
            }
        } else {
            todo!()
        }
        */
    }

    /// Apply a CLEAN update with checks.
    fn apply_update(&mut self, message: PollinationMessage<A>) -> Option<PollinationMessage<A>> {
        todo!()
        /*
        // TODO: inefficient clone
        let core_copy = self.core_map.clone();

        if let Some(patch) = message.patch {
            core_copy.apply(patch.clone());

            if MembershipHash::new(&core_copy) != message.membership_hash {
                // Attempt a fresh patch
                let core_new = ItcMap::from_patch(patch.clone());
                if let Some(id) = find_id(core_new, self.uuid()) {
                }
            } else {
                // MembershipHash is equal, but this could be because our tree has been completely
                // overridden. So, check to see if we're in it.
                if let Some(id) = find_id(core_new, self.uuid()) {
                } else {
                    // We are not in the map, so now check to see if we should request to be
                    // included in the other map.
                    match message.unique_count.cmp(self.unique_count()) {
                        Ordering::Greater => {
                            self.include_me_message()
                        }
                        Ordering::Less => {
                            self.update_message()
                        }
                    }
                }
            }
        } else {
            false
        }
        //core_copy.apply(message.patch
        */
    }
}

fn find_id<A>(map: &ItcMap<NodeInfo<A>>, uuid: Uuid) -> Option<IdTree> {
    map.iter()
        .find(|(_, info)| info.uuid == uuid)
        .map(|(id, _)| id)
        .cloned()
}

// PollinationMessage

pub struct PollinationMessage<A> {
    uuid: Uuid,
    id: IdTree,
    timestamp: EventTree,
    membership_hash: MembershipHash,
    unique_count: usize,
    patch: Option<Patch<NodeInfo<A>>>,
    request_membership: bool,
}

// NodeInfo

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeStatus {
    Healthy,
    Dead,
}

// MembershipHash
use simplehash::fnv1a_64;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
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
