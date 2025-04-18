use rand::Rng;
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RealityToken(u64);

impl RealityToken {
    pub fn new() -> Self {
        let mut rng = rand::rng();
        Self(rng.random::<u64>())
    }

    pub fn increment<I: Hash>(&mut self, id: I) {
        let mut s = DefaultHasher::new();
        id.hash(&mut s);
        self.0 = self.0 ^ s.finish();
    }

    pub fn get(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for RealityToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.0)
    }
}
