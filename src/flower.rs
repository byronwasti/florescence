use crate::pollinator::{RealityToken, Pollinator};
use crate::engine::{Engine, EngineConnection};
use std::collections::{HashMap, HashSet};
use treeclocks::{EventTree, ItcMap, itc_map::UpdatePacket};
use std::sync::Arc;
use std::time::Duration;

struct Topic(String);

struct FlowerCore<I, A> {
    id: I,
    addr: A,
    peer_info: HashMap<I, PeerInfo<I, A>>,
}

struct PeerInfo<I, A> {
    id: I,
    addr: A,
    topics: HashSet<Topic>,
}


pub struct Flower {
    conn: EngineConnection,
}

impl Flower {
    /*
    pub fn builder<I, E>() -> FlowerBuilder<I, E> {
        FlowerBuilder::default()
    }
    */

    pub fn pollinator<P: Pollinator + 'static>(&self, interval: Duration) -> P {
        let (pollinator, inner) = P::from_conn(EngineConnection {});
        pollinator
    }
}

/*
pub struct FlowerBuilder<I, E> {
    id: Option<I>,
    engine: Option<E>,
}

impl<T, I, E: Engine<T, I>> FlowerBuilder<I, E> {
    pub fn id(mut self, id: I) -> Self {
        self.id = Some(id);
        self
    }

    pub fn engine(mut self, engine: E) -> Self {
        self.engine = Some(engine);
        self
    }

    pub async fn bloom(mut self) -> Result<Flower, FlowerError> {

        // TODO: Kick off background task
        //let conn = self.engine.unwrap().run().await;
        //Ok(Flower { conn })
        todo!()
    }
}

impl<I, E> Default for FlowerBuilder<I, E> {
    fn default() -> FlowerBuilder<I, E> {
        FlowerBuilder {
            id: None,
            engine: None,
        }
    }
}
*/

pub enum FlowerError {}
