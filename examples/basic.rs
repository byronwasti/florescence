use anyhow::Result;
use clap::Parser;
use florescence::{Flower, engine::TonicEngine};
use http::Uri;
use std::net::SocketAddr;
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    port: u16,

    #[arg(long, short = 'n')]
    peers: Vec<u16>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    FmtSubscriber::builder()
        .with_env_filter("florescence=debug,treeclocks=trace")
        .with_line_number(true)
        //.with_max_level(tracing::Level::DEBUG)
        .init();

    let id = Uuid::new_v4();
    let socket_addr: SocketAddr = format!("0.0.0.0:{}", args.port).parse()?;
    let uri: Uri = format!("http://0.0.0.0:{}", args.port).parse()?;
    let seed_list: Vec<_> = args
        .peers
        .iter()
        .map(|x| format!("http://0.0.0.0:{x}").parse())
        .collect::<Result<Vec<_>, _>>()?;
    let flower = Flower::builder()
        .engine(TonicEngine::new(socket_addr, uri))
        .seed(&seed_list[..])
        /*
        .seed(&[
            "http://0.0.0.0:8001".parse()?,
            "http://0.0.0.0:8002".parse()?,
        ])
        */
        .bloom()?;

    //let p0 = flower.stream_pollinator::<IdentityMap<u32>>();

    flower.runtime().await?;
    Ok(())
}
