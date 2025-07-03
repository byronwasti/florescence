use crate::{
    ds::WalkieTalkie,
    message::PollinationMessage,
    nucleus::{Nucleus, NucleusError, NucleusResponse},
};

pub mod axum;

pub trait Engine {
    type Addr;
    type Error;

    async fn run(
        self,
        addr: Self::Addr,
    ) -> Result<WalkieTalkie<EngineMessage<Self::Addr>>, Self::Error>;
}

pub struct EngineMessage<A> {
    pollination_msg: PollinationMessage,
    addr: A,
}
