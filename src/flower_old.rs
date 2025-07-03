use crate::{
    connection::Connection,
    constants,
    ds::{StableVec, WalkieTalkie},
    engine_old::Engine,
    handle::FlowerHandle,
    message::PollinationMessage,
    nucleus::Nucleus,
};
use serde::{Deserialize, Serialize};
use std::{
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
                    if self.nucleus.reap_souls() {
                        // TODO: Handle error
                        if let Some(msg) = self.nucleus.msg_heartbeat() {
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

        let msg = self.nucleus.handle_message(message);

        let msg = todo!();

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

    async fn broadcast(&mut self) {
        if let Some(msg) = self.nucleus.msg_heartbeat() {
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
                let msg = self.nucleus.msg_update(patch)?;
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

    /*
    async fn broadcast_death(&self, old_nucleus: Nucleus<E::Addr>) {
        for (idx, conn) in
    }
    */

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
