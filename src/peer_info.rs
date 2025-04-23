use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PeerInfo<A> {
    addr: A,
    // topics: Vec<Topic>,
}

impl<A> PeerInfo<A> {
    pub(crate) fn new(addr: A) -> Self {
        Self { addr }
    }
}

impl<A: fmt::Display> fmt::Display for PeerInfo<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{{{}}}", self.addr)
    }
}
