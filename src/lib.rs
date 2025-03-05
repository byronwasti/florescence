pub mod codec;
pub mod flower;
pub mod gossip;
pub mod tonicgrpc;

use std::sync::Arc;
use std::time::Duration;

pub struct Flower {
    conn: EngineConnection,
}

impl Flower {
    pub fn builder<E>() -> FlowerBuilder<E> {
        FlowerBuilder::default()
    }

    pub fn pollinator<P: Pollinator + 'static>(&self, interval: Duration) -> P {
        let (pollinator, inner) = P::new(EngineConnection {});
        pollinator
    }
}

pub struct FlowerBuilder<E> {
    engine: Option<E>,
}

impl<E: Engine> FlowerBuilder<E> {
    pub fn engine(mut self, engine: E) -> Self {
        self.engine = Some(engine);
        self
    }

    pub async fn bloom(mut self) -> Result<Flower, FlowerError> {
        // TODO: Kick off background task
        let conn = self.engine.unwrap().run().await;
        Ok(Flower { conn })
    }
}

impl<E> Default for FlowerBuilder<E> {
    fn default() -> FlowerBuilder<E> {
        FlowerBuilder { engine: None }
    }
}

pub enum FlowerError {}

trait Engine {
    async fn run(self) -> EngineConnection;
}

struct EngineConnection {}

pub trait Pollinator {
    type A: PollinatorInner + Sized;
    fn new(conn: EngineConnection) -> (Self, Self::A)
    where
        Self: Sized;
}

pub trait PollinatorInner {}

pub struct IdentityMap<T> {
    t: T,
    conn: EngineConnection,
}

impl<T> IdentityMap<T> {
    fn set(&mut self, value: T) {
        todo!()
    }

    fn get(&self) -> T {
        todo!()
    }

    fn fold<B, F>(&self, init: B, f: F) -> B
    where
        F: FnMut(B, T) -> B,
    {
        todo!()
    }
}
