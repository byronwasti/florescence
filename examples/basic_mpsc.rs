use anyhow::Result;
use florescence::{
    Flower, FlowerHandle,
    engine::{
        Engine,
        mpsc::{MpscEngine, World, new_world},
    },
    message::PollinationMessage,
};
use tokio::task::JoinSet;
use tracing::{Instrument, info, instrument};
use tracing_subscriber::FmtSubscriber;

const N: usize = 3;

#[tokio::main]
async fn main() -> Result<()> {
    FmtSubscriber::builder()
        //.json()
        .with_env_filter("basic=debug,florescence=debug,treeclocks=trace")
        .with_line_number(true)
        .with_ansi(false)
        .init();

    let mut set = JoinSet::new();
    let world = new_world();
    for i in 0..N {
        set.spawn(spawn_node(i, world.clone()).in_current_span());
    }

    while let Some(res) = set.join_next().await {
        info!("Node died: {res:?}");
    }

    Ok(())
}

#[instrument(name = "node", skip(world))]
async fn spawn_node<T>(i: usize, world: World<T>) -> Result<()>
where
    MpscEngine<T>: Engine<PollinationMessage, Addr = usize>,
    T: Send + 'static,
{
    let seed_list: Vec<_> = (0..3).map(|_| rand::random_range(0..N)).collect();
    info!("Starting node {i}");
    let flower = Flower::builder()
        .engine(MpscEngine::new(world))
        .seed(&seed_list[..])
        .bloom()
        .await?;

    //let p0 = flower.stream_pollinator::<IdentityMap<u32>>();

    flower.runtime().await?;

    Ok(())
}
