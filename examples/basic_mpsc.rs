use anyhow::Result;
use florescence::{
    Flower, FlowerHandle,
    engine::{Engine, mpsc::MpscEngine},
    message::PollinationMessage,
};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use tokio::{task::JoinSet, time::interval};
use tracing::{Instrument, info, instrument};
use tracing_subscriber::FmtSubscriber;

const N: usize = 5;

#[tokio::main]
async fn main() -> Result<()> {
    FmtSubscriber::builder()
        //.json()
        .with_env_filter("basic=debug,florescence=info,treeclocks=trace")
        .with_line_number(true)
        .with_ansi(false)
        .init();

    let mut handles = vec![];
    let mut engine = MpscEngine::new(N);
    for i in 0..N {
        let handle = spawn_node(i, engine.with_addr(i)).await?;
        handles.push(handle);
    }

    let start = Instant::now();
    let mut interval = interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        println!("\n\n========== {:?} =========", start.elapsed());

        for (i, h) in handles.iter_mut().enumerate() {
            if let Some(n) = h.data().await {
                println!("{i} => {}\n", n.beautiful());
            } else {
                info!("Node died.");
            }
        }
    }

    Ok(())
}

#[instrument(name = "node", skip(engine))]
async fn spawn_node<T>(i: usize, engine: MpscEngine<T>) -> Result<FlowerHandle<usize>>
where
    MpscEngine<T>: Engine<PollinationMessage, Addr = usize>,
    T: Send + 'static,
{
    let mut seed_list: HashSet<_> = (0..3)
        .map(|_| rand::random_range(0..N))
        .filter(|x| *x != i)
        .collect();
    let seed_list: Vec<_> = seed_list.drain().collect();
    info!("Starting node {i} with {seed_list:?}");
    let flower = Flower::builder()
        .engine(engine)
        .seed(&seed_list[..])
        .bloom()
        .await?;

    //let p0 = flower.stream_pollinator::<IdentityMap<u32>>();

    Ok(flower)
}
