use crate::pollinator::{RealityToken, UpdatePacket};
use serde::{Deserialize, Serialize};
use treeclocks::{EventTree, IdTree};

#[derive(Serialize, Deserialize)]
pub struct PollinationMessage<I, A> {
    id: I,
    addr: A,
    topic: String,
    timestamp: EventTree,
    reality_token: RealityToken,
    kind: PollinationMessageKind<I, A>,
}

#[derive(Serialize, Deserialize)]
enum PollinationMessageKind<I, A> {
    Heartbeat,
    RequestFork,
    Update(UpdatePacket),
    RealitySkew {
        ids: Vec<(I, A)>,
    },
    Seed {
        itc_id: IdTree,
        update: UpdatePacket,
    },
}
