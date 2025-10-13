use crate::{
    clock::Clock,
    constants,
    engine::{Engine, EngineEvent},
    message::{PollinationMessage, Topic},
    nucleus::Nucleus,
    router::Router,
};
use std::collections::HashMap;
use thiserror::Error;
use tokio::time::{MissedTickBehavior, interval};
use uuid::Uuid;

pub struct Flower<E: Engine, C, R> {
    #[allow(unused)]
    uuid: Uuid,
    engine: Option<E>,
    clock: C,
    router: R,
    nuclei: HashMap<Topic, NucleiState<E::Addr>>,
    //engine_request_tx: Sender<EngineRequest<E::Addr>>,
    //engine_event_rx: Receiver<EngineEvent>,
    #[allow(unused)]
    own_addr: E::Addr,
}

struct NucleiState<A> {
    nucleus: Nucleus<A>,
    #[allow(unused)]
    seed_list: Vec<A>,
}

impl<E, C, R> Flower<E, C, R>
where
    E: Engine,
    C: Clock,
    R: Router,
{
    pub fn builder() -> FlowerBuilder<E, C, R> {
        FlowerBuilder::new()
    }

    pub async fn run(mut self) -> Result<(), FlowerError> {
        let (_engine_request_tx, mut engine_event_rx) = self
            .engine
            .take()
            .ok_or(FlowerError::MissingEngine)?
            .run_background()
            .await
            .map_err(|err| FlowerError::EngineError(Box::new(err)))?;

        let mut heartbeat = interval(constants::HEARTBEAT_TICK_TIME);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut grim_reaper = interval(constants::RECLAIM_IDS_TICK_TIME);
        grim_reaper.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    let mut msgs = vec![];
                    for (_topic, nuclei_state) in self.nuclei.iter_mut() {
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
                    for (_topic, nuclei_state) in self.nuclei.iter_mut() {
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

                event  = engine_event_rx.recv() => {
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
                                let res = tx.send(msg).await;
                                if let Err(err) = res {
                                    error!("Error sending via mpsc: {err}");
                                }
                            }
                        }
                    } else {
                        error!("Engine shut down.");
                        break Ok(())
                    }
                }
            }
        }
    }

    async fn send(&mut self, _msg: PollinationMessage) {
        todo!()
    }
}

pub struct FlowerBuilder<E: Engine, C, R> {
    engine: Option<E>,
    clock: Option<C>,
    router: Option<R>,
    uuid: Option<Uuid>,
    own_addr: Option<E::Addr>,
    seed_list: Vec<E::Addr>,
}

impl<E, C, R> FlowerBuilder<E, C, R>
where
    E: Engine,
    C: Clock,
    R: Router,
{
    pub fn new() -> Self {
        Self {
            engine: None,
            clock: None,
            router: None,
            uuid: None,
            own_addr: None,
            seed_list: vec![],
        }
    }

    pub fn engine(mut self, engine: E) -> Self {
        self.engine = Some(engine);
        self
    }

    pub fn clock(mut self, clock: C) -> Self {
        self.clock = Some(clock);
        self
    }

    pub fn router(mut self, router: R) -> Self {
        self.router = Some(router);
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

    pub fn build(self) -> Result<Flower<E, C, R>, FlowerError> {
        let uuid = self.uuid.unwrap_or(Uuid::new_v4());
        let nuclei = HashMap::new();
        let own_addr = self.own_addr.ok_or(FlowerError::MissingOwnAddr)?;

        Ok(Flower {
            uuid,
            nuclei,
            own_addr,
            engine: self.engine,
            clock: self.clock.unwrap_or_else(|| todo!()),
            router: self.router.unwrap_or_else(|| todo!()),
        })
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
