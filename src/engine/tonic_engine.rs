use std::pin::Pin;
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming, transport::Server};
use tokio::sync::mpsc::{self, UnboundedSender, UnboundedReceiver};
use crate::message::PollinationMessage;
use uuid::Uuid;
use std::net::SocketAddr;
use tracing::debug;
use serde::{Serialize, Deserialize};
use std::str::FromStr;

mod codec;
mod rpc;

use super::*;
use rpc::{
    TonicReqWrapper,
    gossip_server::{Gossip, GossipServer},
    gossip_client::{GossipClient},
};

// The http crate doesn't support `serde` via a FF, so have to
// do this workaround.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Uri {
    #[serde(with="http_serde::uri")]
    uri: http::Uri,
}

impl Uri {
    pub fn new(uri: http::Uri) -> Self {
        Self {
            uri,
        }
    }
}

impl FromStr for Uri {
    type Err = <http::Uri as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uri = s.parse()?;
        Ok(Self::new(uri))
    }
}

/// Streaming RPC via Tonic library
pub struct TonicEngine<T, I>{
    socket_addr: SocketAddr,
    addr: Uri,
    new_conn_tx: Option<UnboundedSender<Connection<T, I, Uri>>>,
    new_conn_rx: UnboundedReceiver<Connection<T, I, Uri>>,
}

impl<T, I> TonicEngine<T, I> {
    /// `socket_addr` is the connection on the server side
    /// `addr` is the address clients use
    fn new(socket_addr: SocketAddr, addr: http::Uri) -> Self {
        let (new_conn_tx, new_conn_rx) = mpsc::unbounded_channel();
        Self {
            socket_addr,
            addr: Uri { uri: addr },
            new_conn_tx: Some(new_conn_tx),
            new_conn_rx,
        }
    }
}

impl<T, I> Engine<T, I> for TonicEngine<T, I>
where T: Send + Sync + 'static + Serialize + for<'a> Deserialize<'a>,
      I: Send + Sync + 'static + Serialize + for<'a> Deserialize<'a>,
{
    type Addr = Uri;

    fn addr(&self) -> &Self::Addr {
        &self.addr
    }

    fn create_conn(&mut self, addr: Uri) -> Connection<T, I, Self::Addr> {
        let (tx0, rx0) = mpsc::unbounded_channel();
        let (tx1, rx1) = mpsc::unbounded_channel();
        let addr_clone = addr.clone();
        tokio::task::spawn(async move {
            // TODO: Deal with failure
            let mut client = GossipClient::connect(addr.clone().uri).await.unwrap();
            
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
        });

        Connection {
            addr: addr_clone,
            tx: tx0,
            rx:rx1,
        }
    }

    fn get_new_conns(&mut self) -> Vec<Connection<T, I, Self::Addr>> {
        todo!()
    }

    fn start(&mut self)
    {
        let gossiper = Handler::new(self.new_conn_tx.take().expect("start called twice."));

        let socket_addr = self.socket_addr.clone();
        tokio::task::spawn(async move {
            Server::builder()
                .add_service(GossipServer::new(gossiper))
                .serve(socket_addr)
                .await
                .expect("TonicRPC internal failure.")
        });
    }
}

struct Handler<T, I, A> {
    tx: UnboundedSender<Connection<T, I, A>>,
}

impl<T, I, A> Handler<T, I, A> {
    pub fn new(tx: UnboundedSender<Connection<T, I, A>>) -> Self {
        Self {
            tx
        }
    }
}

type ResponseStream = Pin<Box<dyn Stream<Item = Result<TonicReqWrapper, Status>> + Send>>;

#[tonic::async_trait]
impl<T, I, A> Gossip for Handler<T, I, A>
where T: Send + Sync + 'static,
      I: Send + Sync + 'static,
      A: Send + Sync + 'static,
{
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

