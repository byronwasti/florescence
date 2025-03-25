use crate::pollinator::PollinatorCore;
mod tonic_engine;
pub use tonic_engine::TonicEngine;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use crate::message::PollinationMessage;
use serde::{Serialize, Deserialize};

pub trait Engine {
    type Addr;

    /// Must be non-blocking
    fn start(&mut self, tx: UnboundedSender<EngMessage<Self::Addr>>) -> impl std::future::Future<Output=()> + Send;

    fn addr(&self) -> &Self::Addr;

    fn remove_conn(&mut self, addr: Self::Addr) -> impl std::future::Future<Output=()> + Send;

    fn new_conn<T, I, A>(&mut self, addr: Self::Addr) -> impl std::future::Future<Output=(UnboundedSender<PollinationMessage<T, I, A>>, UnboundedReceiver<PollinationMessage<T, I, A>>)> + Send
    where T: for<'a> Deserialize<'a> + Serialize + Send + Sync + 'static,
          I: for<'a> Deserialize<'a> + Serialize + Send + Sync + 'static,
          A: for<'a> Deserialize<'a> + Serialize + Send + Sync + 'static;
}

pub(crate) struct EngineCore<E, A> {
    engine: E,
    pollinators: Vec<PollinatorCore>,
    connections: Vec<Connection<A>>,
}


pub struct Connection<A> {
    addr: A,
}

pub enum EngMessage<A> {
    New(Connection<A>),
    UnableToReach(A),
    Terminated(A),
}

// TODO: Remove
pub struct EngineConnection;
