use crate::{
    ds::WalkieTalkie,
    message::PollinationMessage,
    nucleus::{Nucleus, NucleusError, NucleusResponse},
};
use tokio::sync::mpsc::Sender;

const DEFAULT_CHANNEL_SIZE: usize = 10;

//#[cfg(feature = "axum")]
pub mod axum;

pub trait Engine {
    type Addr;
    type Error;

    async fn run(
        self,
        addr: Self::Addr,
    ) -> Result<WalkieTalkie<EngineRequest<Self::Addr>, EngineEvent>, Self::Error>;
}

pub struct EngineRequest<A> {
    pollination_msg: PollinationMessage,
    addr: A,
    tx: Sender<PollinationMessage>,
}

pub struct EngineEvent {
    pollination_msg: PollinationMessage,
    tx: Sender<PollinationMessage>,
}
