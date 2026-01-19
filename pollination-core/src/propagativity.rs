use std::fmt;
use treeclocks::IdTree;

#[derive(Debug, Clone, Default)]
pub(crate) enum Propagativity {
    #[default]
    Unknown,
    Propagating(IdTree),
    Resting(IdTree),
}

impl Propagativity {
    pub(crate) fn id(&self) -> Option<&IdTree> {
        use Propagativity::*;
        match self {
            Propagating(id) | Resting(id) => Some(id),
            Unknown => None,
        }
    }

    pub(crate) fn propagating(&self) -> bool {
        matches!(self, Propagativity::Propagating(_))
    }

    pub(crate) fn set_propagating(&mut self) -> bool {
        use Propagativity::*;
        let s = std::mem::replace(self, Self::Unknown);
        match s {
            Propagating(id) | Resting(id, ..) => {
                *self = Propagativity::Propagating(id);
                true
            }
            Unknown => false,
        }
    }

    pub(crate) fn resting(id: IdTree) -> Self {
        Propagativity::Resting(id)
    }

    pub(crate) fn propagate(&mut self) -> Option<IdTree> {
        use Propagativity::*;
        let s = std::mem::replace(self, Self::Unknown);
        match s {
            Propagating(id) => {
                let (id, peer_id) = id.fork();
                *self = Propagativity::Resting(id);
                Some(peer_id)
            }
            Resting(id) => {
                *self = Propagativity::Resting(id);
                None
            }
            Unknown => None,
        }
    }
}

impl fmt::Display for Propagativity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        use Propagativity::*;
        match self {
            Unknown => write!(f, "x"),
            Propagating(id) => write!(f, "p.{id}"),
            Resting(id) => write!(f, "r.{id}"),
        }
    }
}
