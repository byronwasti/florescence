use crate::pollinator::{RealityToken, UpdatePacket};
use serde::{Deserialize, Serialize};
use treeclocks::{EventTree, IdTree};

#[derive(Serialize, Deserialize)]
pub struct PollinationMessage<T, I, A> {
    id: I,
    addr: A,
    topic: String,
    timestamp: EventTree,
    reality_token: RealityToken,
    kind: PollinationMessageKind<T, I, A>,
}

#[derive(Serialize, Deserialize)]
enum PollinationMessageKind<T, I, A> {
    Heartbeat,
    RequestFork,
    Update(UpdatePacket<T>),
    RealitySkew {
        ids: Vec<(I, A)>,
    },
    Seed {
        itc_id: IdTree,
        update: UpdatePacket<T>,
    },
}
