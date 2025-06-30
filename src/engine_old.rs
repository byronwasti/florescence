use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;
use tokio::sync::mpsc::{Receiver, Sender};

//mod tonic;
pub mod mpsc;

//pub use tonic::TonicEngine;

pub trait Engine<T>: Send + Sync
where
    T: Serialize + for<'a> Deserialize<'a> + Clone,
{
    type Addr: PartialEq
        + Clone
        + Serialize
        + for<'a> Deserialize<'a>
        + Hash
        + fmt::Display
        + Send
        + Sync;
    // TODO: Associated error type

    /// Must be non-blocking
    // TODO: This is a weird API; maybe it should return engine?
    fn start(&mut self) -> impl Future<Output = ()> + Send;

    fn addr(&self) -> &Self::Addr;

    fn create_conn(
        &mut self,
        addr: Self::Addr,
    ) -> impl Future<Output = (Sender<T>, Receiver<T>)> + Send;

    fn get_new_conn(&mut self) -> impl Future<Output = Option<(Sender<T>, Receiver<T>)>> + Send;
}
