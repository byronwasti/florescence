use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{transport::Server, Request, Response, Status, Streaming};

use crate::gossip::{
    gossip_server::{Gossip, GossipServer},
    GossipRequest, GossipResponse,
};

#[derive(Default)]
struct MyGossiper {}

type ResponseStream = Pin<Box<dyn Stream<Item = Result<GossipResponse, Status>> + Send>>;

#[tonic::async_trait]
impl Gossip for MyGossiper {
    type GossipStream = ResponseStream;
    async fn gossip(
        &self,
        _request: Request<Streaming<GossipRequest>>,
    ) -> Result<Response<ResponseStream>, Status> {
        todo!()
    }
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse().unwrap();
    let gossiper = MyGossiper::default();

    Server::builder()
        .add_service(GossipServer::new(gossiper))
        .serve(addr)
        .await?;

    Ok(())
}
