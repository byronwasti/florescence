mod tonic_engine;
use crate::message::PollinationMessage;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
pub use tonic_engine::TonicEngine;

pub trait Engine<I>: Send {
    type Addr: PartialEq;
    // TODO: Associated error type

    /// Must be non-blocking
    // TODO: This is a weird API; maybe it should return engine?
    fn start(&mut self);

    fn addr(&self) -> &Self::Addr;

    fn create_conn(&mut self, addr: Self::Addr) -> Connection<I>;

    fn get_new_conns(&mut self) -> Vec<Connection<I>>;
}

pub struct Connection<I> {
    pub tx: UnboundedSender<PollinationMessage<I>>,
    pub rx: UnboundedReceiver<PollinationMessage<I>>,
}

impl<I> Connection<I> {
    pub fn new(
        tx: UnboundedSender<PollinationMessage<I>>,
        rx: UnboundedReceiver<PollinationMessage<I>>,
    ) -> Self {
        Self { tx, rx }
    }
}
