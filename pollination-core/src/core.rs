use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::Debug,
};
use thiserror::Error;
use tracing::{debug, error, info, warn};
use treeclocks::{EventTree, IdTree, ItcMap, Patch, itc_map::ItcMapTree};
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
    A: Clone + for<'a> Deserialize<'a> + Serialize + Debug + PartialEq,
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

    // Insert value at IdTree location, returning removed IdTrees and their values.
    fn insert(&mut self, id: IdTree, value: NodeInfo<A>) -> Vec<(IdTree, NodeInfo<A>)> {
        let prev_ts = self.timestamp().clone();
        let removals = self.core_map.insert(id, value);
        assert_ne!(prev_ts.to_string(), self.timestamp().to_string());
        removals
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
            unique_count: self.unique_count(),
            patch: None,
            full_patch: false,
            new_membership: false,
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

    #[tracing::instrument(skip_all, fields(corrupt_id))]
    fn corrupt_id_message(&self, corrupt_id: IdTree) -> PollinationMessage<A> {
        // TODO: This should probably be an update_message
        let mut msg = self.heartbeat_message();
        msg.timestamp = msg.timestamp.mask_out(&corrupt_id);
        msg
    }

    #[tracing::instrument(skip(self))]
    fn request_membership_message(&self) -> PollinationMessage<A> {
        info!("Request membership message");
        // Include all of our peers to be included as well
        let mut msg = self.full_update_message();
        msg.new_membership = true;
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
        info!("Self Origin={:?} timestamp={}", self.origin_hash(), self.timestamp());
        info!("Message Origin={:?} timestamp={}", message.origin_hash, message.timestamp);
        find_id(&self.core_map, self.uuid()).expect("Self to exist");

        if message.origin_hash != self.origin_hash() {
            return self.handle_origin_mismatch_message(message);
        }

        let Some(patch) = message.patch else {
            return self.handle_no_patch_message(message);
        };

        match self.apply_patch(patch) {
            Ok(()) => match self.timestamp().partial_cmp(&message.timestamp) {
                Some(Ordering::Greater) | None => Some(self.update_message(&message.timestamp)),
                Some(Ordering::Equal) => {
                    if self.membership_hash() != message.membership_hash {
                        Some(self.full_update_message())
                    } else {
                        None
                    }
                }
                Some(Ordering::Less) => {
                    error!(
                        "Incomplete update. {} != {}",
                        self.timestamp(),
                        message.timestamp
                    );
                    panic!("Timestamp is less than message timestamp; patch was incomplete");
                }
            },
            Err(corrupt_id) => Some(self.corrupt_id_message(corrupt_id)),
        }
    }

    #[tracing::instrument(skip_all)]
    fn handle_no_patch_message(
        &self,
        message: PollinationMessage<A>,
    ) -> Option<PollinationMessage<A>> {
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
    fn handle_origin_mismatch_message(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Option<PollinationMessage<A>> {
        match message.unique_count.cmp(&self.unique_count()) {
            Ordering::Less => {
                if message.new_membership {
                    self.handle_request_membership_message(message)
                } else {
                    Some(self.heartbeat_message())
                }
            }
            Ordering::Equal => {
                if message.origin_hash > self.origin_hash() {
                    self.handle_take_new_origin(message)
                } else {
                    if message.new_membership {
                        self.handle_request_membership_message(message)
                    } else {
                        Some(self.heartbeat_message())
                    }
                }
            }
            Ordering::Greater => self.handle_take_new_origin(message),
        }
    }

    #[tracing::instrument(skip_all)]
    fn handle_request_membership_message(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Option<PollinationMessage<A>> {
        assert!(message.full_patch);
        if self.add_peers_from_patch(message.patch.expect("Patch should be present")) {
            Some(self.heartbeat_message())
        } else {
            Some(self.full_update_message())
        }
    }

    #[tracing::instrument(skip_all)]
    fn handle_take_new_origin(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Option<PollinationMessage<A>> {
        if !message.full_patch {
            Some(self.request_membership_message())
        } else {
            let new_core = ItcMap::from_patch(message.patch.expect("Expected patch to be present"));
            if let Some(new_id) = find_id(&new_core, self.uuid()) {
                // TODO: Also add any new peers at this point

                self.id = new_id;
                self.core_map = new_core;
                self.origin_hash = message.origin_hash;
                let old_timestamp = self.timestamp().clone();
                self.increment();
                assert_ne!(self.timestamp(), &old_timestamp);
                Some(self.update_message(&message.timestamp))
            } else {
                Some(self.request_membership_message())
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn apply_patch(&mut self, patch: Patch<NodeInfo<A>>) -> std::result::Result<(), IdTree> {
        let own_map_tree = self.core_map.to_itc_map_tree();
        let peer_map = ItcMap::from_patch(patch);
        let peer_map_tree = peer_map.to_itc_map_tree();
        info!("Own map tree: {own_map_tree:?}");
        info!("Peer map tree: {peer_map_tree:?}");
        let mut outcome = apply_patch(&own_map_tree, &peer_map_tree);
        let corrupt_id = outcome.corrupt_id.take();

        if let Some(corrupt_id) = corrupt_id {
            warn!("Corrupt ID in patch application; Skipping entire update");
            return Err(corrupt_id);
        }

        let mut all_ids = IdTree::zero();
        let mut inserts = outcome
            .insert
            .drain(..)
            .map(|(id, val)| (id, val.clone()))
            .collect::<Vec<_>>();
        for (id, val) in inserts.drain(..) {
            all_ids = all_ids.join(id.clone());
            self.core_map.insert_without_event(id, val.clone());
        }

        let new_timestamp = self.timestamp().clone().join(peer_map.timestamp().clone());
        let new_ts_diff = new_timestamp.clone().diff(&self.timestamp());
        if all_ids != IdTree::zero() && !new_ts_diff.contains(&all_ids) {
            panic!("New TS doesn't contain all updates: new_ts_diff={new_ts_diff}, all_ids={all_ids}");
        }
        self.core_map.set_timestamp(new_timestamp);
        info!("Clean timestamp update");
        Ok(())
    }

    /// Returns whether or not peers were added
    fn add_peers_from_patch(&mut self, patch: Patch<NodeInfo<A>>) -> bool {
        // TODO: Efficiency
        let peer_map = ItcMap::from_patch(patch);

        let mut peers = vec![];
        for (_, node) in peer_map.iter() {
            // TODO: Horribly inefficient; does it matter?
            if find_id(&self.core_map, node.uuid).is_some() {
                continue;
            }

            peers.push(node.clone());
        }

        let prev_timestamp = self.timestamp().clone();
        let prev_id = self.id().clone();
        if peers.is_empty() {
            info!("No peers to add");
            false
        } else {
            info!("Peers to add; sending provide member message");
            self.add_peers(peers);
            info!("prev_id={prev_id} prev_ts={prev_timestamp} new_ts={}", self.timestamp());
            // TODO: This assert doesn't work but it does point to an optimization.
            // Raising the timestamp individually is less efficient than incrementing it for all at
            // once (more fill opportunities when its all together). Given Ideal IDs there should be
            // no problem with this.
            //assert_eq!(self.timestamp(), &prev_timestamp.event(&prev_id));
            true
        }
    }

    // Evenly divide the Self address space and distribute to peers. Then, assign every nodes
    // information to the map.
    fn add_peers(&mut self, mut peers: Vec<NodeInfo<A>>) {
        info!("Adding {} peers: {peers:?}", peers.len());
        info!("Starting id={} ts={}", self.id(), self.timestamp());
        let mut new_ids = self.id.clone().fork_many(peers.len() + 1);
        self.id = new_ids[0].clone();
        self.increment();
        info!("new_id={} new_ts={}", self.id(), self.timestamp());
        //self.insert(new_ids[0].clone(), self.own_info.clone());

        assert_eq!(new_ids.len(), peers.len() + 1);

        for (new_id, info) in new_ids.drain(1..).zip(peers.drain(..)) {
            let removals = self.insert(new_id.clone(), info);
            info!("peer_id={new_id} new_ts={}", self.timestamp());
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

#[derive(Clone, Debug, PartialEq)]
struct PatchOutcome<'a, A> {
    insert: Vec<(IdTree, &'a NodeInfo<A>)>,
    // NOTE: This is a "compound" ID, not an Ideal IDTree
    corrupt_id: Option<IdTree>,
}

impl<'a, A> PatchOutcome<'a, A> {
    fn new(id: IdTree, node_info: &'a NodeInfo<A>) -> Self {
        let insert = vec![(id, node_info)];
        PatchOutcome {
            insert,
            ..Default::default()
        }
    }

    fn corrupt(id: IdTree) -> Self {
        PatchOutcome {
            corrupt_id: Some(id),
            ..Default::default()
        }
    }

    fn merge(mut self, mut other: PatchOutcome<'a, A>) -> Self {
        self.insert.append(&mut other.insert);
        self.corrupt_id = match (self.corrupt_id, other.corrupt_id) {
            (None, None) => None,
            (Some(v0), None) => Some(v0),
            (None, Some(v1)) => Some(v1),
            (Some(v0), Some(v1)) => Some(v0.join(v1)),
        };

        self
    }
}

impl<'a, A> Default for PatchOutcome<'a, A> {
    fn default() -> Self {
        PatchOutcome {
            insert: vec![],
            corrupt_id: None,
        }
    }
}

fn apply_patch<'a, A>(
    own_map: &'a ItcMapTree<NodeInfo<A>>,
    peer_map: &'a ItcMapTree<NodeInfo<A>>,
) -> PatchOutcome<'a, A>
where
    A: PartialEq + Debug,
{
    use ItcMapTree::*;

    match (own_map, peer_map) {
        (Unknown, _) => panic!("Own map contains ItcMapTree::Unknown"),
        (_, Unknown) => PatchOutcome::default(),

        (
            Leaf {
                id_tree: id0,
                timestamp: ts0,
                value: v0,
            },
            Leaf {
                id_tree: id1,
                timestamp: ts1,
                value: v1,
            },
        ) => {
            info!("id0={id0} id1={id1} ts0={ts0} ts1={ts1} v0={v0:?} v1={v1:?}");
            match ts0.cmp(ts1) {
                Ordering::Greater => {
                    info!("Self contains a greater timestamp, ignoring patch");
                    PatchOutcome::default()
                }
                Ordering::Less => PatchOutcome::new((*id1).clone(), v1),
                Ordering::Equal => {
                    if v0.uuid != v1.uuid {
                        // Panicking for now because we don't have removal of peers yet, so this
                        // should never happen. In the future we just take the greater
                        warn!(
                            "UUID skew between equal peers. Indicates a partition event occurred."
                        );
                        panic!("TODO: Remove panic");
                    } else {
                        // TODO: Both of these asserts _should_ apply, but unfortunately
                        // don't during convergence. Need to figure out a way to make them work.
                        //assert_eq!(id0, id1);
                        //assert_eq!(v0, v1);
                        PatchOutcome::default()
                    }
                }
            }
        }

        (a @ Leaf { id_tree, .. }, SubTree(l, r)) => {
            let l = apply_patch(a, l);
            let r = apply_patch(a, r);

            let outcome = l.merge(r);

            if outcome.corrupt_id.is_some() {
                PatchOutcome::corrupt((*id_tree).clone())
            } else if !outcome.insert.is_empty() {
                let updated_id_tree = outcome
                    .insert
                    .iter()
                    .fold(IdTree::zero(), |acc, (id, _)| acc.join(id.clone()));

                // Either we have updates for everything, or we don't
                // TODO: This check doesn't work; we need to keep track of where we're at in the
                // tree for this to work properly
                /*
                if updated_id_tree != **id_tree {
                    warn!("Corrupt patch at {id_tree}, only updated {updated_id_tree}");
                    PatchOutcome::corrupt((*id_tree).clone())
                } else {
                    outcome
                }
                */
                outcome
            } else {
                outcome
            }
        }

        (SubTree(l, r), b @ Leaf { id_tree, .. }) => {
            let l = apply_patch(l, b);
            let r = apply_patch(r, b);

            if l != r {
                // If the Leaf doesn't subsume all our data, then we're corrupt here
                warn!("Corrupt patch at {id_tree}");
                PatchOutcome::corrupt((*id_tree).clone())
            } else {
                l
            }
        }

        (SubTree(l0, r0), SubTree(l1, r1)) => {
            let l = apply_patch(l0, l1);
            let r = apply_patch(r0, r1);
            l.merge(r)
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
    pub unique_count: usize,
    pub patch: Option<Patch<NodeInfo<A>>>,
    pub new_membership: bool,
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
