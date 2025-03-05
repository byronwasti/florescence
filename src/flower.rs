use treeclocks::{itc_map::UpdatePacket, EventTree, ItcMap};
use std::collections::HashMap;

struct RealityToken(u64);

struct FlowerCore<T, I, A> {
    id: I,
    addr: A,
    timestamp: EventTree,
    reality_token: RealityToken,
    data: ItcMap<T>,
    decay_list: Vec<I>,
    peer_info: HashMap<I, PeerInfo<I, A>>,
}

struct PeerInfo<I, A> {
    id: I,
    addr: A,
}

struct Message<T, I, A> {
    id: I,
    addr: A,
    timestamp: EventTree,
    reality_token: RealityToken,
    kind: MessageKind<T, I, A>,
}

enum MessageKind<T, I, A> {
    Heartbeat,
    Update(UpdatePacket<T>),
    RealitySkew { ids: Vec<(I, A)> },
}
