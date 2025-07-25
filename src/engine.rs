use crate::{
    ds::WalkieTalkie,
    message::PollinationMessage,
    nucleus::{Nucleus, NucleusError, NucleusResponse},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, hash::Hash};
use tokio::sync::mpsc::{Receiver, Sender};

const DEFAULT_CHANNEL_SIZE: usize = 10;

#[cfg(feature = "axum")]
pub mod axum;

pub trait Engine: 'static {
    type Addr: Clone + Serialize + for<'de> Deserialize<'de> + Hash + fmt::Display + Send;
    type Error: std::error::Error + 'static;

    async fn run_background(
        self,
    ) -> Result<(Sender<EngineRequest<Self::Addr>>, Receiver<EngineEvent>), Self::Error>;
}

pub struct EngineRequest<A> {
    pub pollination_msg: PollinationMessage,
    pub addr: A,
    pub tx: Sender<PollinationMessage>,
}

pub struct EngineEvent {
    pub pollination_msg: PollinationMessage,
    pub tx: Sender<PollinationMessage>,
}
