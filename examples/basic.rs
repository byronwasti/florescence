use florescence::{Flower, engine::TonicEngine, pollinator::IdentityMap};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let id = "some-id";
    let addr = SocketAddr::parse("0.0.0.0:8001");
    let flower = Flower::builder(id).engine(TonicEngine::new(addr)).bloom()?;

    let p0 = flower.stream_pollinator::<IdentityMap<u32>>();

    flower.runtime().await
}
