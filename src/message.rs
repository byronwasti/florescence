use crate::{reality_token::RealityToken, serialization::*};
use serde::{Deserialize, Serialize};
use treeclocks::{EventTree, IdTree};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PollinationMessage {
    Heartbeat {
        uuid: Uuid,
        id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
    },
    Update {
        uuid: Uuid,
        id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
        patch: BinaryPatch,
    },
    RealitySkew {
        uuid: Uuid,
        id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
        patch: BinaryPatch,
        peer_count: usize,
    },
    NewMember {
        uuid: Uuid,
    },
    Seed {
        uuid: Uuid,
        id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
        patch: BinaryPatch,
        peer_count: usize,
        new_id: Option<IdTree>,
    },
}

impl PollinationMessage {
    pub fn timestamp(&self) -> Option<&EventTree> {
        use PollinationMessage::*;
        match self {
            NewMember { .. } => None,
            Heartbeat { timestamp, .. }
            | Update { timestamp, .. }
            | RealitySkew { timestamp, .. }
            | Seed { timestamp, .. } => Some(timestamp),
        }
    }

    pub fn id(&self) -> Option<&IdTree> {
        use PollinationMessage::*;
        match self {
            NewMember { .. } => None,
            Heartbeat { id, .. } | Update { id, .. } | RealitySkew { id, .. } | Seed { id, .. } => {
                Some(id)
            }
        }
    }

    pub fn light_clone(&self) -> Self {
        let mut new = self.clone();
        // Assuming the compiler will optimize away the clone
        new.delete_patch();
        new
    }

    fn delete_patch(&mut self) {
        use PollinationMessage::*;
        match self {
            Heartbeat { .. } | NewMember { .. } => {}
            Update { patch, .. } | RealitySkew { patch, .. } | Seed { patch, .. } => {
                let _ = std::mem::take(patch);
            }
        }
    }
}

impl std::fmt::Display for PollinationMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        use PollinationMessage::*;
        match self {
            Heartbeat {
                uuid,
                id,
                timestamp,
                reality_token,
            } => {
                write!(f, "HB - {uuid} - {id} - {timestamp} - {reality_token}")
            }
            Update {
                uuid,
                id,
                timestamp,
                reality_token,
                patch,
            } => {
                write!(
                    f,
                    "UP - {uuid} - {id} - {timestamp} - {reality_token} - {patch}"
                )
            }
            RealitySkew {
                uuid,
                id,
                timestamp,
                reality_token,
                peer_count,
                patch,
            } => {
                write!(
                    f,
                    "RS - {uuid} - {id} - {timestamp} - {reality_token} - {peer_count} - {patch}"
                )
            }
            NewMember { uuid } => {
                write!(f, "NM - {uuid}")
            }
            Seed {
                uuid,
                id,
                timestamp,
                reality_token,
                new_id,
                peer_count,
                patch,
            } => {
                write!(
                    f,
                    "SE - {uuid} - {id} - {timestamp} - {reality_token} - {peer_count} - {new_id:?} - {patch}"
                )
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct BinaryPatch {
    inner: Vec<u8>,
}

impl std::fmt::Display for BinaryPatch {
    #[cfg(not(feature = "json"))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "[")?;
        for b in self.inner.iter() {
            write!(f, "{b:02X} ")?;
        }
        write!(f, "]")?;

        Ok(())
    }

    #[cfg(feature = "json")]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let s = String::from_utf8(self.inner.clone()).map_err(|_| std::fmt::Error {})?;
        write!(f, "{s}")
    }
}

impl BinaryPatch {
    pub fn new<T: Serialize>(val: T) -> Result<Self, SerializeError> {
        let inner = serialize(val)?;
        Ok(Self { inner })
    }

    pub fn decode<T: for<'de> Deserialize<'de>>(self) -> Result<T, DeserializeError> {
        let res = deserialize(self.inner)?;
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use treeclocks::{ItcMap, Patch};

    #[test]
    fn test_binary_patch_behavior() {
        let patch = BinaryPatch::new("this is some string".to_string()).unwrap();
        let string: String = patch.decode().unwrap();
        assert_eq!(string, "this is some string".to_string());
    }

    #[test]
    fn test_binary_patch_behavior_itc_map() {
        let mut m = ItcMap::new();
        m.insert(IdTree::One, 1);

        let p_in = m.diff(&EventTree::new());

        let p_bin = BinaryPatch::new(p_in).unwrap();
        let p_out = p_bin.decode::<Patch<i32>>().unwrap();

        //assert_eq!(string, bbh);
    }
}
