mod tonic_engine;
use crate::message::PollinationMessage;
use tokio::sync::mpsc::{Receiver, Sender};
pub use tonic_engine::TonicEngine;

pub trait Engine: Send {
    type Addr: PartialEq;
    // TODO: Associated error type

    /// Must be non-blocking
    // TODO: This is a weird API; maybe it should return engine?
    fn start(&mut self);

    fn addr(&self) -> &Self::Addr;

    fn create_conn(&mut self, addr: Self::Addr) -> Connection;

    fn get_new_conns(&mut self) -> Vec<Connection>;
}

pub type Connection = (Sender<PollinationMessage>, Receiver<PollinationMessage>);
