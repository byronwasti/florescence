use crate::engine::{Connection, Engine};
use crate::pollinator::{Pollinator, RealityToken};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use treeclocks::{EventTree, ItcMap, itc_map::UpdatePacket};

#[derive(Clone, Debug)]
struct Topic(String);

#[derive(Clone, Debug)]
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

    pub async fn runtime(mut self) -> anyhow::Result<()> {
        self.handle.await??;
        Ok(())
    }
}

pub struct Flower<I, E: Engine<I>> {
    id: I,
    engine: E,
    seed_list: Vec<E::Addr>,
    peer_info: ItcMap<PeerInfo<I, E::Addr>>,
    conns: Vec<Connection<I, E::Addr>>,
}

impl<I, E: Engine<I>> Flower<I, E> {
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
        loop {
            todo!()
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
    I: Clone + Send + 'static,
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

    pub fn bloom(mut self) -> anyhow::Result<FlowerHandle<I>> {
        let mut engine = self.engine.expect("No engine");
        engine.start();

        let flower = Flower {
            id: self.id.expect("no id provided"),
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
