use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct RealityToken(u64);

impl RealityToken {
    pub fn new<I: Hash>(id: &I) -> Self {
        let mut s = DefaultHasher::new();
        id.hash(&mut s);
        Self(!s.finish())
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
