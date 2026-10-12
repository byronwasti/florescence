use pollination_simulation::{PollinationConfig, SimulatedPollinationCore};
use pollination_simulator::{Config, Sim};

fn main() {
    let mut sim: Sim<SimulatedPollinationCore> = Sim::new(Config {
        node_count: 50,
        seed: 1234,
        message_queue_size: 5,
        custom: PollinationConfig {
            rand_robin_count: 2,
        },
    });

    for _ in 0..10_000 {
        sim.step().expect("No error");
    }
}
