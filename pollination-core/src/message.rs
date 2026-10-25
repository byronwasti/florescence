use crate::{membership_hash::MembershipHash, node_info::NodeInfo, origin_hash::OriginHash};
use serde::{Deserialize, Serialize};
use treeclocks::{EventTree, IdTree, Patch};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollinationMessage<A> {
    pub uuid: Uuid,
    pub id: IdTree,
    pub timestamp: EventTree,
    pub origin_hash: OriginHash,
    pub membership_hash: MembershipHash,
    pub unique_count: usize,
    pub patch: Option<Patch<NodeInfo<A>>>,
    pub new_membership: bool,
    // TODO: Derive from Patch
    pub full_patch: bool,
}

impl<A: std::fmt::Debug> std::fmt::Display for PollinationMessage<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(
            f,
            "PollinationMessage {{ uuid={} id={} timestamp={} origin_hash={} membership_hash={} unique_count={} new_membership={:?} full={} patch={} }}",
            self.uuid,
            self.id,
            self.timestamp,
            self.origin_hash.u64(),
            self.membership_hash.u64(),
            self.unique_count,
            self.new_membership,
            self.full_patch,
            if let Some(patch) = &self.patch {
                format!("{patch}")
            } else {
                "None".to_string()
            },
        )
    }
}
