use pollination_simulation::*;

#[derive(Debug, Clone)]
struct TestNode {
    id: usize,
}

impl Simulee for TestNode {
    type Snapshot = TestNode;
    type Config = ();
    type Message = ();
    type Action = TestAction;
    type HistoricalEvent = ();

    fn new(config: &Config<Self::Config>, index: usize, rng: u64) -> Self {
        Self { id: index }
    }

    fn list_actions(
        &self,
        wall_time: u64,
        mail_available: bool,
        config: &Self::Config,
    ) -> impl Iterator<Item = (Self::Action, f64)> {
        vec![].into_iter()
    }

    fn step(
        &mut self,
        event: Self::Action,
        message: Option<Self::Message>,
        wall_time: u64,
        config: &Self::Config,
    ) -> Self::HistoricalEvent {
        ()
    }

    fn snapshot(&self) -> Self::Snapshot {
        self.clone()
    }
}

enum TestAction {
    One,
    Two,
}

impl Action for TestAction {
    fn takes_mail(&self) -> bool {
        match &self {
            Self::One => true,
            Self::Two => false,
        }
    }
}

#[test]
fn basic_history() {
    let config = Config::new(5, 1234, ());
    let mut sim: Sim<TestNode> = Sim::new(&config);
    /*
    let mut sim = Sim::new(StartupConfig {
        node_count: 5,
        seed: 123,
        connections: 2,
    });

    sim.step(&StepConfig::default());

    println!("History: {:?}", sim.history);
    */
}
