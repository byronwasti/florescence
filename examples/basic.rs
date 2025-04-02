use anyhow::Result;
use clap::Parser;
use florescence::{Flower, engine::TonicEngine, pollinator::IdentityMap};
use http::Uri;
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let id = Uuid::new_v4();
    let socket_addr: SocketAddr = format!("0.0.0.0:{}", args.port).parse()?;
    let uri: Uri = format!("http://0.0.0.0:{}", args.port).parse()?;
    let flower = Flower::builder()
        .id(id)
        .engine(TonicEngine::new(socket_addr, uri))
        .seed(&[
            "http://0.0.0.0:8001".parse()?,
            "http://0.0.0.0:8002".parse()?,
        ])
        .bloom()?;

    //let p0 = flower.stream_pollinator::<IdentityMap<u32>>();

    flower.runtime().await;
    Ok(())
}
