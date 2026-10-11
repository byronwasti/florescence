use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};
use thiserror::Error;
use tracing::{debug, error, info};
use treeclocks::{EventTree, IdTree, ItcMap, Patch, itc_map::tree::ItcMapTree};
use uuid::Uuid;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PollinationCore<A> {
    id: IdTree,
    core_map: ItcMap<NodeInfo<A>>,
    origin_hash: OriginHash,
    own_info: NodeInfo<A>,
}

impl<A> PollinationCore<A> {
    pub fn membership_hash(&self) -> MembershipHash {
        // TODO: Efficiency
        MembershipHash::from_itc_map(&self.core_map)
    }

    pub fn origin_hash(&self) -> OriginHash {
        self.origin_hash
    }
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
        let origin_hash = OriginHash::from_membership_hash(MembershipHash::from_itc_map(&core_map));
        Self {
            id,
            core_map,
            origin_hash,
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

    /// Increment the logical timestamp associated with this nodes data.
    pub fn increment(&mut self) {
        self.own_info.timestamp += 1;
        let removals = self.insert(self.id.clone(), self.own_info().clone());
        assert_eq!(removals.len(), 1);
    }

    fn set_own_info(&mut self) {
        let removals = self.insert(self.id.clone(), self.own_info().clone());
        assert_eq!(removals.len(), 1);
    }

    // Insert value at IdTree location, returning removed IdTrees and their values.
    fn insert(&mut self, id: IdTree, value: NodeInfo<A>) -> Vec<(IdTree, NodeInfo<A>)> {
        self.core_map.insert(id, value)
    }

    #[tracing::instrument(skip_all)]
    pub fn heartbeat_message(&self) -> PollinationMessage<A> {
        info!("Heartbeat message");
        PollinationMessage {
            uuid: self.own_info.uuid.clone(),
            id: self.id.clone(),
            timestamp: self.core_map.timestamp().clone(),
            origin_hash: self.origin_hash(),
            membership_hash: self.membership_hash(),
            outcome_hash: None,
            unique_count: self.unique_count(),
            patch: None,
            full_patch: false,
            new_membership: NewMembership::None,
        }
    }

    // NOTE: Degrades to be heartbeat_message() if the timestamps are equal
    #[tracing::instrument(skip_all, fields(timestamp))]
    fn update_message(&self, timestamp: &EventTree) -> PollinationMessage<A> {
        info!("Update message");
        let mut msg = self.heartbeat_message();
        msg.patch = self.core_map.diff(timestamp);
        msg
    }

    #[tracing::instrument(skip_all)]
    fn full_update_message(&self) -> PollinationMessage<A> {
        info!("Full update message");
        let mut msg = self.update_message(&EventTree::new());
        msg.full_patch = true;
        msg
    }

    #[tracing::instrument(skip_all, fields(timestamp, outcome_hash))]
    fn check_update_message(
        &self,
        timestamp: &EventTree,
        outcome_hash: MembershipHash,
    ) -> PollinationMessage<A> {
        let mut msg = self.update_message(timestamp);
        msg.outcome_hash = Some(outcome_hash);
        msg
    }

    #[tracing::instrument(skip(self))]
    fn request_membership_message(&self) -> PollinationMessage<A> {
        info!("Request membership message");
        // Include all of our peers to be included as well
        let mut msg = self.full_update_message();
        msg.new_membership = NewMembership::Request;
        msg
    }

    #[tracing::instrument(skip(self))]
    fn response_membership_message(&self) -> PollinationMessage<A> {
        info!("Response membership message");
        let mut msg = self.full_update_message();
        msg.new_membership = NewMembership::Response;
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

    #[tracing::instrument(skip_all,fields(id=?self.addr()))]
    pub fn handle_message(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Option<PollinationMessage<A>> {
        debug!("SELF_DUMP={}", self);
        info!("Message from={}, ItcId={}", message.uuid, message.id);
        find_id(&self.core_map, self.uuid()).expect("Self to exist");

        // TODO: This doesn't work due to multi-inclusion
        /*
        assert_eq!(
            &find_id(&self.core_map, self.uuid()).expect("Self to exist"),
            &self.id
        );
        */

        match self.apply_patch(message.clone()) {
            Ok(()) => {
                if matches!(
                    self.timestamp().partial_cmp(&message.timestamp),
                    Some(Ordering::Greater) | None
                ) {
                    Some(self.update_message(&message.timestamp))
                } else {
                    Some(self.heartbeat_message())
                }
            }
            Err(ApplyPatchError::NoPatch) => self.handle_no_patch_message(message),
            Err(ApplyPatchError::UncleanMembership(hash)) if !message.full_patch => {
                error!(
                    "Unable to apply the patch due to unclean membership update; might still be okay its a partial"
                );

                Some(self.check_update_message(&message.timestamp, hash))
            }
            Err(err) => {
                error!("Unable to apply the patch: {err:?}");

                if message.full_patch {
                    info!("Full patch, evaluating...");
                    let new_core = ItcMap::from_patch(message.patch.unwrap().clone());

                    let comparison = compare_itc_maps(&self.core_map, &new_core);
                    if comparison > 0
                        || (comparison == 0 && self.membership_hash() < message.membership_hash)
                    {
                        info!("Peer membership is better");
                        if let Some(new_id) = find_id(&new_core, self.uuid()) {
                            info!("Switching memberships");
                            assert!(
                                new_core.get(&new_id).unwrap().timestamp
                                    <= self.own_info().timestamp
                            );
                            self.core_map = new_core;
                            self.id = new_id;
                            self.increment();
                            self.origin_hash = message.origin_hash;
                            Some(self.update_message(&message.timestamp))
                        } else {
                            Some(self.request_membership_message())
                        }
                    } else {
                        info!("Self membership is better");
                        if matches!(message.new_membership, NewMembership::Request) {
                            info!("New membership requested");
                            let mut peers = vec![];
                            for (_, node) in new_core.iter() {
                                // TODO: Horribly inefficient; does it matter?
                                if find_id(&self.core_map, node.uuid).is_some() {
                                    continue;
                                }

                                peers.push(node.clone());
                            }

                            if peers.is_empty() {
                                info!("No peers to add");
                                Some(self.heartbeat_message())
                            } else {
                                info!("Peers to add; sending provide member message");
                                self.add_peers(peers);
                                Some(self.response_membership_message())
                            }
                        } else {
                            info!("Peer hasn't requested membership");
                            Some(self.full_update_message())
                        }
                    }
                } else {
                    info!("Partial patch");
                    Some(self.full_update_message())
                }
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn handle_no_patch_message(
        &self,
        message: PollinationMessage<A>,
    ) -> Option<PollinationMessage<A>> {
        if message.origin_hash != self.origin_hash() {
            return Some(self.full_update_message());
        }

        match self.timestamp().partial_cmp(&message.timestamp) {
            Some(Ordering::Greater) | None => {
                // We have some information peer doesn't have; sync before doing anything
                Some(self.update_message(&message.timestamp))
            }
            Some(Ordering::Less) => {
                // We need information
                Some(self.heartbeat_message())
            }
            Some(Ordering::Equal) => {
                if &self.membership_hash() != &message.membership_hash {
                    // We know there is a skew; send all the information we know.
                    Some(self.full_update_message())
                } else {
                    // We're all synced up; nothing to do
                    None
                }
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn apply_patch_new(
        &mut self,
        message: PollinationMessage<A>,
    ) -> std::result::Result<(), ApplyPatchError> {
        let Some(patch) = message.patch else {
            return Err(ApplyPatchError::NoPatch);
        };

        let peer_map = ItcMap::from_patch(patch);
        let peer_map_tree = peer_map.tree();
        let self_map_tree = self.core_map.tree();

        let _res = compare_trees(&self_map_tree, &peer_map_tree);

        todo!()
    }

    // Attempt to apply a Patch, and unsuccessful will return relevant issue in application
    #[tracing::instrument(skip_all)]
    fn apply_patch(
        &mut self,
        message: PollinationMessage<A>,
    ) -> std::result::Result<(), ApplyPatchError> {
        let Some(patch) = message.patch else {
            return Err(ApplyPatchError::NoPatch);
        };

        if message.origin_hash != self.origin_hash {
            return Err(ApplyPatchError::OriginMismatch);
        }

        // First, apply the patch to the core (safely, to a clone)
        // and gather up as much metadata about the applied patch.
        let mut patched_core = self.core_map.clone();
        let (added, removed) = patched_core.apply(patch);
        let net_added = analyze_added_removed(&added, &removed);
        let patched_membership_hash = MembershipHash::from_itc_map(&patched_core);

        // TODO: Fix this check. Sometimes the patch just has older values.
        let clean_membership = patched_membership_hash == message.membership_hash
            || Some(patched_membership_hash) == message.outcome_hash;
        //|| patched_membership_hash == self.membership_hash();
        let (is_present, same_id, same_info) = if let Some(id) = find_id(&patched_core, self.uuid())
        {
            // Some quick checks and asserts
            let value = patched_core.get(&id).expect("ID to be present");
            assert_eq!(value.uuid, self.own_info.uuid);
            assert!(self.own_info.timestamp >= value.timestamp);
            (
                true,
                id == self.id,
                self.own_info.timestamp >= value.timestamp,
            )
        } else {
            (false, false, false)
        };

        // Second, try to account for every edge case and fail miserably.
        match (clean_membership, is_present, same_id, same_info) {
            (false, ..) => Err(ApplyPatchError::UncleanMembership(patched_membership_hash)),
            (_, false, ..) => Err(ApplyPatchError::NotPresentInNew),
            (true, true, false, ..) => Err(ApplyPatchError::DifferingId),
            (true, true, true, false) => {
                // A skew. Same ID, but different value means the patch had a higher EventTree but a lower node-Timestamp,
                // which means this node has swapped trees. Hashes should differ.
                assert_ne!(self.membership_hash(), patched_membership_hash);
                Err(ApplyPatchError::ReducedSelfTimestamp)
            }
            (true, true, true, true) => {
                if net_added >= 0 {
                    info!("Clean update");
                } else {
                    // NOTE: Technically it doesn't _lose_ information if we remove 5 dead nodes with a single node update,
                    // but `net_added` will still be negative in that case. So not totally dirty
                    info!(
                        "Dirty update, lost information ({}); still applied it",
                        net_added
                    )
                }

                // Clean update from our perspective, with net new information; so just take it
                self.core_map = patched_core;

                Ok(())
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
            "{:?}::{} timestamp={}, id={}, membership_hash={:?}, map={}",
            self.own_info.addr,
            self.own_info.uuid,
            self.core_map.timestamp(),
            &self.id,
            self.membership_hash(),
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
    let entries_b = map_b.iter().map(|(_, d)| d.uuid).collect::<HashSet<_>>();

    map_a
        .iter()
        .filter_map(|(_, d)| {
            if !entries_b.contains(&d.uuid) {
                Some(d.to_owned())
            } else {
                None
            }
        })
        .collect()
}

fn unique_diff_count<A>(map_a: &ItcMap<NodeInfo<A>>, map_b: &ItcMap<NodeInfo<A>>) -> (i64, i64) {
    let entries_a = map_a.iter().map(|(_, d)| d.uuid).collect::<HashSet<_>>();
    let entries_b = map_b.iter().map(|(_, d)| d.uuid).collect::<HashSet<_>>();

    let diff_a = map_a
        .iter()
        .filter(|(_, d)| !entries_b.contains(&d.uuid))
        .count() as i64;
    let diff_b = map_b
        .iter()
        .filter(|(_, d)| !entries_a.contains(&d.uuid))
        .count() as i64;

    (diff_a, diff_b)
}

fn analyze_added_removed<A: std::fmt::Debug>(
    added: &[(IdTree, &NodeInfo<A>)],
    removed: &[(IdTree, NodeInfo<A>)],
) -> i64 {
    for (id, node) in added {
        info!("Added: {id} -> {node}");
    }
    for (id, node) in removed {
        info!("Removed: {id} -> {node}");
    }

    let entries_added = added
        .iter()
        .map(|(_, d)| (d.uuid, d.timestamp))
        .collect::<HashMap<_, _>>();
    let entries_removed = added
        .iter()
        .map(|(_, d)| (d.uuid, d.timestamp))
        .collect::<HashMap<_, _>>();

    let added_info: i64 = entries_added
        .iter()
        .filter_map(|(uuid, timestamp_added)| {
            if let Some(timestamp_removed) = entries_removed.get(&uuid) {
                if timestamp_added > timestamp_removed {
                    Some(1)
                } else {
                    None
                }
            } else {
                Some(1)
            }
        })
        .sum();

    let removed_info: i64 = entries_removed
        .iter()
        .filter_map(|(uuid, timestamp_removed)| {
            if let Some(timestamp_added) = entries_added.get(&uuid) {
                if timestamp_removed > timestamp_added {
                    Some(1)
                } else {
                    None
                }
            } else {
                Some(1)
            }
        })
        .sum();

    added_info - removed_info
}

fn compare_itc_maps<A>(a: &ItcMap<NodeInfo<A>>, b: &ItcMap<NodeInfo<A>>) -> i64 {
    let a_map = itc_map_to_hash_map(&a);
    let b_map = itc_map_to_hash_map(&b);

    let mut merge = HashMap::new();
    for (uuid, ts) in a_map.iter() {
        merge.insert(uuid, (Some(ts), None));
    }
    for (uuid, ts) in b_map.iter() {
        if let Some(v) = merge.get_mut(uuid) {
            v.1 = Some(ts)
        } else {
            merge.insert(uuid, (None, Some(ts)));
        }
    }

    merge.values().fold(0, |acc, (a, b)| match (a, b) {
        (Some(_a), None) => acc - 1,
        (None, Some(_b)) => acc + 1,
        (Some(a), Some(b)) if a > b => acc - 1,
        (Some(a), Some(b)) if a < b => acc + 1,
        _ => acc,
    })
}

fn itc_map_to_hash_map<A>(itc_map: &ItcMap<NodeInfo<A>>) -> HashMap<Uuid, u64> {
    let mut map = HashMap::new();
    itc_map.iter().map(|(_, d)| {
        if let Some(v) = map.get_mut(&d.uuid) {
            if d.timestamp > *v {
                *v = d.timestamp;
            }
        } else {
            map.insert(d.uuid, d.timestamp);
        }
    });

    map
}

fn compare_trees<A>(own_map: &ItcMapTree<A>, peer_map: &ItcMapTree<A>) -> bool {
    use ItcMapTree::*;
    match (own_map, peer_map) {
        (Unknown, _) => panic!("Own map contains ItcMapTree::Unknown"),
        (_own, Unknown) => todo!(),

        (
            Leaf {
                id_tree: _id0,
                timestamp: _ts0,
                value: _v0,
            },
            Leaf {
                id_tree: _id1,
                timestamp: _ts1,
                value: _v1,
            },
        ) => {
            todo!()
        }

        (a @ Leaf { .. }, SubTree(l, r)) => {
            let _l = compare_trees(a, l);
            let _r = compare_trees(a, r);

            todo!()
        }

        (SubTree(l, r), b @ Leaf { .. }) => {
            let _l = compare_trees(l, b);
            let _r = compare_trees(r, b);

            todo!()
        }

        (SubTree(l0, r0), SubTree(l1, r1)) => {
            let _l = compare_trees(l0, l1);
            let _r = compare_trees(r0, r1);

            todo!()
        }
    }
}

// PollinationMessage

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollinationMessage<A> {
    pub uuid: Uuid,
    pub id: IdTree,
    pub timestamp: EventTree,
    pub origin_hash: OriginHash,
    pub membership_hash: MembershipHash,
    pub outcome_hash: Option<MembershipHash>,
    pub unique_count: usize,
    pub patch: Option<Patch<NodeInfo<A>>>,
    pub new_membership: NewMembership,
    // TODO: Derive from Patch
    pub full_patch: bool,
}

impl<A: std::fmt::Debug> std::fmt::Display for PollinationMessage<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(
            f,
            "PollinationMessage {{ uuid={} id={} timestamp={} origin_hash={} membership_hash={} unique_count={} new_membership={:?} full={} patch={} }}",
            self.uuid,
            self.id,
            self.timestamp,
            self.origin_hash.0,
            self.membership_hash.0,
            self.unique_count,
            self.new_membership,
            self.full_patch,
            if let Some(patch) = &self.patch {
                format!("{patch}")
            } else {
                "None".to_string()
            },
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NewMembership {
    None,
    Request,
    Response,
}

impl NewMembership {
    fn is_none(&self) -> bool {
        matches!(self, NewMembership::None)
    }

    fn is_request(&self) -> bool {
        matches!(self, NewMembership::Request)
    }

    fn is_response(&self) -> bool {
        matches!(self, NewMembership::Response)
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

#[derive(Debug, Error)]
pub enum ApplyPatchError {
    #[error("Incorrect membership_hash value; matches neither self nor other: {0:?}")]
    UncleanMembership(MembershipHash),

    #[error("Self not present")]
    NotPresentInNew,

    #[error("Self has differing ID")]
    DifferingId,

    #[error("Self timestamp is changed")]
    ReducedSelfTimestamp,

    #[error("No patch present in the message")]
    NoPatch,

    #[error("Origin mismatch")]
    OriginMismatch,
}

// NodeInfo

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(
    PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy, Serialize, Deserialize, Hash, Default,
)]
pub struct MembershipHash(u64);

impl MembershipHash {
    fn from_itc_map<A>(itc_map: &ItcMap<NodeInfo<A>>) -> Self {
        itc_map.map_recursive(
            &|node: &NodeInfo<A>| Self::hash(node.uuid.as_bytes()),
            &|l, r| Self::combine(l, r),
        )
    }

    fn hash(bytes: &[u8]) -> Self {
        Self(fnv1a_64(bytes))
    }

    fn combine(left: MembershipHash, right: MembershipHash) -> Self {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&left.0.to_le_bytes());
        bytes[8..].copy_from_slice(&right.0.to_le_bytes());
        Self(fnv1a_64(&bytes))
    }

    pub fn u64(&self) -> u64 {
        self.0
    }
}

#[derive(
    PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy, Serialize, Deserialize, Hash, Default,
)]
pub struct OriginHash(u64);

impl OriginHash {
    fn from_membership_hash(mh: MembershipHash) -> Self {
        Self(mh.0)
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

        let h0 = MembershipHash::from_itc_map(&m0);
        let h1 = MembershipHash::from_itc_map(&m1);

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
