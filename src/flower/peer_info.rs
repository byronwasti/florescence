use serde::{Deserialize, Serialize};
use std::fmt;
use treeclocks::{EventTree, IdTree, ItcMap};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PeerInfo<A> {
    id: IdTree,
    addr: A,
    // topics: Vec<Topic>,
}

impl<A> PeerInfo<A> {
    pub(crate) fn new(id: IdTree, addr: A) -> Self {
        Self { id, addr }
    }
}

impl<A: fmt::Display> fmt::Display for PeerInfo<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{{{} {}}}", self.id, self.addr)
    }
}
