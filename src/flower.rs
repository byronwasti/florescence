use crate::connection::Connection;
use crate::constants;
use crate::ds::StableVec;
use crate::engine::Engine;
use crate::message::{Patch, PollinationMessage};
use crate::reality_token::RealityToken;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::{self, Debug};
use std::hash::Hash;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, trace, warn};
use treeclocks::{EventTree, IdTree, ItcMap};

mod handle;
mod peer_info;
mod propagativity;

use handle::*;
use peer_info::*;
use propagativity::*;

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Topic(String);

#[allow(dead_code)]
pub struct Flower<E: Engine> {
    propagativity: Propagativity,
    reality_token: RealityToken,
    engine: E,
    seed_list: Vec<E::Addr>,
    core_map: ItcMap<PeerInfo<E::Addr>>,
    conns: StableVec<Connection>,
    receivers: JoinSet<ReceiverLoop>,
}

type ReceiverLoop = (
    Receiver<PollinationMessage>,
    usize,
    Option<PollinationMessage>,
);

impl<E> Flower<E>
where
    E: Engine,
    E::Addr: Clone + Serialize + for<'de> Deserialize<'de> + Hash + fmt::Display,
{
    pub fn builder() -> FlowerBuilder<E> {
        FlowerBuilder::default()
    }

    async fn run(mut self) -> anyhow::Result<()> {
        let id = self.propagativity.id().unwrap();
        self.core_map.insert(
            id.clone(),
            PeerInfo::new(id.clone(), self.engine.addr().clone()),
        );

        let mut seed_list = std::mem::take(&mut self.seed_list);
        for addr in seed_list.drain(..) {
            if addr != *self.engine.addr() {
                let mut conn = self.engine.create_conn(addr);
                let rx = conn.take_rx().unwrap();
                let idx = self.conns.push(conn);
                self.spawn_receiver_loop(idx, rx);
            }
        }

        let mut interval = interval(constants::TICK_TIME);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        info!("Looping");
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    trace!("TICK");

                    if let Some(id) = self.propagativity.id() {
                        trace!("bumping event for id {id:?}");
                        self.core_map.event(id);
                    }

                    // TODO: Handle error
                    if let Some(msg) = self.msg_heartbeat() {
                        debug!("o b {msg}");
                        let _ = self.broadcast(msg).await;
                    }
                }

                new_conn = self.engine.get_new_conn() => {
                    //if let Some((tx, mut rx)) = new_conn {
                    if let Some(mut conn) = new_conn {
                        let mut rx = conn.take_rx().unwrap();
                        let idx = self.conns.push(conn);
                        self.receivers.spawn(async move {
                            let msg = rx.recv().await;
                            (rx, idx, msg)
                        });
                    } else {
                        error!("Engine stopped working.");

                        // TODO: No panic
                        panic!("Engine stopped working.");
                    }
                }

                res = self.receivers.join_next() => {
                    match res {
                        Some(Ok((mut rx, idx, msg))) => {
                            if let Some(msg) = msg {
                                self.handle_msg(idx, msg).await;
                                self.spawn_receiver_loop(idx, rx);
                            } else {
                                error!("Msg was None; connection closed");
                            }
                        }
                        Some(Err(err)) => {
                            error!("Error waiting on JoinSet: {err:?}");
                        }
                        None => {
                            trace!("No open connections. Sleeping for 100ms");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        }
    }

    async fn handle_msg(&mut self, idx: usize, message: PollinationMessage) {
        debug!("i {idx}: {message}");
        let msg = match message {
            PollinationMessage::Heartbeat {
                id,
                timestamp,
                reality_token,
            } => self.handle_heartbeat(idx, id, timestamp, reality_token),
            PollinationMessage::Update {
                id,
                timestamp,
                reality_token,
                patch,
            } => self.handle_update(idx, id, timestamp, reality_token, patch),
            PollinationMessage::RealitySkew {
                id,
                timestamp,
                reality_token,
                patch,
                peer_count,
            } => self.handle_reality_skew(idx, id, timestamp, reality_token, patch, peer_count),
            PollinationMessage::NewMember {} => self.handle_new_member(),
            PollinationMessage::Seed {
                id,
                timestamp,
                reality_token,
                patch,
                new_id,
            } => self.handle_seed(idx, id, timestamp, reality_token, patch, new_id),
            PollinationMessage::SeeOther {
                id,
                timestamp,
                reality_token,
                patch,
            } => self.handle_see_other(idx, id, timestamp, reality_token, patch),
        };

        debug!("s {self}");

        if let Some(msg) = msg {
            debug!("o {idx}: {msg}");
            // TODO: Handle errors
            let conn = self.conns.get_mut(idx).unwrap();
            conn.send(msg).await;
        }
    }

    fn handle_heartbeat(
        &self,
        _idx: usize,
        _peer_id: IdTree,
        peer_ts: EventTree,
        peer_rt: RealityToken,
    ) -> Option<PollinationMessage> {
        match self.core_map.timestamp().partial_cmp(&peer_ts) {
            Some(Ordering::Greater) | None => {
                let patch = self.create_patch(peer_ts);
                self.msg_update(patch)
            }
            Some(Ordering::Less) => self.msg_heartbeat(),
            Some(Ordering::Equal) => {
                if peer_rt != self.reality_token {
                    let patch = self.create_patch(peer_ts);
                    self.msg_reality_skew(patch)
                } else {
                    None
                }
            }
        }
    }

    fn handle_update(
        &mut self,
        idx: usize,
        peer_id: IdTree,
        peer_ts: EventTree,
        peer_rt: RealityToken,
        patch: Patch,
    ) -> Option<PollinationMessage> {
        // TODO: handle error
        let patch = patch.downcast().unwrap();
        match self.core_map.timestamp().partial_cmp(&peer_ts) {
            Some(Ordering::Greater) => {
                let patch = self.create_patch(peer_ts);
                self.msg_update(patch)
            }
            cmp @ Some(Ordering::Less) | cmp @ None => {
                if peer_rt != self.reality_token {
                    let mut core_clone = self.core_map.clone();
                    let mut rt_clone = self.reality_token.clone();

                    let mut removals = core_clone.apply(patch);
                    for (id, _) in removals.drain(..) {
                        rt_clone.increment(id);
                    }

                    if rt_clone != peer_rt {
                        let patch = self.create_patch(peer_ts);
                        self.msg_reality_skew(patch)
                    } else {
                        self.core_map = core_clone;
                        self.reality_token = rt_clone;

                        if cmp.is_none() {
                            let patch = self.create_patch(peer_ts);
                            self.msg_update(patch)
                        } else {
                            None
                        }
                    }
                } else {
                    let mut removals = self.core_map.apply(patch);
                    for (id, _) in removals.drain(..) {
                        self.reality_token.increment(id);
                    }

                    if cmp.is_none() {
                        let patch = self.create_patch(peer_ts);
                        self.msg_update(patch)
                    } else {
                        None
                    }
                }
            }
            Some(Ordering::Equal) => {
                if peer_rt != self.reality_token {
                    let patch = self.create_patch(peer_ts);
                    self.msg_reality_skew(patch)
                } else {
                    None
                }
            }
        }
    }

    fn handle_reality_skew(
        &mut self,
        _idx: usize,
        _peer_id: IdTree,
        peer_ts: EventTree,
        peer_rt: RealityToken,
        _patch: Patch,
        peer_count: usize,
    ) -> Option<PollinationMessage> {
        // TODO: Verify the skew

        match self.core_map.len().cmp(&peer_count) {
            Ordering::Greater => {
                let patch = self.create_patch(peer_ts);
                self.msg_reality_skew(patch)
            }
            Ordering::Less => self.msg_new_member(),
            Ordering::Equal => {
                if self.reality_token > peer_rt {
                    let patch = self.create_patch(peer_ts);
                    self.msg_reality_skew(patch)
                } else {
                    self.msg_new_member()
                }
            }
        }
    }

    fn handle_new_member(&mut self) -> Option<PollinationMessage> {
        if let Some(peer_id) = self.propagativity.propagate() {
            let id = self.propagativity.id().unwrap();
            self.core_map.insert(
                id.clone(),
                PeerInfo::new(id.clone(), self.engine.addr().clone()),
            );

            self.msg_seed(peer_id)
        } else {
            self.msg_see_other()
        }
    }

    fn handle_seed(
        &mut self,
        _idx: usize,
        _peer_id: IdTree,
        peer_ts: EventTree,
        peer_rt: RealityToken,
        patch: Patch,
        new_id: IdTree,
    ) -> Option<PollinationMessage> {
        // TODO: handle error
        match patch.downcast() {
            Ok(patch) => {
                self.core_map = ItcMap::new();
                self.core_map.apply(patch);
                self.core_map.insert(
                    new_id.clone(),
                    PeerInfo::new(new_id.clone(), self.engine.addr().clone()),
                );
                self.propagativity = Propagativity::Resting(new_id, Instant::now());
                self.reality_token = peer_rt;

                self.msg_heartbeat()
            }
            Err(err) => {
                error!("Error parsing patch: {err:?}");
                None
            }
        }
    }

    fn handle_see_other(
        &mut self,
        _idx: usize,
        _peer_id: IdTree,
        peer_ts: EventTree,
        peer_rt: RealityToken,
        patch: Patch,
    ) -> Option<PollinationMessage> {
        warn!("SeeOther not implemented.");
        // TODO: Needs a rethink
        /*
        // TODO: handle error
        let patch = patch.downcast().unwrap();
        self.core_map = ItcMap::new();
        self.core_map.apply(patch);
        self.reality_token = peer_rt;
        */
        None
    }

    fn msg_heartbeat(&self) -> Option<PollinationMessage> {
        let id = self.propagativity.id()?.clone();
        Some(PollinationMessage::Heartbeat {
            id,
            timestamp: self.core_map.timestamp().to_owned(),
            reality_token: self.reality_token,
        })
    }

    fn msg_update(&self, patch: Patch) -> Option<PollinationMessage> {
        let id = self.propagativity.id()?.clone();
        Some(PollinationMessage::Update {
            id,
            timestamp: self.core_map.timestamp().to_owned(),
            reality_token: self.reality_token.clone(),
            patch,
        })
    }

    fn msg_reality_skew(&self, patch: Patch) -> Option<PollinationMessage> {
        let id = self.propagativity.id()?.clone();
        Some(PollinationMessage::RealitySkew {
            id,
            timestamp: self.core_map.timestamp().to_owned(),
            reality_token: self.reality_token.clone(),
            patch,
            peer_count: self.core_map.len(),
        })
    }

    // Option is unecessary but maintains symmetry
    fn msg_new_member(&self) -> Option<PollinationMessage> {
        Some(PollinationMessage::NewMember {})
    }

    fn msg_seed(&self, new_id: IdTree) -> Option<PollinationMessage> {
        let id = self.propagativity.id()?.clone();
        let patch = self.core_map.diff(&EventTree::Leaf(0));
        let patch = Patch::new(patch).expect("Error serializing patch");
        Some(PollinationMessage::Seed {
            id,
            timestamp: self.core_map.timestamp().to_owned(),
            reality_token: self.reality_token.clone(),
            patch,
            new_id,
        })
    }

    fn msg_see_other(&self) -> Option<PollinationMessage> {
        let id = self.propagativity.id()?.clone();
        let patch = self.core_map.diff(&EventTree::Leaf(0));
        let patch = Patch::new(patch).expect("Error serializing patch");
        Some(PollinationMessage::SeeOther {
            id,
            timestamp: self.core_map.timestamp().to_owned(),
            reality_token: self.reality_token.clone(),
            patch,
        })
    }

    fn create_patch(&self, peer_ts: EventTree) -> Patch {
        let itc_patch = self.core_map.diff(&peer_ts);
        Patch::new(itc_patch).expect("Error serializing patch")
    }

    async fn broadcast(&mut self, message: PollinationMessage) -> anyhow::Result<()> {
        for conn in self.conns.iter_mut() {
            conn.send(message.clone()).await?;
        }

        Ok(())
    }

    fn spawn_receiver_loop(&mut self, idx: usize, mut rx: Receiver<PollinationMessage>) {
        self.receivers.spawn(async move {
            let msg = rx.recv().await;
            (rx, idx, msg)
        });
    }
}

impl<E: Engine> fmt::Display for Flower<E>
where
    E::Addr: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let conns = self
            .conns
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                if let Some((prev_msg, timeout)) = &c.prev_msg {
                    format!(
                        "{}: {{ {}, {} }}",
                        idx,
                        prev_msg,
                        timeout.elapsed().as_millis()
                    )
                } else {
                    format!("{}, ", idx)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "{} - {} - [{}] - {}",
            self.propagativity, self.reality_token, conns, self.core_map,
        )
    }
}

