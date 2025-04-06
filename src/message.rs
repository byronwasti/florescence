use crate::pollinator::Patch;
use crate::reality_token::RealityToken;
use serde::{Deserialize, Serialize};
use treeclocks::{EventTree, IdTree};

/*
#[derive(Serialize, Deserialize)]
pub struct PollinationMessage<I, A> {
    id: I,
    //addr: A,
    //topic: String,
    timestamp: EventTree,
    reality_token: RealityToken,
    kind: PollinationMessageKind<I, A>,
}
*/

#[derive(Serialize, Deserialize)]
pub enum PollinationMessage<I> {
    Heartbeat {
        id: I,
        itc_id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
    },
    Update {
        id: I,
        itc_id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
        patch: Patch,
    },
    RealitySkew {
        id: I,
        itc_id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
        patch: Patch,
        peer_count: usize,
    },
    NewMember {
        id: I,
    },
    Seed {
        id: I,
        itc_id: IdTree,
        //topic: String,
        timestamp: EventTree,
        reality_token: RealityToken,
        patch: Patch,
        new_itc_id: IdTree,
    },
}
