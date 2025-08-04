use crate::message::PollinationMessage;
use serde::{Deserialize, Serialize};
use std::{fmt, future::Future, hash::Hash};
use tokio::sync::mpsc::{Receiver, Sender};

const DEFAULT_CHANNEL_SIZE: usize = 10;

#[cfg(feature = "axum")]
pub mod axum;

pub trait Engine: 'static {
    type Addr: Clone + Serialize + for<'de> Deserialize<'de> + Hash + fmt::Display + Send;
    type Error: std::error::Error + 'static;

    fn run_background(
        self,
    ) -> impl Future<
        Output = Result<(Sender<EngineRequest<Self::Addr>>, Receiver<EngineEvent>), Self::Error>,
    > + Send;
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
