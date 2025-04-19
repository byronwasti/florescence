use crate::connection::Connection;

mod tonic_engine;

pub use tonic_engine::TonicEngine;

pub trait Engine: Send {
    type Addr: PartialEq;
    // TODO: Associated error type

    /// Must be non-blocking
    // TODO: This is a weird API; maybe it should return engine?
    fn start(&mut self);

    fn addr(&self) -> &Self::Addr;

    fn create_conn(&mut self, addr: Self::Addr) -> Connection;

    fn get_new_conn(&mut self) -> impl Future<Output = Option<Connection>> + Send;
}
