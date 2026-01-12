use pollination_simulator::*;
use rand::Rng;

#[derive(Debug, Clone)]
struct TestNode {
    id: NodeIndex,
    handled_messages: u64,
    sent_messages: u64,
}

impl Simulee for TestNode {
    type Config = ();
    type Message = ();
    type HistoricalEvent = ();

    fn new<R: Rng + ?Sized>(rng: &mut R, config: &Config<Self::Config>, id: NodeIndex) -> Self {
        Self {
            id,
            handled_messages: 0,
            sent_messages: 0,
        }
    }

    fn step<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        config: &Config<Self::Config>,
        wall_time: u64,
        delivery: &mut Option<Delivery<Self::Message>>,
    ) -> Option<(Self::HistoricalEvent, Vec<(NodeIndex, Self::Message)>)> {
        None
    }
}

#[test]
fn basic_history() {
    let config = Config::new(5, 1234, ());
    let mut sim: Sim<TestNode> = Sim::new(config);

    let nodes: Vec<_> = sim.nodes().collect();
    insta::assert_debug_snapshot!(nodes);

    sim.step();
}
