use crate::{
    constants,
    engine::{Engine, EngineEvent, EngineRequest},
    message::{PollinationMessage, Topic},
    nucleus::{Nucleus, NucleusError},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, hash::Hash, time::Duration};
use thiserror::Error;
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{MissedTickBehavior, interval},
};
use uuid::Uuid;

pub struct Flower {}

impl Flower {
    pub fn builder<E: Engine>() -> FlowerBuilder<E> {
        FlowerBuilder::new()
    }
}

struct FlowerCore<E: Engine> {
    uuid: Uuid,
    nuclei: HashMap<Topic, NucleiState<E::Addr>>,
    engine_request_tx: Sender<EngineRequest<E::Addr>>,
    engine_event_rx: Receiver<EngineEvent>,
    own_addr: E::Addr,
}

struct NucleiState<A> {
    nucleus: Nucleus<A>,
    seed_list: Vec<A>,
}

impl<E> FlowerCore<E>
where
    E: Engine,
{
    fn handle(&self) -> Flower {
        Flower {}
    }

    async fn run(mut self) {
        let mut heartbeat = interval(constants::HEARTBEAT_TICK_TIME);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut grim_reaper = interval(constants::RECLAIM_IDS_TICK_TIME);
        grim_reaper.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    let mut msgs = vec![];
                    for (topic, nuclei_state) in self.nuclei.iter_mut() {
                        nuclei_state.nucleus.bump();
                        if let Some(msg) = nuclei_state.nucleus.msg_heartbeat() {
                            msgs.push(msg);
                        }
                    }

                    for msg in msgs.drain(..) {
                        self.send(msg).await;
                    }
                }

                _ = grim_reaper.tick() => {
                    let mut msgs = vec![];
                    for (topic, nuclei_state) in self.nuclei.iter_mut() {
                        if nuclei_state.nucleus.reap_souls() {
                            if let Some(msg) = nuclei_state.nucleus.msg_heartbeat() {
                                msgs.push(msg);
                            }
                        }
                    }
                    for msg in msgs.drain(..) {
                        self.send(msg).await;
                    }
                }

                event  = self.engine_event_rx.recv() => {
                    if let Some(EngineEvent { tx, pollination_msg: msg }) = event {
                        if let Some(nuclei_state) = self.nuclei.get_mut(&msg.topic()) {
                            // TODO: Handle cleanup of nucleus
                            let return_msg = match nuclei_state.nucleus.handle_message(msg) {
                                Ok(res) => res.response,
                                Err(err) => {
                                    error!("Error handling message: {err:?}");
                                    None
                                }
                            };

                            if let Some(msg) = return_msg  {
                                tx.send(msg);
                            }
                        }
                    } else {
                        error!("Engine shut down.");
                        break
                    }
                }
            }
        }
    }

    async fn send(&mut self, msg: PollinationMessage) {
        todo!()
    }
}

pub struct FlowerBuilder<E: Engine> {
    engine: Option<E>,
    uuid: Option<Uuid>,
    own_addr: Option<E::Addr>,
    seed_list: Vec<E::Addr>,
}

impl<E> FlowerBuilder<E>
where
    E: Engine,
{
    pub fn new() -> Self {
        Self {
            engine: None,
            uuid: None,
            own_addr: None,
            seed_list: vec![],
        }
    }

    pub fn engine(mut self, engine: E) -> Self {
        self.engine = Some(engine);
        self
    }

    pub fn uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = Some(uuid);
        self
    }

    pub fn own_addr(mut self, own_addr: E::Addr) -> Self {
        self.own_addr = Some(own_addr);
        self
    }

    pub fn seed_list(mut self, seed_list: Vec<E::Addr>) -> Self {
        self.seed_list = seed_list;
        self
    }

    pub async fn start(self) -> Result<Flower, FlowerError> {
        let uuid = self.uuid.unwrap_or(Uuid::new_v4());
        let nuclei = HashMap::new();
        let own_addr = self.own_addr.ok_or(FlowerError::MissingOwnAddr)?;

        let (engine_request_tx, engine_event_rx) = self
            .engine
            .ok_or(FlowerError::MissingEngine)?
            .run_background()
            .await
            .map_err(|err| FlowerError::EngineError(Box::new(err)))?;

        let core: FlowerCore<E> = FlowerCore {
            uuid,
            nuclei,
            own_addr,
            engine_request_tx,
            engine_event_rx,
        };

        let handle = core.handle();

        //tokio::spawn(background_runner(core));
        tokio::spawn(core.run());

        Ok(handle)
    }
}

#[derive(Debug, Error)]
pub enum FlowerError {
    #[error("No `own_addr` set")]
    MissingOwnAddr,

    #[error("No `engine` set")]
    MissingEngine,

    #[error("Engine error")]
    EngineError(#[from] Box<dyn std::error::Error>),
}
