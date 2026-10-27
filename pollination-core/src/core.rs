use crate::{
    membership_hash::MembershipHash,
    message::PollinationMessage,
    node_info::{NodeInfo, NodeStatus},
    origin_hash::OriginHash,
};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::Debug,
};
use tracing::{debug, error, info, warn};
use treeclocks::{EventTree, IdTree, ItcMap, Patch};
use uuid::Uuid;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PollinationCore<A> {
    id: IdTree,
    core_map: ItcMap<NodeInfo<A>>,
    origin_hash: OriginHash,
    own_info: NodeInfo<A>,
    // TODO: This is pretty gross, but necessary
    prev_store: HashMap<OriginHash, (IdTree, u64)>,
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
            prev_store: HashMap::new(),
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
    #[tracing::instrument(skip_all)]
    pub fn increment(&mut self) {
        let _removals = self.increment_inner();

        //assert_eq!(removals.len(), 1);
    }

    // Unchecked `increment()` for use with recycling logic
    fn increment_inner(&mut self) -> Vec<(IdTree, NodeInfo<A>)> {
        self.own_info.timestamp += 1;
        let removals = self.insert(self.id.clone(), self.own_info().clone());
        for (id, data) in &removals {
            debug!("Removing {id} value={data}")
        }
        removals
    }

    // Insert value at IdTree location, returning removed IdTrees and their values.
    fn insert(&mut self, id: IdTree, value: NodeInfo<A>) -> Vec<(IdTree, NodeInfo<A>)> {
        let prev_ts = self.timestamp().clone();
        let removals = self.core_map.insert(id, value);
        assert_ne!(&prev_ts, self.timestamp());
        removals
    }

    /** Messages **/

    #[tracing::instrument(skip_all)]
    pub fn heartbeat_message(&self) -> PollinationMessage<A> {
        info!("Heartbeat message");
        PollinationMessage {
            uuid: self.own_info.uuid,
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

    #[tracing::instrument(skip(self))]
    fn request_membership_message(&self) -> PollinationMessage<A> {
        info!("Request membership message");
        // Include all of our peers to be included as well
        let mut msg = self.full_update_message();
        msg.new_membership = true;
        msg
    }

    /** Message Handling **/

    #[tracing::instrument(skip_all,fields(id=?self.addr()))]
    pub fn handle_message(
        &mut self,
        message: PollinationMessage<A>,
    ) -> Option<PollinationMessage<A>> {
        debug!("SELF_DUMP={}", self);
        info!("Message from={}, ItcId={}", message.uuid, message.id);
        info!(
            "Self Origin={:?} timestamp={}",
            self.origin_hash(),
            self.timestamp()
        );
        info!(
            "Message Origin={:?} timestamp={}",
            message.origin_hash, message.timestamp
        );
        debug!("MESSAGE_DUMP={}", message);
        find_first_valid_id(&self.core_map, self.uuid()).expect("Self to exist");

        // TODO: core_map can be incomplete; definitely need a cache system or something
        //assert!(self.core_map.is_complete());

        // TODO: Is there a better place this should go? I feel like in-line is totally fine
        self.recycle();

        if message.origin_hash != self.origin_hash() {
            return self.handle_origin_mismatch_message(message);
        }

        let Some(patch) = message.patch else {
            return self.handle_no_patch_message(message);
        };

        if self.apply_patch(patch) {
            match self.timestamp().partial_cmp(&message.timestamp) {
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
            }
        } else {
            // We only reject the patch when `Self` is overwritten. The membership_hash must be
            // inequal, as we never have a ItcMap without ourselves in it
            // TODO: This assert catches something every now and then
            //assert_ne!(message.membership_hash, self.membership_hash());
            if message.membership_hash > self.membership_hash() {
                Some(self.request_membership_message())
            } else {
                Some(self.full_update_message())
            }
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
                if self.membership_hash() != message.membership_hash {
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
            if let Some(new_id) = find_first_valid_id(&new_core, self.uuid()) {
                // TODO: Also add any new peers at this point
                let peers_missing: Vec<_> = {
                    let s: HashSet<Uuid> = new_core.iter().map(|(_, n)| n.uuid).collect();
                    self.core_map
                        .iter()
                        .filter_map(|(_, n)| {
                            if !s.contains(&n.uuid) {
                                Some(n.to_owned())
                            } else {
                                None
                            }
                        })
                        .collect()
                };

                let value = self.timestamp().clone().get(self.id()).max();
                self.prev_store
                    .insert(self.origin_hash, (self.id.clone(), value));
                self.id = new_id;
                self.core_map = new_core;
                self.origin_hash = message.origin_hash;
                let old_timestamp = self.timestamp().clone();

                // Use old max value iff it overlaps
                if let Some((remembered_id, val)) = self.prev_store.get(&self.origin_hash())
                    && self.id.contains(remembered_id)
                {
                    self.id = remembered_id.clone();
                    let ts = self.timestamp();
                    let new_ts = ts.clone().join(self.id.set_value(*val));
                    self.core_map.set_timestamp(new_ts);
                }

                self.add_peers(peers_missing);
                //self.increment();
                assert_ne!(self.timestamp(), &old_timestamp);
                Some(self.update_message(&message.timestamp))
            } else {
                Some(self.request_membership_message())
            }
        }
    }

    /** Misc. Helper methods **/

    #[tracing::instrument(skip_all)]
    fn apply_patch(&mut self, patch: Patch<NodeInfo<A>>) -> bool {
        let own_map_tree = self.core_map.to_itc_map_tree();
        debug!("Own map tree: {own_map_tree:?}");
        debug!("Peer Patch: {patch}");
        let peer_map = ItcMap::from_patch(patch);
        debug!("Peer map: {peer_map}");
        let peer_map_tree = peer_map.to_itc_map_tree();
        debug!("Peer map tree: {peer_map_tree:?}");
        let mut outcome = crate::apply_patch::apply_patch(&own_map_tree, &peer_map_tree);

        let mut all_ids = IdTree::zero();
        let mut inserts = outcome
            .insert
            .drain(..)
            .map(|(id, val)| (id, val.clone()))
            .collect::<Vec<_>>();

        let overlap = inserts.iter().any(|(id, _)| {
            //debug!("Comparing {id} with {}", self.id());
            id.overlaps(self.id())
        });

        if overlap {
            return false;
            /*
            let update_to_self = inserts
                .iter()
                .map(|(id, n)| (id, n.uuid, n.status))
                .find(|(id, uuid, status)| *uuid == self.uuid() && *status == NodeStatus::Healthy);
            if let Some((new_id, uuid, status)) = update_to_self {
                assert_eq!(uuid, self.uuid());

                if new_id == self.id() {
                    warn!("Self upsert, this isn't usual");
                } else {
                    return false;
                }
            } else {
                return false;
            }
            */
        }

        for (id, val) in inserts.drain(..) {
            debug!("Insert id={id} value={val}");
            all_ids = all_ids.join(id.clone());
            self.core_map.insert_without_event(id, val.clone());
        }

        let new_timestamp = self.timestamp().clone().join(peer_map.timestamp().clone());
        let new_ts_diff = new_timestamp.clone().diff(self.timestamp());
        if all_ids != IdTree::zero() && !new_ts_diff.contains(&all_ids) {
            // TODO: This maybe should be a panic?
            // XXX: I feel like this should panic
            error!(
                "New TS doesn't contain all updates: new_ts_diff={new_ts_diff}, all_ids={all_ids}"
            );
        }
        self.core_map.set_timestamp(new_timestamp);
        info!("Clean timestamp update");
        if !self.core_map.is_complete() {
            error!("Core_map is incomplete after patch; will crash");
        }

        true
    }

    /// Returns whether or not peers were added
    fn add_peers_from_patch(&mut self, patch: Patch<NodeInfo<A>>) -> bool {
        // TODO: Efficiency
        let peer_map = ItcMap::from_patch(patch);

        let mut peers = vec![];
        for (_, node) in peer_map.iter() {
            // TODO: Horribly inefficient; does it matter?
            if find_first_valid_id(&self.core_map, node.uuid).is_some() {
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
            info!(
                "prev_id={prev_id} prev_ts={prev_timestamp} new_ts={}",
                self.timestamp()
            );
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
        debug!("Starting id={} ts={}", self.id(), self.timestamp());
        let mut new_ids = self.id.clone().fork_many(peers.len() + 1);
        self.id = new_ids[0].clone();
        self.increment();
        debug!("new_id={} new_ts={}", self.id(), self.timestamp());
        //self.insert(new_ids[0].clone(), self.own_info.clone());

        assert_eq!(new_ids.len(), peers.len() + 1);

        for (new_id, info) in new_ids.drain(1..).zip(peers.drain(..)) {
            let removals = self.insert(new_id.clone(), info);
            debug!("peer_id={new_id} new_ts={}", self.timestamp());
            debug!("Removing {} ids: {:?}", removals.len(), &removals);
            // There should be no removals, as inserting our own info should have removed ourselves
            // already.
            assert!(removals.is_empty());
        }
    }

    /* Recycling */

    pub fn recycle(&mut self) -> bool {
        self.recycle_duplicate_self();
        let res = self.recycle_dead_peers();
        // TODO: Map is allowed to be incomplete, but maybe we check that it isn't made
        // more incomplete by this?
        //assert!(self.core_map.is_complete());
        res
    }

    fn recycle_duplicate_self(&mut self) {
        let mut to_remove = vec![];

        for (id, value) in self.core_map.iter() {
            if value.uuid == self.uuid() && id != self.id() && value.status != NodeStatus::Dead {
                to_remove.push((id.clone(), value.clone()));
            }
        }

        for (id, mut value) in to_remove.drain(..) {
            info!("Marking duplicate as dead: id={id} value={value}");
            value.status = NodeStatus::Dead;
            self.insert(id, value);
        }
    }

    #[tracing::instrument(skip(self))]
    fn recycle_dead_peers(&mut self) -> bool {
        info!("Recycling old IDs");
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

        info!("Dead peers, attempting to recycle");
        let new_id = crate::recycling::recycle_ids(self.id.clone(), dead_peers);
        if self.id != new_id {
            info!("Old IDs recycled, new_id={new_id}");
            self.id = new_id;
            self.increment_inner();
            info!("New timestamp: {}", self.timestamp());
            true
        } else {
            false
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
            self.id,
            self.membership_hash(),
            map,
        )
    }
}

// TODO: This could be expanded to "find_ids"; though probably a smarter ID selection & reorg is
// better long-term
fn find_first_valid_id<A>(map: &ItcMap<NodeInfo<A>>, uuid: Uuid) -> Option<IdTree> {
    map.iter()
        .find(|(_, info)| info.uuid == uuid && info.status == NodeStatus::Healthy)
        .map(|(id, _)| id)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::uuid;

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
