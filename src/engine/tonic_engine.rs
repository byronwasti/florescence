use std::pin::Pin;
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming, transport::Server};
use tokio::sync::mpsc::{self, UnboundedSender, UnboundedReceiver};
use crate::message::PollinationMessage;
use uuid::Uuid;
use http::Uri;
use std::net::SocketAddr;
use tracing::debug;
use serde::{Serialize, Deserialize};

mod codec;
mod rpc;

use super::*;
use rpc::{
    TonicReqWrapper,
    gossip_server::{Gossip, GossipServer},
    gossip_client::{GossipClient},
};


/// Streaming RPC via Tonic library
pub struct TonicEngine{
    socket_addr: SocketAddr,
    addr: Uri,
    tx: Option<UnboundedSender<EngMessage<Uri>>>,
}

impl TonicEngine {
    /// `socket_addr` is the connection on the server side
    /// `addr` is the address clients use
    fn new(socket_addr: SocketAddr, addr: Uri) -> Self {
        Self {
            socket_addr,
            addr,
            tx: None,
        }
    }
}

impl Engine for TonicEngine
{
    type Addr = Uri;

    fn addr(&self) -> &Self::Addr {
        &self.addr
    }

    fn remove_conn(&mut self, addr: Self::Addr) -> impl std::future::Future<Output=()> + Send {
        async {}
    }

    fn new_conn<T, I, A>(&mut self, addr: Uri) -> impl std::future::Future<Output=(UnboundedSender<PollinationMessage<T, I, A>>, UnboundedReceiver<PollinationMessage<T, I, A>>)> + Send
    where T: for<'a> Deserialize<'a> + Serialize + Send + Sync + 'static,
          I: for<'a> Deserialize<'a> + Serialize + Send + Sync + 'static,
          A: for<'a> Deserialize<'a> + Serialize + Send + Sync + 'static,
    {
        let (tx0, rx0) = mpsc::unbounded_channel();
        let (tx1, rx1) = mpsc::unbounded_channel();
        tokio::task::spawn(async move {
            // TODO: Deal with failure
            let mut client = GossipClient::connect(addr.clone()).await.unwrap();
            
            let in_stream = UnboundedReceiverStream::new(rx0)
                .map(|x: PollinationMessage<_, _, _>| {
                    TonicReqWrapper {
                        raw: bincode::serde::encode_to_vec(x, bincode::config::standard()).expect("Unable to serialize message"),
                    }
                });
            // TODO: Deal with failure
            let mut res = client.gossip(in_stream)
                .await
                .unwrap();

            let mut out_stream = res.into_inner();

            loop {
                match out_stream.next().await {
                    Some(Ok(val)) => {
                        if let Ok((val, _)) = bincode::serde::decode_from_slice(&val.raw, bincode::config::standard()) {
                            if let Err(err) = tx1.send(val) {
                                debug!("Internal mpsc errored: {err}");
                                break
                            }
                        } else {
                            break
                        }
                    }
                    Some(Err(err)) => {
                        debug!("Receiving stream errored: {err}");
                        break
                    }
                    None => {
                        debug!("Receiving stream empty.");
                        break
                    }

                }
            }
            /*
            while let Some(rec) = out_stream.next().await {
                if let Err(_) = tx1.send(Some(rec)) {
                    break
                }
            }
            */

            //tx1.send(EngMessage::Terminated(addr))
        });

        std::future::ready((tx0, rx1))
    }

    fn start(&mut self, tx: UnboundedSender<EngMessage<Uri>>) -> impl std::future::Future<Output=()> + Send
    {
        self.tx = Some(tx.clone());
        run(self.socket_addr.clone(), tx)
    }
}

async fn run(socket_addr: SocketAddr, tx: UnboundedSender<EngMessage<Uri>>) {
    let gossiper = Handler::new(tx);

    tokio::task::spawn(async move {
        Server::builder()
            .add_service(GossipServer::new(gossiper))
            .serve(socket_addr)
            .await
            .expect("TonicRPC internal failure.")
    });
}

struct Handler {
    tx: UnboundedSender<EngMessage<Uri>>,
}

impl Handler {
    pub fn new(tx: UnboundedSender<EngMessage<Uri>>) -> Self {
        Self {
            tx
        }
    }
}

type ResponseStream = Pin<Box<dyn Stream<Item = Result<TonicReqWrapper, Status>> + Send>>;

#[tonic::async_trait]
impl Gossip for Handler {
    type GossipStream = ResponseStream;
    async fn gossip(
        &self,
        mut request: Request<Streaming<TonicReqWrapper>>,
    ) -> Result<Response<ResponseStream>, Status> {

        // TODO: This must be coordinated with the EngineCore
        let (tx0, rx0) = mpsc::unbounded_channel();
        let (tx1, rx1) = mpsc::unbounded_channel();

        let mut in_stream = request.into_inner();

        tokio::spawn(async move {
            while let res = in_stream.message().await {
                match res {
                    Ok(None) => {
                        // Stream is closed by peer
                        todo!()
                    }
                    Ok(v) => {
                        tx1.send(v);
                    }
                    Err(err) => {
                    }
                }
            }
        });

        let out_stream = UnboundedReceiverStream::new(rx0);
        Ok(Response::new(
                Box::pin(out_stream) as Self::GossipStream
        ))
    }
}

