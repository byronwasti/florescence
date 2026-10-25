use crate::membership_hash::MembershipHash;
use serde::{Deserialize, Serialize};

#[derive(
    PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy, Serialize, Deserialize, Hash, Default,
)]
pub struct OriginHash(u64);

impl OriginHash {
    pub fn from_membership_hash(mh: MembershipHash) -> Self {
        Self(mh.u64())
    }

    pub fn u64(&self) -> u64 {
        self.0
    }
}
