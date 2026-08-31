use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    mem,
};
use thiserror::Error;
use tracing::{error, info};
use treeclocks::{EventTree, IdTree, ItcMap, Patch};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PollinationCore<A> {
    id: IdTree,
    core_map: ItcMap<NodeInfo<A>>,
    own_info: NodeInfo<A>,
}

impl<A> PollinationCore<A>
where
    A: Clone + for<'a> Deserialize<'a> + Serialize + std::fmt::Debug,
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

    pub fn addr(&self) -> &A {
        &self.own_info.addr
    }

    pub fn timestamp(&self) -> &EventTree {
        self.core_map.timestamp()
    }

    pub fn id(&self) -> &IdTree {
        &self.id
    }

    pub fn uuid(&self) -> Uuid {
        self.own_info.uuid
    }

    /// For diagnostic purposes
    pub fn own_info(&self) -> &NodeInfo<A> {
        &self.own_info
    }

    /// For diagnostic purposes
    pub fn core_map(&self) -> &ItcMap<NodeInfo<A>> {
        &self.core_map
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

    /// Increment the logical timestamp associated with this nodes data.
    pub fn increment(&mut self) {
        self.own_info.timestamp += 1;
        let removals = self.insert(self.id.clone(), self.own_info().clone());
        assert_eq!(removals.len(), 1);
    }

    #[tracing::instrument(level = "info", skip(self))]
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
    #[tracing::instrument(skip(self))]
    fn update_message(&self, timestamp: &EventTree) -> PollinationMessage<A> {
        let mut msg = self.heartbeat_message();
        msg.patch = self.core_map.diff(timestamp);
        msg
    }

    #[tracing::instrument(skip(self))]
    fn new_member_message(&self) -> PollinationMessage<A> {
        // Include all of our peers to be included as well
        let mut msg = self.update_message(&EventTree::new());
        msg.new_membership = NewMembership::Request;
        msg
    }

    #[tracing::instrument(skip(self))]
    fn provide_member_message(&self) -> PollinationMessage<A> {
        let mut msg = self.update_message(&EventTree::new());
        msg.new_membership = NewMembership::Provide;
        msg
    }

    #[tracing::instrument(skip(self))]
    pub fn recycle(&mut self) -> bool {
        info!("Recycling?");
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

    #[tracing::instrument(skip(self))]
    fn handle_skew(&self, message: PollinationMessage<A>) -> PollinationMessage<A> {
        // TODO: Swap compares to be self.compare(other)
        match message.unique_count.cmp(&self.unique_count()) {
            Ordering::Greater => {
                info!("Peer has greater unique_count; request membership");
                self.new_member_message()
            }
            Ordering::Less => {
                info!("Peer has lower unique_count; send update");
                self.update_message(&EventTree::new())
            }
            Ordering::Equal => {
                if message.membership_hash > self.membership_hash() {
                    info!("Membership hash of peer is greater; request membership");
                    self.new_member_message()
                } else {
                    info!("Membership hash of peer is lower; heartbeat");
                    self.heartbeat_message()
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    fn handle_new_members(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Result<Option<PollinationMessage<A>>> {
        if self.membership_hash() == message.membership_hash {
            info!("Membership_hash mismatch; bailing");
            return Ok(None);
        }

        // A PollinationMessage with NewMembership::Request can be assumed to have the full ItcMap
        // as part of the request.
        // TODO: Proper error handling
        let peer_map: ItcMap<NodeInfo<A>> =
            ItcMap::from_patch(message.patch.ok_or(PollinationError::NoPatch)?);

        let (a, b) = unique_diff_count(&self.core_map, &peer_map);
        if a < b {
            // NOTE: I think this can actually end up in a live-lock situation, but probably
            // too rare to be a problem.
            info!("New group has more unique; bailing");
            return Ok(None);
        }

        let mut peers = vec![];
        for (_, node) in peer_map.iter() {
            // TODO: Horribly inefficient; does it matter?
            if find_id(&self.core_map, node.uuid).is_some() {
                continue;
            }

            peers.push(node.clone());
        }

        if peers.is_empty() {
            info!("No peers to add");
            Ok(None)
        } else {
            info!("Peers to add; sending provide member message");
            self.add_peers(peers);
            Ok(Some(self.provide_member_message()))
        }
    }

    /// Handling a new core_map which has ourselves included.
    /// Take on a peers core_map; merge them
    // TODO: This name is horrible.
    #[tracing::instrument(skip(self))]
    fn handle_provided_membership(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Result<Option<PollinationMessage<A>>> {
        if self.membership_hash() == message.membership_hash {
            info!("Membership hash equality; bailing");
            return Ok(None);
        }

        let peer_map: ItcMap<NodeInfo<A>> =
            ItcMap::from_patch(message.patch.ok_or(PollinationError::NoPatch)?);

        let (a, b) = unique_diff_count(&self.core_map, &peer_map);
        if a > b {
            info!("More unique us; bailing");
            return Ok(None);
        }

        if let Some(new_id) = find_id(&peer_map, self.uuid()) {
            // Swap identities to the new map
            let mut new_self = PollinationCore {
                id: new_id,
                core_map: peer_map,
                own_info: self.own_info.clone(),
            };
            mem::swap(self, &mut new_self);
            let old_self = new_self;

            // Add peers not present in the current ItcMap to the Map
            let mut non_present = non_present(&old_self.core_map, &self.core_map);
            let mut new_ids = self.id.clone().fork_many(non_present.len() + 1);
            let mut new_ids = new_ids.drain(..);

            self.id = new_ids.next().expect("fork_many bug");
            let removed = self.core_map.insert(self.id.clone(), self.own_info.clone());
            assert_eq!(removed.len(), 1);

            for non_present in non_present.drain(..) {
                let id = new_ids.next().expect("fork_many bug");
                let removed = self.core_map.insert(id, non_present);
                assert_eq!(removed.len(), 0);
            }

            self.increment();

            info!("Merged with peers; sending update");
            return Ok(Some(self.update_message(&message.timestamp)));
        } else {
            info!("No self in new map; bailing");
            return Err(PollinationError::NoSelf);
        }
    }

    #[tracing::instrument(skip_all,fields(id=?self.addr()))]
    pub fn handle_message(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Option<PollinationMessage<A>> {
        info!("SELF_DUMP={}", self);
        assert_eq!(
            &find_id(&self.core_map, self.uuid()).expect("Self to exist"),
            &self.id
        );

        if message.new_membership.is_request() {
            match self.handle_new_members(message.clone()) {
                Ok(Some(msg)) => return Some(msg),
                Ok(None) => info!("Nothing to do for handling requested new membership"),
                Err(err) => {
                    error!("{err}");
                    // TODO: Remove panic
                    panic!("Bug present in requested membership route")
                }
            }
        }

        if message.new_membership.is_provide() {
            match self.handle_provided_membership(message.clone()) {
                Ok(Some(msg)) => return Some(msg),
                Ok(None) => info!("Nothing to do for handling provided membership"),
                Err(err) => {
                    error!("{err}");
                    // TODO: Remove panic
                    panic!("Bug present in provided membership route")
                }
            }
        }

        info!("Handling basic...");
        if let Some(patch) = message.patch.clone() {
            info!("Patch present in message.");
            let mut updated_core = self.core_map.clone();
            let (added, removed) = updated_core.apply(patch); // TODO: Use these

            if find_id(&updated_core, self.uuid()).is_some() {
                if MembershipHash::new(&updated_core) != message.membership_hash {
                    info!("Membership has mismatch");
                    // Definitely unclean update; memberhsip hash mismatch
                    Some(self.handle_skew(message))
                } else {
                    info!("Clean update; heartbeat");
                    // Assume clean
                    // TODO: Are there edge cases?
                    self.core_map = updated_core;
                    self.id = find_id(&self.core_map(), self.uuid()).expect("Self to be present");
                    //Some(self.update_message(&message.timestamp))
                    Some(self.heartbeat_message())
                }
            } else {
                info!("Removed self; unclean update");
                // Definitely unclean update; removed self
                Some(self.handle_skew(message))
            }
        } else {
            info!("No patch");
            if &message.timestamp == self.timestamp() {
                if self.membership_hash() == message.membership_hash {
                    info!("All matches; do nothing");
                    None
                } else {
                    info!("Handling skew.");
                    Some(self.handle_skew(message))
                }
            } else {
                // TODO: Inefficient; we should be comparing timestamps
                info!("Sending update");
                Some(self.update_message(&message.timestamp))
            }
        }
    }

    // Evenly divide the Self address space and distribute to peers. Then, assign every nodes
    // information to the map.
    #[tracing::instrument(skip(self))]
    fn add_peers(&mut self, mut peers: Vec<NodeInfo<A>>) {
        let mut new_ids = self.id.clone().fork_many(peers.len() + 1);
        self.id = new_ids[0].clone();
        self.insert(new_ids[0].clone(), self.own_info.clone());

        assert_eq!(new_ids.len(), peers.len() + 1);

        for (new_id, info) in new_ids.drain(1..).zip(peers.drain(..)) {
            let removals = self.insert(new_id, info);
            info!("Removing {} ids: {:?}", removals.len(), &removals);
            // There should be no removals, as inserting our own info should have removed ourselves
            // already.
            assert!(removals.is_empty());
        }
    }
}

impl<A: std::fmt::Debug> std::fmt::Display for PollinationCore<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        let map = self
            .core_map
            .iter()
            .map(|(id, info)| {
                /*
                format!(
                    "{} => {{ {:?}::{} timestamp={} status={:?} }}",
                    id, info.addr, info.uuid, info.timestamp, info.status
                )
                */
                format!("{id} => {info}")
            })
            .collect::<Vec<_>>();

        let map = map.join(", ");

        write!(
            f,
            "{:?}::{} timestamp={}, id={}, map={}",
            self.own_info.addr,
            self.own_info.uuid,
            self.core_map.timestamp(),
            &self.id,
            map,
        )
    }
}

fn find_id<A>(map: &ItcMap<NodeInfo<A>>, uuid: Uuid) -> Option<IdTree> {
    map.iter()
        .find(|(_, info)| info.uuid == uuid)
        .map(|(id, _)| id)
        .cloned()
}

fn non_present<A>(map_a: &ItcMap<NodeInfo<A>>, map_b: &ItcMap<NodeInfo<A>>) -> Vec<NodeInfo<A>>
where
    NodeInfo<A>: Clone,
{
    let entries_a = map_a
        .iter()
        .map(|(_, d)| (d.uuid, d))
        .collect::<HashMap<_, _>>();
    let entries_b = map_b
        .iter()
        .map(|(_, d)| (d.uuid, d))
        .collect::<HashMap<_, _>>();

    entries_a
        .iter()
        .filter_map(|(uuid, &d0)| {
            if !entries_b.contains_key(uuid) {
                Some((*d0).to_owned())
            } else {
                None
            }
        })
        .collect()
}

fn unique_diff_count<A>(map_a: &ItcMap<NodeInfo<A>>, map_b: &ItcMap<NodeInfo<A>>) -> (i64, i64) {
    let entries_a = map_a
        .iter()
        .map(|(_, d)| (d.uuid, d.timestamp))
        .collect::<HashMap<_, _>>();
    let entries_b = map_b
        .iter()
        .map(|(_, d)| (d.uuid, d.timestamp))
        .collect::<HashMap<_, _>>();

    let diff_a = entries_a
        .iter()
        .filter(|(d0, t0)| {
            if let Some(t1) = entries_b.get(d0) {
                *t0 > t1
            } else {
                true
            }
        })
        .count() as i64;
    let diff_b = entries_b
        .iter()
        .filter(|(d0, t0)| {
            if let Some(t1) = entries_a.get(d0) {
                *t0 > t1
            } else {
                true
            }
        })
        .count() as i64;

    (diff_a, diff_b)
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

impl<A: std::fmt::Debug> std::fmt::Display for PollinationMessage<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(
            f,
            "PollinationMessage {{ uuid={0} id={1} timestamp={2} membership_hash={3} unique_count={4} new_membership={5:?} patch={6} }}",
            self.uuid,
            self.id,
            self.timestamp,
            self.membership_hash.0,
            self.unique_count,
            self.new_membership,
            if let Some(patch) = &self.patch {
                format!("{patch}")
            } else {
                "None".to_string()
            },
        )
    }
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

    #[error("No self present in the ItcMap")]
    NoSelf,
}

pub type Result<T> = std::result::Result<T, PollinationError>;

// NodeInfo

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo<A> {
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

impl<A: std::fmt::Debug> std::fmt::Display for NodeInfo<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(
            f,
            "{{ {:?}::{} timestamp={} status={:?} }}",
            self.addr, self.uuid, self.timestamp, self.status
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum NodeStatus {
    Healthy,
    Dead,
}

// MembershipHash
use simplehash::fnv1a_64;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy, Serialize, Deserialize, Hash)]
pub struct MembershipHash(u64);

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
    use uuid::uuid;

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

    #[test]
    fn test_basic_sync_0() {
        let mut pc0 = PollinationCore::new(uuid!("6008eb2a-7387-41d6-976d-6c66904d19c6"), 0);
        let mut pc1 = PollinationCore::new(uuid!("eb06c0df-c501-4800-852e-a9f619cd8163"), 1);

        let msg = pc0.heartbeat_message();
        println!("{msg}");
        let msg = pc1.handle_message(msg).expect("Message");
        println!("{msg}");
        let msg = pc0.handle_message(msg).expect("Message");
        println!("{msg}");
        let msg = pc1.handle_message(msg).expect("Message");
        println!("{msg}");
        let msg = pc0.handle_message(msg).expect("Message");
        println!("{msg}");
        let msg = pc1.handle_message(msg).expect("Message");
        println!("{msg}");
    }
}
