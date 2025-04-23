use crate::connection::Connection;
use crate::constants;
use crate::ds::StableVec;
use crate::engine::Engine;
use crate::handle::FlowerHandle;
use crate::message::{BinaryPatch, PollinationMessage};
use crate::nucleus::{Nucleus, NucleusError};
use crate::peer_info::PeerInfo;
use crate::reality_token::RealityToken;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::{self, Debug};
use std::hash::Hash;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinSet;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, trace, warn};
use treeclocks::{EventTree, IdTree};

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Topic(String);

pub struct Flower<E: Engine> {
    nucleus: Nucleus<E::Addr>,
    engine: E,
    seed_list: Vec<E::Addr>,
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
        self.nucleus.set(self.own_info());

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
                    self.nucleus.bump();

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
                        Some(Ok((rx, idx, msg))) => {
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
            if let Err(err) = conn.send(msg).await {
                error!("Connection error: {err}");
                self.conns.remove(idx);
            }
        }
    }

    fn handle_heartbeat(
        &self,
        _idx: usize,
        _peer_id: IdTree,
        peer_ts: EventTree,
        peer_rt: RealityToken,
    ) -> Option<PollinationMessage> {
        match self.nucleus.timestamp().partial_cmp(&peer_ts) {
            Some(Ordering::Greater) | None => {
                let patch = self.nucleus.create_patch(&peer_ts);
                self.msg_update(patch)
            }
            Some(Ordering::Less) => self.msg_heartbeat(),
            Some(Ordering::Equal) => {
                if peer_rt != self.nucleus.reality_token() {
                    let patch = self.nucleus.create_patch(&peer_ts);
                    self.msg_reality_skew(patch)
                } else {
                    None
                }
            }
        }
    }

    fn handle_update(
        &mut self,
        _idx: usize,
        _peer_id: IdTree,
        peer_ts: EventTree,
        peer_rt: RealityToken,
        patch: BinaryPatch,
    ) -> Option<PollinationMessage> {
        match self.nucleus.timestamp().partial_cmp(&peer_ts) {
            Some(Ordering::Greater) => {
                let patch = self.nucleus.create_patch(&peer_ts);
                self.msg_update(patch)
            }
            cmp @ Some(Ordering::Less) | cmp @ None => match self.nucleus.apply(peer_rt, patch) {
                Ok(()) => {
                    if cmp.is_none() {
                        let patch = self.nucleus.create_patch(&peer_ts);
                        self.msg_update(patch)
                    } else {
                        None
                    }
                }
                Err(NucleusError::RealitySkew) => {
                    let patch = self.nucleus.create_patch(&peer_ts);
                    self.msg_reality_skew(patch)
                }
                Err(err) => {
                    error!("Error applying patch: {err}");
                    None
                }
            },
            Some(Ordering::Equal) => {
                if peer_rt != self.nucleus.reality_token() {
                    let patch = self.nucleus.create_patch(&peer_ts);
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
        patch: BinaryPatch,
        peer_count: usize,
    ) -> Option<PollinationMessage> {
        if matches!(
            self.nucleus.apply(peer_rt, patch),
            Err(NucleusError::RealitySkew)
        ) {
            match self.nucleus.peer_count().cmp(&peer_count) {
                Ordering::Greater => {
                    let patch = self.nucleus.create_patch(&peer_ts);
                    self.msg_reality_skew(patch)
                }
                Ordering::Less => self.msg_new_member(),
                Ordering::Equal => {
                    if self.nucleus.reality_token() > peer_rt {
                        let patch = self.nucleus.create_patch(&peer_ts);
                        self.msg_reality_skew(patch)
                    } else {
                        self.msg_new_member()
                    }
                }
            }
        } else {
            warn!("Reality skew detected by peer, but not by self.");
            self.msg_heartbeat()
        }
    }

    fn handle_new_member(&mut self) -> Option<PollinationMessage> {
        if let Some(peer_id) = self.nucleus.propagate() {
            self.msg_seed(peer_id)
        } else {
            self.msg_see_other()
        }
    }

    fn handle_seed(
        &mut self,
        _idx: usize,
        _peer_id: IdTree,
        _peer_ts: EventTree,
        peer_rt: RealityToken,
        patch: BinaryPatch,
        new_id: IdTree,
    ) -> Option<PollinationMessage> {
        // TODO: handle error better
        match patch.deserialize() {
            Ok(patch) => {
                self.nucleus = Nucleus::from_parts(new_id, peer_rt, patch);
                if self.nucleus.set(self.own_info()) {
                    error!(
                        "PeerId's were removed when handling initial insert from seed. This is a sign of bug in stability of ID's."
                    );
                }

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
        _peer_ts: EventTree,
        _peer_rt: RealityToken,
        _patch: BinaryPatch,
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
        let id = self.nucleus.id()?.clone();
        Some(PollinationMessage::Heartbeat {
            id,
            timestamp: self.nucleus.timestamp().to_owned(),
            reality_token: self.nucleus.reality_token(),
        })
    }

    fn msg_update(&self, patch: BinaryPatch) -> Option<PollinationMessage> {
        let id = self.nucleus.id()?.clone();
        Some(PollinationMessage::Update {
            id,
            timestamp: self.nucleus.timestamp().to_owned(),
            reality_token: self.nucleus.reality_token(),
            patch,
        })
    }

    fn msg_reality_skew(&self, patch: BinaryPatch) -> Option<PollinationMessage> {
        let id = self.nucleus.id()?.clone();
        Some(PollinationMessage::RealitySkew {
            id,
            timestamp: self.nucleus.timestamp().to_owned(),
            reality_token: self.nucleus.reality_token(),
            patch,
            peer_count: self.nucleus.peer_count(),
        })
    }

    // Option is unecessary but maintains symmetry
    fn msg_new_member(&self) -> Option<PollinationMessage> {
        Some(PollinationMessage::NewMember {})
    }

    fn msg_seed(&self, new_id: IdTree) -> Option<PollinationMessage> {
        let id = self.nucleus.id()?.clone();
        let patch = self.nucleus.create_patch(&EventTree::Leaf(0));
        Some(PollinationMessage::Seed {
            id,
            timestamp: self.nucleus.timestamp().to_owned(),
            reality_token: self.nucleus.reality_token(),
            patch,
            new_id,
        })
    }

    fn msg_see_other(&self) -> Option<PollinationMessage> {
        let id = self.nucleus.id()?.clone();
        let patch = self.nucleus.create_patch(&EventTree::Leaf(0));
        Some(PollinationMessage::SeeOther {
            id,
            timestamp: self.nucleus.timestamp().to_owned(),
            reality_token: self.nucleus.reality_token(),
            patch,
        })
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

    fn own_info(&self) -> PeerInfo<E::Addr> {
        PeerInfo::new(self.engine.addr().clone())
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
        write!(f, "{} - [{}]", self.nucleus, conns)
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

        let flower = Flower {
            nucleus: Nucleus::new(),
            engine,
            seed_list: self.seed_list,
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