pub struct FlowerBuilder<E: Engine> {
    engine: Option<E>,
    seed_list: Vec<E::Addr>,
}

impl<E> FlowerBuilder<E>
where
    E: Engine + 'static + Send + Sync,
    E::Addr: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> + Hash + fmt::Display,
{
    pub fn engine(mut self, engine: E) -> Self {
        self.engine = Some(engine);
        self
    }

    pub fn seed(mut self, addrs: &[E::Addr]) -> Self {
        self.seed_list = addrs.to_vec();
        self
    }

    pub fn bloom(self) -> anyhow::Result<FlowerHandle> {
        let mut engine = self.engine.expect("No engine");
        engine.start();

        let id = IdTree::One;

        let flower = Flower {
            propagativity: Propagativity::Propagating(IdTree::One),
            reality_token: RealityToken::new(),
            engine,
            seed_list: self.seed_list,
            core_map: ItcMap::new(),
            conns: StableVec::new(),
            receivers: JoinSet::new(),
        };

        let handle = tokio::task::spawn(async move { flower.run().await });

        Ok(FlowerHandle { handle })
    }
}

impl<E: Engine> Default for FlowerBuilder<E> {
    fn default() -> FlowerBuilder<E> {
        FlowerBuilder {
            engine: None,
            seed_list: vec![],
        }
    }
}
