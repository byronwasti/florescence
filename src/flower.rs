use crate::engine::{Connection, Engine};
use crate::message::PollinationMessage;
use crate::reality_token::RealityToken;
use std::collections::HashSet;
use std::hash::Hash;
use std::marker::PhantomData;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};
use treeclocks::{EventTree, IdTree, ItcMap};

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Topic(String);

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct PeerInfo<I, A> {
    id: I,
    addr: A,
    topics: HashSet<Topic>,
}

pub struct FlowerHandle<I> {
    handle: JoinHandle<anyhow::Result<()>>,
    _p: PhantomData<I>,
}

impl<I> FlowerHandle<I> {
    /*
    pub fn pollinator<P: Pollinator + 'static>(&self, interval: Duration) -> P {
        let (pollinator, inner) = P::from_conn(EngineConnection {});
        pollinator
    }
    */

    pub async fn runtime(self) -> anyhow::Result<()> {
        self.handle.await??;
        Ok(())
    }
}

#[allow(dead_code)]
pub struct Flower<I, E: Engine<I>> {
    id: I,
    itc_id: IdTree,
    timestamp: EventTree,
    reality_token: RealityToken,
    engine: E,
    seed_list: Vec<E::Addr>,
    peer_info: ItcMap<PeerInfo<I, E::Addr>>,
    conns: Vec<Connection<I>>,
}

impl<I, E> Flower<I, E>
where
    E: Engine<I>,
    I: Clone,
{
    pub fn builder() -> FlowerBuilder<I, E> {
        FlowerBuilder::default()
    }

    async fn run(mut self) -> anyhow::Result<()> {
        for addr in self.seed_list {
            if addr != *self.engine.addr() {
                let conn = self.engine.create_conn(addr);
                self.conns.push(conn);
            }
        }

        let mut interval = interval(Duration::from_millis(100));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    for conn in &mut self.conns {
                        // TODO: Handle error
                        let _ = conn.tx.send(PollinationMessage::Heartbeat {
                            id: self.id.clone(),
                            itc_id: self.itc_id.clone(),
                            timestamp: self.timestamp.clone(),
                            reality_token: self.reality_token,
                        });
                    }
                }
            }
        }
    }
}

pub struct FlowerBuilder<I, E: Engine<I>> {
    id: Option<I>,
    engine: Option<E>,
    seed_list: Vec<E::Addr>,
}

impl<I, E> FlowerBuilder<I, E>
where
    E: Engine<I> + 'static,
    I: Clone + Send + Hash + 'static,
    E::Addr: Clone + Send,
{
    pub fn id(mut self, id: I) -> Self {
        self.id = Some(id);
        self
    }

    pub fn engine(mut self, engine: E) -> Self {
        self.engine = Some(engine);
        self
    }

    pub fn seed(mut self, addrs: &[E::Addr]) -> Self {
        self.seed_list = addrs.to_vec();
        self
    }

    pub fn bloom(self) -> anyhow::Result<FlowerHandle<I>> {
        let mut engine = self.engine.expect("No engine");
        engine.start();

        let id = self.id.expect("no id provided");
        let itc_id = IdTree::One;
        let timestamp = EventTree::new();
        let timestamp = timestamp.event(&itc_id);

        let flower = Flower {
            reality_token: RealityToken::new(&id),
            id,
            itc_id,
            timestamp,
            engine,
            seed_list: self.seed_list,
            peer_info: ItcMap::new(),
            conns: vec![],
        };

        let handle = tokio::task::spawn(async { flower.run().await });

        Ok(FlowerHandle {
            handle,
            _p: PhantomData,
        })
    }
}

impl<I, E: Engine<I>> Default for FlowerBuilder<I, E> {
    fn default() -> FlowerBuilder<I, E> {
        FlowerBuilder {
            id: None,
            engine: None,
            seed_list: vec![],
        }
    }
}
