use crate::constants;
use std::fmt;
use std::time::Instant;
use treeclocks::IdTree;

#[derive(Debug, Clone)]
pub(crate) enum Propagativity {
    Unknown,
    Propagating(IdTree),
    Resting(IdTree, Instant),
}

impl Propagativity {
    pub(crate) fn id(&self) -> Option<&IdTree> {
        use Propagativity::*;
        match self {
            Propagating(id) | Resting(id, _) => Some(id),
            Unknown => None,
        }
    }

    pub(crate) fn force_propagating(&mut self) {
        use Propagativity::*;
        let s = std::mem::take(self);
        match s {
            Propagating(id) | Resting(id, ..) => {
                *self = Propagativity::Propagating(id);
            }
            Unknown => {}
        }
    }

    pub(crate) fn propagate(&mut self) -> Option<IdTree> {
        use Propagativity::*;
        let s = std::mem::take(self);
        match s {
            Propagating(id) => {
                let (id, peer_id) = id.fork();
                *self = Propagativity::Resting(id, Instant::now());
                Some(peer_id)
            }
            Resting(id, timeout) => {
                if timeout.elapsed() > constants::PROPAGATION_TIMEOUT {
                    let (id, peer_id) = id.fork();
                    *self = Propagativity::Resting(id, Instant::now());
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

impl Default for Propagativity {
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
