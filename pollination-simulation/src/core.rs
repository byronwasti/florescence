use pollination::{EventTree, core::*};
use pollination_simulator::{Config, Delivery, NodeIndex, Simulee};
use rand::{
    distr::{Distribution, weighted::WeightedIndex},
    prelude::*,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SimulatedPollinationCore {
    inner: PollinationCore<NodeIndex>,
}

impl SimulatedPollinationCore {
    pub fn membership_hash(&self) -> MembershipHash {
        self.inner.membership_hash()
    }

    pub fn timestamp(&self) -> &EventTree {
        self.inner.timestamp()
    }

    pub fn inner(&self) -> &PollinationCore<NodeIndex> {
        &self.inner
    }
}

impl Simulee for SimulatedPollinationCore {
    type Config = PollinationConfig;
    type Message = PollinationMessage<NodeIndex>;
    type HistoricalEvent = PollinationEvent<NodeIndex>;

    fn new<R: Rng + ?Sized>(rng: &mut R, _config: &Config<Self::Config>, id: NodeIndex) -> Self {
        let inner = PollinationCore::new(Uuid::from_u128(rng.random()), id);
        Self { inner }
    }

    #[allow(clippy::type_complexity)]
    fn step<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        config: &Config<Self::Config>,
        nodes: &[NodeIndex],
        wall_time: u64,
        delivery: &mut Option<Delivery<Self::Message>>,
    ) -> Option<(Self::HistoricalEvent, Vec<(NodeIndex, Self::Message)>)> {
        let mut events = vec![];
        let mut weights = vec![];

        events.push(StepOptions::HandleMessage);
        weights.push(100);

        events.push(StepOptions::Heartbeat);
        weights.push(2);

        events.push(StepOptions::Skip);
        weights.push(2);

        let dist = WeightedIndex::new(&weights).expect("Invalid random weights");
        let event = events[dist.sample(rng)];

        match event {
            StepOptions::Skip => None,
            StepOptions::Heartbeat => {
                let msg = self.inner.heartbeat_message();

                let msgs = nodes
                    .iter()
                    .choose_multiple(rng, config.custom.rand_robin_count)
                    .into_iter()
                    .map(|id| (*id, msg.clone()))
                    .collect();

                Some((PollinationEvent::Heartbeat, msgs))
            }
            StepOptions::HandleMessage => {
                let mail = delivery.as_mut()?.take();
                let from = mail.from;
                let msg = mail.msg;

                let res = self.inner.handle_message(msg);
                match res {
                    Some(msg) => Some((PollinationEvent::HandleMessage, vec![(from, msg)])),
                    None => Some((PollinationEvent::HandleMessage, vec![])),
                }
            }
        }
    }
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
