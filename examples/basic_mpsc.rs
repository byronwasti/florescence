use anyhow::Result;
use florescence::{
    Flower,
    engine::{
        Engine,
        mpsc::{MpscEngine, World, new_world},
    },
    message::PollinationMessage,
};
use tokio::task::JoinSet;
use tracing::{Instrument, info, instrument};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    FmtSubscriber::builder()
        .with_env_filter("basic=debug,florescence=debug,treeclocks=trace")
        .with_line_number(true)
        .init();

    let mut set = JoinSet::new();
    let world = new_world();
    for i in 0..2 {
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
    info!("Starting node {i}");
    let seed_list: Vec<usize> = (0..i).into_iter().collect();
    let flower = Flower::builder()
        .engine(MpscEngine::new(world))
        .seed(&seed_list[..])
        .bloom()?;

    //let p0 = flower.stream_pollinator::<IdentityMap<u32>>();

    flower.runtime().await?;

    Ok(())
}
