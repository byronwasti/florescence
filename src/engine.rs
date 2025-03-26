use crate::pollinator::PollinatorCore;
mod tonic_engine;
use crate::message::PollinationMessage;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
pub use tonic_engine::TonicEngine;

pub trait Engine<T, I> {
    type Addr;

    /// Must be non-blocking
    fn start(&mut self);

    fn addr(&self) -> &Self::Addr;

    fn create_conn(&mut self, addr: Self::Addr) -> Connection<T, I, Self::Addr>;

    fn get_new_conns(&mut self) -> Vec<Connection<T, I, Self::Addr>>;
}

pub(crate) struct EngineCore<E, T, I, A> {
    engine: E,
    pollinators: Vec<PollinatorCore>,
    connections: Vec<Connection<T, I, A>>,
}

pub struct Connection<T, I, A> {
    pub tx: UnboundedSender<PollinationMessage<T, I, A>>,
    pub rx: UnboundedReceiver<PollinationMessage<T, I, A>>,
}

impl<T, I, A> Connection<T, I, A> {
    pub fn new(
        tx: UnboundedSender<PollinationMessage<T, I, A>>,
        rx: UnboundedReceiver<PollinationMessage<T, I, A>>,
    ) -> Self {
        Self { tx, rx }
    }
}

// TODO: Remove
pub struct EngineConnection;
