pub use pollination::{EventTree, core::*};
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
    first_heartbeat: bool,
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
        wall_time: u64,
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
                self.inner.increment();
                let msg = self.inner.heartbeat_message();

                let msgs = nodes
                    .iter()
                    .sample(rng, config.custom.rand_robin_count)
                    .into_iter()
                    .map(|id| (*id, msg.clone()))
                    .collect();

                Some((PollinationEvent::Heartbeat, msgs))
            }
            StepOptionHandleMessage => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use pollination_simulator::Sim;
    use rand_chacha::ChaCha12Rng;

    #[test]
    #[ignore]
    fn basic_convergence() {
        tracing_subscriber::fmt().with_test_writer().try_init();
        let config = Config {
            node_count: 20,
            seed: 1337,
            message_queue_size: 5,
            custom: PollinationConfig {
                rand_robin_count: 2,
            },
        };
        let mut sim: Sim<SimulatedPollinationCore> = Sim::new(config.clone());

        for _ in 0..25_500 {
            sim.step();
        }

        assert!(sim.has_converged(|s: &SimulatedPollinationCore| s.membership_hash()));
    }

    #[test]
    #[ignore]
    fn deterministic_simulation() {
        tracing_subscriber::fmt().with_test_writer().try_init();
        let config = Config {
            node_count: 20,
            seed: 1337,
            message_queue_size: 5,
            custom: PollinationConfig {
                rand_robin_count: 2,
            },
        };
        let mut sim1: Sim<SimulatedPollinationCore> = Sim::new(config.clone());
        let mut sim2: Sim<SimulatedPollinationCore> = Sim::new(config);

        for idx in 0..1000 {
            println!("============================== {idx} ================================");
            sim1.step();
            println!("Sim1: {:?}", sim1.history().last());

            sim2.step();
            println!("Sim2: {:?}", sim2.history().last());

            let s1: u8 = sim1.dangerous_get_rng().random();
            let s2: u8 = sim2.dangerous_get_rng().random();
            assert_eq!(s1, s2);

            let s1: u8 = sim1.dangerous_get_rng().random();
            let s2: u8 = sim2.dangerous_get_rng().random();
            assert_eq!(s1, s2);

            let s1: u8 = sim1.dangerous_get_rng().random();
            let s2: u8 = sim2.dangerous_get_rng().random();
            assert_eq!(s1, s2);
        }
    }

    #[test]
    #[ignore]
    fn deterministic_shuffle() {
        tracing_subscriber::fmt().with_test_writer().try_init();
        let seed = 1234;
        let mut rng1 = ChaCha12Rng::seed_from_u64(seed);
        let mut rng2 = ChaCha12Rng::seed_from_u64(seed);

        for idx in 0..10000 {
            let mut arr1 = (0..20).collect::<Vec<_>>();
            let mut arr2 = (0..20).collect::<Vec<_>>();

            arr1.shuffle(&mut rng1);
            arr2.shuffle(&mut rng2);

            assert_eq!(&arr1, &arr2);
            assert_eq!(rng1.random::<u64>(), rng2.random::<u64>());
        }
    }
}
