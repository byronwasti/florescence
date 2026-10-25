use crate::node_info::NodeInfo;
use serde::{Deserialize, Serialize};
use simplehash::fnv1a_64;
use treeclocks::ItcMap;

#[derive(
    PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy, Serialize, Deserialize, Hash, Default,
)]
pub struct MembershipHash(u64);

impl MembershipHash {
    pub fn from_itc_map<A>(itc_map: &ItcMap<NodeInfo<A>>) -> Self {
        itc_map.fold_recursive(
            &|node: &NodeInfo<A>| Self::hash(node.uuid.as_bytes()),
            &|l, r| Self::combine(l, r),
        )
    }

    fn hash(bytes: &[u8]) -> Self {
        Self(fnv1a_64(bytes))
    }

    fn combine(left: MembershipHash, right: MembershipHash) -> Self {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&left.0.to_le_bytes());
        bytes[8..].copy_from_slice(&right.0.to_le_bytes());
        Self(fnv1a_64(&bytes))
    }

    pub fn u64(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_membership_hashing() {
        let mut m0 = ItcMap::new();
        let mut m1 = ItcMap::new();

        let mut i0 = IdTree::one();
        let (i0, i1) = i0.fork();

        let n0: NodeInfo<_> = NodeInfo::new(Uuid::from_u128(1030), 0);
        let n1: NodeInfo<_> = NodeInfo::new(Uuid::from_u128(2313), 1);

        m0.insert(i0.clone(), n0.clone());
        m0.insert(i1.clone(), n1.clone());

        // Different tree
        m1.insert(i1.clone(), n0.clone());
        m1.insert(i0.clone(), n1.clone());

        let h0 = MembershipHash::from_itc_map(&m0);
        let h1 = MembershipHash::from_itc_map(&m1);

        assert_ne!(h0, h1);
    }
}
