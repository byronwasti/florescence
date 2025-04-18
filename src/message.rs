use crate::reality_token::RealityToken;
use serde::{Deserialize, Serialize};
use treeclocks::{EventTree, IdTree};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Patch {
    inner: Vec<u8>,
}

impl std::fmt::Display for Patch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "[")?;
        for b in self.inner.iter() {
            write!(f, "{b:02X} ")?;
        }
        write!(f, "]")?;

        Ok(())
    }
}

impl Patch {
    pub fn new<T: Serialize>(val: T) -> anyhow::Result<Self> {
        let inner = bincode::serde::encode_to_vec(val, bincode::config::standard())?;
        Ok(Self { inner })
    }

    pub fn downcast<T: for<'de> Deserialize<'de>>(self) -> anyhow::Result<T> {
        let (res, rem) =
            bincode::serde::decode_from_slice(&self.inner, bincode::config::standard())?;
        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PollinationMessage {
    Heartbeat {
        id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
    },
    Update {
        id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
        patch: Patch,
    },
    RealitySkew {
        id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
        patch: Patch,
        peer_count: usize,
    },
    NewMember {},
    Seed {
        id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
        patch: Patch,
        new_id: IdTree,
    },
    SeeOther {
        id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
        patch: Patch,
    },
}

impl std::fmt::Display for PollinationMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        use PollinationMessage::*;
        match self {
            Heartbeat {
                id,
                //topic,
                timestamp,
                reality_token,
            } => {
                write!(f, "HB - {id} - {timestamp} - {reality_token}")
            }
            Update {
                id,
                //topic,
                timestamp,
                reality_token,
                patch,
            } => {
                write!(f, "HB - {id} - {timestamp} - {reality_token} - {patch}")
            }
            RealitySkew {
                id,
                //topic,
                timestamp,
                reality_token,
                peer_count,
                patch,
            } => {
                write!(
                    f,
                    "RS - {id} - {timestamp} - {reality_token} - {peer_count} - {patch}"
                )
            }
            NewMember {} => {
                write!(f, "NM")
            }
            Seed {
                id,
                //topic,
                timestamp,
                reality_token,
                new_id,
                patch,
            } => {
                write!(
                    f,
                    "SE - {id} - {timestamp} - {reality_token} - {new_id} - {patch}"
                )
            }
            SeeOther {
                id,
                //topic,
                timestamp,
                reality_token,
                patch,
            } => {
                write!(f, "SO - {id} - {timestamp} - {reality_token} - {patch}")
            }
        }
    }
}
