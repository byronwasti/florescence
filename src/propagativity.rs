use crate::constants;
use std::fmt;
use std::time::Instant;
use treeclocks::IdTree;

pub trait TimeoutProvider {
    fn start() -> Self;
    fn elapsed(&self) -> bool;
}

impl TimeoutProvider for Instant {
    fn start() -> Self {
        Instant::now()
    }

    fn elapsed(&self) -> bool {
        // TODO: Make the timeout configurable
        self.elapsed() > constants::PROPAGATION_TIMEOUT
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Propagativity<T = Instant> {
    Unknown,
    Propagating(IdTree),
    Resting(IdTree, T),
}

impl<T: TimeoutProvider> Propagativity<T> {
    pub(crate) fn id(&self) -> Option<&IdTree> {
        use Propagativity::*;
        match self {
            Propagating(id) | Resting(id, _) => Some(id),
            Unknown => None,
        }
    }

    pub(crate) fn force_propagating(&mut self) {
        use Propagativity::*;
        let s = std::mem::replace(self, Self::Unknown);
        match s {
            Propagating(id) | Resting(id, ..) => {
                *self = Propagativity::Propagating(id);
            }
            Unknown => {}
        }
    }

    pub(crate) fn resting(id: IdTree) -> Self {
        Propagativity::Resting(id, T::start())
    }

    pub(crate) fn propagate(&mut self) -> Option<IdTree> {
        use Propagativity::*;
        let s = std::mem::replace(self, Self::Unknown);
        match s {
            Propagating(id) => {
                let (id, peer_id) = id.fork();
                *self = Propagativity::Resting(id, T::start());
                Some(peer_id)
            }
            Resting(id, timeout) => {
                if timeout.elapsed() {
                    let (id, peer_id) = id.fork();
                    *self = Propagativity::Resting(id, T::start());
                    Some(peer_id)
                } else {
                    *self = Propagativity::Resting(id, timeout);
                    None
                }
            }
            Unknown => None,
        }
    }
}

impl<T: Default> Default for Propagativity<T> {
    fn default() -> Self {
        Self::Unknown
    }
}

impl fmt::Display for Propagativity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        use Propagativity::*;
        match self {
            Unknown => write!(f, "x"),
            Propagating(id) => write!(f, "p.{id}"),
            Resting(id, _timeout) => write!(f, "r.{id}"),
        }
    }
}
