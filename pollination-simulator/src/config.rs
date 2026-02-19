use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
