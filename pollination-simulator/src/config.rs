#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Config<C> {
    pub node_count: usize,
    pub seed: u64,
    pub custom: C,
}

impl<C> Config<C> {
    pub fn new(node_count: usize, seed: u64, custom: C) -> Config<C> {
        Config {
            node_count,
            seed,
            custom,
        }
    }
}

/*
pub struct StartupConfig {
    pub node_count: usize,
    pub seed: u64,
    pub connections: usize,
}

pub struct StepConfig {
    pub timeout_propagativity: u64,
    pub timeout_heartbeat: u64,
    pub timeout_reap: u64,

    /// Only used if connections == 0
    pub rand_robin_count: usize,
}

impl Default for StepConfig {
    fn default() -> StepConfig {
        StepConfig {
            timeout_propagativity: 13,
            timeout_heartbeat: 5,
            timeout_reap: 8,
            rand_robin_count: 2,
        }
    }
}
*/
