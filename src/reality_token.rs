use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RealityToken(u128);

impl RealityToken {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid.as_u128())
    }

    pub fn zero() -> Self {
        Self(0)
    }

    pub fn push(&mut self, uuid: Uuid) {
        self.0 ^= uuid.as_u128()
    }

    pub fn get(&self) -> u128 {
        self.0
    }
}

impl std::fmt::Display for RealityToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.0)
    }
}
