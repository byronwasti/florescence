pub use pollination::{treeclocks::EventTree, *};
pub use pollination_router::*;
use pollination_simulator::{Config, Delivery, NodeIndex, Simulee};
use rand::{
    distr::{Distribution, weighted::WeightedIndex},
    prelude::*,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SimulatedRouter {
    inner: PollinationRouter<NodeIndex>,
    first_heartbeat: bool,
}

impl SimulatedRouter {
    pub fn membership_hash(&self) -> MembershipHash {
        self.inner.core().membership_hash()
    }

    pub fn origin_hash(&self) -> OriginHash {
        self.inner.core().origin_hash()
    }

    pub fn timestamp(&self) -> &EventTree {
        self.inner.core().timestamp()
    }

    pub fn inner(&self) -> &PollinationCore<NodeIndex> {
        &self.inner.core()
    }
}

impl Simulee for SimulatedRouter {
    type Config = PollinationConfig;
    type Message = PollinationMessage<NodeIndex>;
    type HistoricalEvent = PollinationEvent<NodeIndex>;

    fn new<R: Rng + ?Sized>(rng: &mut R, config: &Config<Self::Config>, id: NodeIndex) -> Self {
        let core = PollinationCore::new(Uuid::from_u128(rng.random()), id);
        let mut peers: VecDeque<_> = (0..config.node_count).map(|n| NodeIndex::new(n)).collect();
        peers.make_contiguous().shuffle(rng);
        let inner = PollinationRouter::new(core, peers);
        Self {
            inner,
            first_heartbeat: false,
        }
    }

    #[allow(clippy::type_complexity)]
    fn step<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        config: &Config<Self::Config>,
        nodes: &[NodeIndex],
        _wall_time: u64,
        delivery: &mut Option<Delivery<Self::Message>>,
    ) -> Option<(Self::HistoricalEvent, Vec<(NodeIndex, Self::Message)>)> {
        let mut events = vec![];
        let mut weights = vec![];

        events.push(StepOptions::HandleMessage);
        weights.push(100);

        events.push(StepOptions::Heartbeat);
        weights.push(if !self.first_heartbeat {
            self.first_heartbeat = true;
            200
        } else {
            2
        });

        events.push(StepOptions::Skip);
        weights.push(5);

        let dist = WeightedIndex::new(&weights).expect("Invalid random weights");
        let event = events[dist.sample(rng)];

        match event {
            StepOptions::Skip => None,
            StepOptions::Heartbeat => {
                let msgs = self.inner.heartbeat();
                Some((PollinationEvent::Heartbeat, msgs))
            }
            StepOptions::HandleMessage => {
                let mail = delivery.as_mut()?.take();
                let from = mail.from;
                let msg = mail.msg;

                let res = self.inner.handle_message(from, msg);
                Some((PollinationEvent::HandleMessage, res))
            }
        }
    }
}

struct RouterConfig {
    core_config: PollinationConfig,
    lookback_count: usize,
    alpha: u64,
    beta: u64,
}

#[derive(Debug, Copy, Clone)]
enum StepOptions {
    Heartbeat,
    HandleMessage,
    Skip,
}

#[derive(Debug)]
pub enum PollinationEvent<A> {
    Heartbeat,
    HandleMessage,
    One(A),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PollinationConfig {
    pub rand_robin_count: usize,
}
