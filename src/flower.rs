use crate::{
    connection::Connection,
    constants,
    ds::{StableVec, WalkieTalkie},
    engine::Engine,
    handle::FlowerHandle,
    message::{BinaryPatch, PollinationMessage},
    nucleus_old::{Nucleus, NucleusError},
    peer_info::PeerInfo,
    reality_token::RealityToken,
};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    fmt::{self, Debug},
    hash::Hash,
    time::Duration,
};
use tokio::{
    sync::mpsc::Receiver,
    task::JoinSet,
    time::{MissedTickBehavior, interval},
};
#[allow(unused)]
use tracing::{Instrument, debug, error, info, instrument, trace, warn};
use treeclocks::{EventTree, IdTree};
use uuid::Uuid;

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Topic(String);

pub struct Flower<E: Engine<PollinationMessage>> {
    nucleus: Nucleus<E::Addr>,
    engine: E,
    seed_list: Vec<E::Addr>,
    conns: StableVec<Connection>,
    receivers: JoinSet<ReceiverLoop>,
    handle_comm: WalkieTalkie<Nucleus<E::Addr>, ()>,
}

type ReceiverLoop = (
    Receiver<PollinationMessage>,
    usize,
    Option<PollinationMessage>,
);

impl<E> Flower<E>
where
    E: Engine<PollinationMessage>,
    E::Addr: Clone + Serialize + for<'de> Deserialize<'de> + Hash + fmt::Display,
{
    pub fn builder() -> FlowerBuilder<E> {
        FlowerBuilder::default()
    }

    //#[instrument(skip_all, add_field(info = self.own_info()))]
    async fn run(mut self) -> anyhow::Result<()> {
        let mut seed_list = std::mem::take(&mut self.seed_list);
        for addr in seed_list.drain(..) {
            if addr != *self.engine.addr() {
                let (tx, rx) = self.engine.create_conn(addr.clone()).await;
                let idx = self.conns.push(Connection::new(tx));
                self.spawn_receiver_loop(idx, rx);
            }
        }

        let mut heartbeat = interval(constants::HEARTBEAT_TICK_TIME);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut grim_reaper = interval(constants::RECLAIM_IDS_TICK_TIME);
        grim_reaper.set_missed_tick_behavior(MissedTickBehavior::Skip);

        debug!("Looping: {}", self.nucleus);
        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    debug!("TICK");
                    self.nucleus.bump();
                    self.broadcast().await;
                }

                _ = self.handle_comm.recv() => {
                    let _ = self.handle_comm.send(self.nucleus.clone()).await;
                }

                _ = grim_reaper.tick() => {
                    debug!("REAPER");
                    if self.nucleus.reap_souls().is_some() {
                        // TODO: Handle error
                        if let Some(msg) = self.msg_heartbeat() {
                            debug!("d b {msg}");
                            let _ = self.broadcast_update().await;
                        }
                    }
                }

                new_conn = self.engine.get_new_conn() => {
                    if let Some((tx, rx)) = new_conn {
                        let idx = self.conns.push(Connection::new(tx));
                        self.spawn_receiver_loop(idx, rx);
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
                                self.preprocess_msg(idx, &msg).await;
                                self.handle_msg(idx, msg).await;
                                self.spawn_receiver_loop(idx, rx);
                            } else {
                                error!("Msg was None; connection closed for {idx}");
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

    async fn preprocess_msg(&mut self, idx: usize, message: &PollinationMessage) {
        let conn = self.conns.get_mut(idx).expect("logic bug");
        conn.peer_id = message.id().cloned();
        conn.peer_ts = message.timestamp().cloned();
    }

    async fn handle_msg(&mut self, idx: usize, message: PollinationMessage) {
        debug!("s0 {self}");
        debug!("i {idx}: {message}");

        let msg = self.handle_msg_inner(idx, message);

        if let Some(msg) = msg {
            debug!("o {idx}: {msg}");
            // TODO: Handle errors
            let conn = self.conns.get_mut(idx).unwrap();
            if let Err(err) = conn.send(msg).await {
                error!("Connection error: {err}");
                self.conns.remove(idx);
            }
        }

        debug!("s1 {self}");
    }

    fn handle_msg_inner(
        &mut self,
        idx: usize,
        message: PollinationMessage,
    ) -> Option<PollinationMessage> {
        self.nucleus.check_and_reset_reality_token();
        match message {
            PollinationMessage::Heartbeat {
                uuid: _,
                id,
                timestamp,
                reality_token,
            } => self.handle_heartbeat(idx, id, timestamp, reality_token),
            PollinationMessage::Update {
                uuid: _,
                id,
                timestamp,
                reality_token,
                patch,
            } => self.handle_update(idx, id, timestamp, reality_token, patch),
            PollinationMessage::RealitySkew {
                uuid: _,
                id,
                timestamp,
                reality_token,
                patch,
                peer_count,
            } => self.handle_reality_skew(idx, id, timestamp, reality_token, patch, peer_count),
            PollinationMessage::NewMember { uuid: _ } => self.handle_new_member(),
            PollinationMessage::Seed {
                uuid: _,
                id,
                timestamp,
                reality_token,
                patch,
                peer_count,
                new_id,
            } => self.handle_seed(idx, id, timestamp, reality_token, patch, peer_count, new_id),
        }
    }

    #[instrument(name = "HB", skip_all)]
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

    #[instrument(name = "UP", skip_all)]
    fn handle_update(
        &mut self,
        _idx: usize,
        _peer_id: IdTree,
        peer_ts: EventTree,
        peer_rt: RealityToken,
        peer_patch: BinaryPatch,
    ) -> Option<PollinationMessage> {
        match self.nucleus.timestamp().partial_cmp(&peer_ts) {
            Some(Ordering::Greater) => {
                let patch = self.nucleus.create_patch(&peer_ts);
                self.msg_update(patch)
            }
            cmp @ Some(Ordering::Less) | cmp @ None => {
                if peer_rt != self.nucleus.reality_token() {
                    let mut new_nucleus = self.nucleus.clone();
                    match new_nucleus.apply(peer_patch) {
                        Ok(()) => {
                            if new_nucleus.reality_token() != peer_rt {
                                let patch = self.nucleus.create_patch(&peer_ts);
                                self.msg_reality_skew(patch)
                            } else {
                                error!("Clean Update does not contain Self!");
                                assert!(new_nucleus.contains_self());
                                self.nucleus = new_nucleus;
                                self.msg_heartbeat()
                            }
                        }
                        Err(err) => {
                            error!("Error applying patch: {err}");
                            None
                        }
                    }
                } else {
                    // TODO: Handle error
                    self.nucleus
                        .apply(peer_patch)
                        .expect("Unable to apply patch");

                    self.msg_heartbeat()
                }
            }

            /*
            match self.nucleus.apply_old(peer_rt, patch) {
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
            */
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

    #[instrument(name = "RS", skip_all)]
    fn handle_reality_skew(
        &mut self,
        _idx: usize,
        _peer_id: IdTree,
        peer_ts: EventTree,
        peer_rt: RealityToken,
        peer_patch: BinaryPatch,
        peer_count: usize,
    ) -> Option<PollinationMessage> {
        if peer_rt != self.nucleus.reality_token() {
            let mut new_nucleus = self.nucleus.clone();
            match new_nucleus.apply(peer_patch) {
                Ok(()) => {
                    if new_nucleus.reality_token() != peer_rt {
                        if peer_count > self.nucleus.peer_count() {
                            self.msg_new_member()
                        } else if peer_count == self.nucleus.peer_count() {
                            if peer_rt > self.nucleus.reality_token() {
                                self.msg_new_member()
                            } else {
                                let patch = self.nucleus.create_patch(&peer_ts);
                                self.msg_reality_skew(patch)
                            }
                        } else {
                            let patch = self.nucleus.create_patch(&peer_ts);
                            self.msg_reality_skew(patch)
                        }
                    } else {
                        assert!(self.nucleus.contains_self());
                        self.nucleus = new_nucleus;
                        self.msg_heartbeat()
                    }
                }
                Err(err) => {
                    error!("Error applying patch: {err}");
                    None
                }
            }
        } else {
            debug!("Reality skew detected by peer, but not by self.");
            self.msg_heartbeat()
        }
        /*
        if matches!(
            self.nucleus.apply_old(peer_rt, patch),
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
            debug!("Reality skew detected by peer, but not by self.");
            self.msg_heartbeat()
        }
        */
    }

    #[instrument(name = "NM", skip_all)]
    fn handle_new_member(&mut self) -> Option<PollinationMessage> {
        if let Some(peer_id) = self.nucleus.propagate() {
            self.msg_seed(peer_id)
        } else {
            self.msg_see_other()
        }
    }

    #[instrument(name = "SE", skip_all)]
    fn handle_seed(
        &mut self,
        idx: usize,
        peer_id: IdTree,
        peer_ts: EventTree,
        peer_rt: RealityToken,
        peer_patch: BinaryPatch,
        peer_count: usize,
        new_id: Option<IdTree>,
    ) -> Option<PollinationMessage> {
        if peer_rt != self.nucleus.reality_token() {
            let mut new_nucleus = self.nucleus.clone();
            match new_nucleus.apply(peer_patch.clone()) {
                Ok(()) => {
                    if new_nucleus.reality_token() != peer_rt {
                        if peer_count > self.nucleus.peer_count() {
                            assert!(!new_nucleus.contains_self());

                            // Take the new ID
                            self.nucleus.reset(new_id, peer_patch);
                            let patch = self.nucleus.create_patch(&peer_ts);
                            self.msg_update(patch)
                        } else if peer_count == self.nucleus.peer_count() {
                            assert!(!new_nucleus.contains_self());

                            if peer_rt > self.nucleus.reality_token() {
                                // Take the new ID
                                self.nucleus.reset(new_id, peer_patch);
                                let patch = self.nucleus.create_patch(&peer_ts);
                                self.msg_update(patch)
                            } else {
                                let patch = self.nucleus.create_patch(&peer_ts);
                                self.msg_reality_skew(patch)
                            }
                        } else {
                            let patch = self.nucleus.create_patch(&peer_ts);
                            self.msg_reality_skew(patch)
                        }
                    } else {
                        if new_nucleus.contains_self() {
                            self.nucleus = new_nucleus;
                            debug!("Mark dead; Added");
                            self.nucleus.mark_dead(new_id);
                            let patch = self.nucleus.create_patch(&peer_ts);
                            self.msg_update(patch)
                        } else {
                            // Take the new ID
                            self.nucleus.reset(new_id, peer_patch);
                            let patch = self.nucleus.create_patch(&peer_ts);
                            self.msg_update(patch)
                        }
                    }
                }
                Err(err) => {
                    error!("Error applying patch: {err}");
                    None
                }
            }
        } else {
            assert!(self.nucleus.contains_self());
            debug!("Mark dead; the same reality_token");
            self.nucleus.mark_dead(new_id);
            let patch = self.nucleus.create_patch(&peer_ts);
            self.msg_update(patch)
        }
        /*
        // TODO: Need to fix
        if self.nucleus.reality_token() == peer_rt {
            self.nucleus.mark_dead(new_id);
            let patch = self.nucleus.create_patch(&peer_ts);
            self.msg_update(patch)
        } else {
            let msg = self.handle_update(idx, peer_id, peer_ts.clone(), peer_rt, patch.clone());
            if matches!(msg, Some(PollinationMessage::RealitySkew { .. })) {
                // TODO: Figure out how to broadcast here...
                self.nucleus.mark_dead(self.nucleus.id()?.clone());
                self.broadcast_update().await;

                // TODO: Handle error
                //self.nucleus = Nucleus::from_parts(new_id, peer_rt, patch);
                self.nucleus.reset(new_id, patch);
                self.nucleus.check_and_reset_reality_token();
                if self.nucleus.set(self.engine.addr().clone()) {
                    error!(
                        "PeerId's were removed when handling initial insert from seed. This is a sign of bug in stability of ID's."
                    );
                    panic!("Core logic bug");
                }
                self.nucleus.check_and_reset_reality_token();
            }

            let patch = self.nucleus.create_patch(&peer_ts);
            self.msg_update(patch)
        }
        */
    }

    #[instrument(name = "SO", skip_all)]
    fn handle_see_other(
        &mut self,
        _idx: usize,
        _peer_id: IdTree,
        _peer_ts: EventTree,
        _peer_rt: RealityToken,
        _patch: BinaryPatch,
    ) -> Option<PollinationMessage> {
        debug!("SeeOther not implemented.");
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
            uuid: self.nucleus.uuid(),
            id,
            timestamp: self.nucleus.timestamp().to_owned(),
            reality_token: self.nucleus.reality_token(),
        })
    }

    fn msg_update(&self, patch: BinaryPatch) -> Option<PollinationMessage> {
        let id = self.nucleus.id()?.clone();
        Some(PollinationMessage::Update {
            uuid: self.nucleus.uuid(),
            id,
            timestamp: self.nucleus.timestamp().to_owned(),
            reality_token: self.nucleus.reality_token(),
            patch,
        })
    }

    fn msg_reality_skew(&self, patch: BinaryPatch) -> Option<PollinationMessage> {
        let id = self.nucleus.id()?.clone();
        Some(PollinationMessage::RealitySkew {
            uuid: self.nucleus.uuid(),
            id,
            timestamp: self.nucleus.timestamp().to_owned(),
            reality_token: self.nucleus.reality_token(),
            patch,
            peer_count: self.nucleus.peer_count(),
        })
    }

    // Option is unecessary but maintains symmetry
    fn msg_new_member(&self) -> Option<PollinationMessage> {
        Some(PollinationMessage::NewMember {
            uuid: self.nucleus.uuid(),
        })
    }

    fn msg_seed(&self, new_id: IdTree) -> Option<PollinationMessage> {
        let id = self.nucleus.id()?.clone();
        let patch = self.nucleus.create_patch(&EventTree::Leaf(0));
        Some(PollinationMessage::Seed {
            uuid: self.nucleus.uuid(),
            id,
            timestamp: self.nucleus.timestamp().to_owned(),
            reality_token: self.nucleus.reality_token(),
            patch,
            peer_count: self.nucleus.peer_count(),
            new_id,
        })
    }

    fn msg_see_other(&self) -> Option<PollinationMessage> {
        let id = self.nucleus.id()?.clone();
        let patch = self.nucleus.create_patch(&EventTree::Leaf(0));
        Some(PollinationMessage::SeeOther {
            uuid: self.nucleus.uuid(),
            id,
            timestamp: self.nucleus.timestamp().to_owned(),
            reality_token: self.nucleus.reality_token(),
            patch,
        })
    }

    async fn broadcast(&mut self) {
        if let Some(msg) = self.msg_heartbeat() {
            debug!("o b {msg}");
            for conn in self.conns.iter_mut() {
                // TODO: Handle error
                let _ = conn.send(msg.clone()).await;
            }
        }
    }

    async fn broadcast_update(&mut self) -> Option<()> {
        let mut actions = vec![];
        for (idx, conn) in self.conns.enumerate() {
            if let Some(peer_ts) = &conn.peer_ts {
                let patch = self.nucleus.create_patch(&peer_ts);
                let msg = self.msg_update(patch)?;
                actions.push((idx, msg))
            }
        }

        for (idx, msg) in actions {
            let conn = self.conns.get_mut(idx)?;
            // TODO: Handle errors
            let _ = conn.send(msg).await;
        }

        Some(())
    }

    fn spawn_receiver_loop(&mut self, idx: usize, mut rx: Receiver<PollinationMessage>) {
        self.receivers.spawn(
            async move {
                let msg = rx.recv().await;
                (rx, idx, msg)
            }
            .in_current_span(),
        );
    }
}

impl<E: Engine<PollinationMessage>> fmt::Display for Flower<E>
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

pub struct FlowerBuilder<E: Engine<PollinationMessage>> {
    engine: Option<E>,
    seed_list: Vec<E::Addr>,
}

impl<E> FlowerBuilder<E>
where
    E: Engine<PollinationMessage> + 'static + Send + Sync,
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

    pub async fn bloom(self) -> anyhow::Result<FlowerHandle<E::Addr>> {
        let mut engine = self.engine.expect("No engine");
        engine.start().await;

        let (w0, w1) = WalkieTalkie::pair();
        let flower = Flower {
            nucleus: Nucleus::new(Uuid::new_v4(), engine.addr().clone()),
            engine,
            seed_list: self.seed_list,
            conns: StableVec::new(),
            receivers: JoinSet::new(),
            handle_comm: w0,
        };

        let handle = tokio::task::spawn(async move { flower.run().await }.in_current_span());

        Ok(FlowerHandle::new(w1, handle))
    }
}

impl<E: Engine<PollinationMessage>> Default for FlowerBuilder<E> {
    fn default() -> FlowerBuilder<E> {
        FlowerBuilder {
            engine: None,
            seed_list: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::{Receiver, Sender, channel};

    #[test]
    fn test_seed_1() {
        let mut f0 = flower_marionette(Uuid::from_u128(1));
        f0.nucleus.set(0);
        assert!(f0.nucleus.contains_self());

        let mut f1 = flower_marionette(Uuid::from_u128(2));
        f1.nucleus.set(1);
        assert!(f1.nucleus.contains_self());

        let msg = f0.msg_heartbeat().unwrap();
        println!("f0 -> {msg}");
        let msg = f1.handle_msg_inner(0, msg).unwrap();
        println!("f1 -> {msg}");
        let msg = f0.handle_msg_inner(0, msg).unwrap();
        println!("f0 -> {msg}");
        let msg = f1.handle_msg_inner(0, msg).unwrap();
        println!("f1 -> {msg}");
        let msg = f0.handle_msg_inner(0, msg).unwrap();
        println!("f0 -> {msg}");
        let msg = f1.handle_msg_inner(0, msg).unwrap();
        println!("f1 -> {msg}");

        // Gets rid of
        f0.nucleus.force_propagating();
        f1.nucleus.force_propagating();
        insta::assert_debug_snapshot!((f0.nucleus, f1.nucleus));
    }

    #[test]
    fn test_seed_2() {
        let mut f0 = flower_marionette(Uuid::from_u128(1));
        f0.nucleus.set(0);
        assert!(f0.nucleus.contains_self());

        let mut f1 = flower_marionette(Uuid::from_u128(2));
        f1.nucleus.set(1);
        assert!(f1.nucleus.contains_self());

        let msg = f0.msg_heartbeat().unwrap();
        println!("{msg}");
        let msg = f1.handle_msg_inner(0, msg).unwrap();
        println!("{msg}");
        let msg = f0.handle_msg_inner(0, msg).unwrap();
        println!("{msg}");
        let msg = f1.handle_msg_inner(0, msg).unwrap();
        println!("{msg}");
        let msg = f0.handle_msg_inner(0, msg).unwrap();
        println!("{msg}");
        let msg = f1.handle_msg_inner(0, msg).unwrap();
        println!("{msg}");
        f0.nucleus.force_propagating();
        f1.nucleus.force_propagating();
        insta::assert_debug_snapshot!((f0.nucleus, f1.nucleus));
    }

    fn flower_marionette(uuid: Uuid) -> Flower<FakeEngine> {
        let (w0, _) = WalkieTalkie::pair();
        Flower {
            nucleus: Nucleus::new(uuid, 0),
            engine: FakeEngine {},
            seed_list: vec![],
            conns: StableVec::new(),
            receivers: JoinSet::new(),
            handle_comm: w0,
        }
    }

    struct FakeEngine {}
    impl<T> Engine<T> for FakeEngine
    where
        T: Serialize + for<'a> Deserialize<'a> + Clone + Send,
    {
        type Addr = usize;
        async fn start(&mut self) {}

        fn addr(&self) -> &Self::Addr {
            &0
        }

        async fn create_conn(&mut self, addr: usize) -> (Sender<T>, Receiver<T>) {
            unimplemented!()
        }

        async fn get_new_conn(&mut self) -> Option<(Sender<T>, Receiver<T>)> {
            unimplemented!()
        }
    }
}
