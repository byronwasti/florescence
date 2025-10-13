use anyhow::Result;
use clap::Parser;
use florescence::{Flower, engine::axum::AxumEngine};
use tracing::info;
use tracing_subscriber::FmtSubscriber;
use url::Url;

#[derive(Parser, Debug)]
struct Args {}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    FmtSubscriber::builder()
        .with_env_filter("basic_axum=debug,florescence=debug,treeclocks=trace")
        .with_line_number(true)
        //.with_max_level(tracing::Level::DEBUG)
        .init();

    let mut seed_list = vec![];
    for port in 8000..8003 {
        let socket_addr = format!("0.0.0.0:{port}");
        let socket_addr = socket_addr.parse()?;
        let url = format!("http://0.0.0.0:{port}");
        let url: Url = url.parse()?;

        let flower = Flower::builder()
            .engine(AxumEngine::new(socket_addr))
            .own_addr(url.clone())
            .seed_list(seed_list.clone())
            .start();

        info!("Flower started at {url}");
        seed_list.push(url);
    }

    Ok(())
}
