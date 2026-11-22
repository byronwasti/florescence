use pollination::{PollinationCore, PollinationMessage};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::{HashMap, VecDeque, hash_map::Entry},
    fmt::Debug,
};
use uuid::Uuid;

const TOP_PEER_COUNT: usize = 5;
const PEER_PULL: usize = 2;
const OLDEST_PEER_COUNT: usize = 3;
const LOOKBACK_WINDOW_COUNT: usize = 100;

#[derive(Debug, Clone)]
pub struct PollinationRouter<A> {
    core: PollinationCore<A>,
    peer_tracker: PeerTracker,
    seed_addrs: VecDeque<A>,
}

impl<A> PollinationRouter<A>
where
    A: Clone + for<'a> Deserialize<'a> + Serialize + Debug + PartialEq,
{
    pub fn new(core: PollinationCore<A>, mut seed_addrs: VecDeque<A>) -> Self {
        Self {
            core,
            peer_tracker: PeerTracker::new(),
            seed_addrs,
        }
    }

    pub fn core(&self) -> &PollinationCore<A> {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut PollinationCore<A> {
        &mut self.core
    }

    pub fn handle_message(
        &mut self,
        peer_addr: A,
        peer_msg: PollinationMessage<A>,
    ) -> Vec<(A, PollinationMessage<A>)> {
        self.peer_tracker.receive(peer_msg.uuid);
        if let Some(msg) = self.core.handle_message(peer_msg) {
            vec![(peer_addr, msg)]
        } else {
            vec![]
        }
    }

    pub fn heartbeat(&mut self) -> Vec<(A, PollinationMessage<A>)> {
        self.core.increment();
        let msg = self.core.heartbeat_message();

        // TODO: Remove hard-code
        let mut peers: Vec<_> = self
            .peer_tracker
            .top_k(TOP_PEER_COUNT)
            .drain(..)
            .filter_map(|(id, _)| self.core.get_peer(id))
            .map(|n| n.addr.clone())
            .collect();

        let seed_pulls = PEER_PULL + TOP_PEER_COUNT - peers.len();
        let mut seed_pulls: Vec<_> = (0..seed_pulls)
            .filter_map(|_| self.seed_addrs.pop_front())
            .collect();

        peers.append(&mut seed_pulls);

        let (oldest_peer, val) = self.core.timestamp().min_index();
        if val < self.core.timestamp().max() / 2 {
            let mut oldest_peers = self.core.core_map().get_many(&oldest_peer);
            for peer in oldest_peers.drain(..).take(OLDEST_PEER_COUNT) {
                peers.push(peer.1.addr.clone());
            }
        }

        peers.drain(..).map(|addr| (addr, msg.clone())).collect()
    }

    pub fn top_k_peers(&self, k: usize) -> Vec<(Uuid, usize)> {
        self.peer_tracker.top_k(k)
    }
}

#[derive(Default, Debug, Clone)]
struct PeerTracker {
    contact_history: VecDeque<Uuid>,
    pub lookback_count: usize,
}

impl PeerTracker {
    pub fn new() -> Self {
        Self {
            contact_history: VecDeque::new(),
            // TODO: Remove hard-code
            lookback_count: LOOKBACK_WINDOW_COUNT,
        }
    }

    pub fn receive(&mut self, uuid: Uuid) {
        self.contact_history.push_back(uuid);
        self.contact_history.retain_back(self.lookback_count);
    }

    pub fn send(&mut self, uuid: Uuid) {
        self.receive(uuid);
    }

    /// Returns (Uuid, # of messages in lookback_count)
    pub fn top_k(&self, k: usize) -> Vec<(Uuid, usize)> {
        let mut contacts = self
            .contact_history
            .iter()
            .fold(HashMap::new(), |mut acc, uuid| {
                if let Some(entry) = acc.get_mut(uuid) {
                    *entry += 1;
                } else {
                    acc.insert(*uuid, 1);
                }
                acc
            })
            .drain()
            .map(|(key, value)| (key, value))
            .collect::<Vec<_>>();

        contacts.sort_by_key(|(k, v)| Reverse(*v));
        contacts.truncate(k);
        contacts
    }
}

#[derive(Default, Copy, Clone)]
struct PeerInfo {
    sent: u64,
    received: u64,
}
