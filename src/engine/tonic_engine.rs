use crate::message::PollinationMessage;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::pin::Pin;
use std::str::FromStr;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender, error::TryRecvError};
use tokio_stream::{Stream, StreamExt, wrappers::UnboundedReceiverStream};
use tonic::{Request, Response, Status, Streaming, transport::Server};
use tracing::{debug, error};

mod codec;
mod rpc;

use super::*;
use rpc::{
    TonicReqWrapper,
    gossip_client::GossipClient,
    gossip_server::{Gossip, GossipServer},
};

// The http crate doesn't support `serde` via a FF, so have to
// do this workaround.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Uri {
    #[serde(with = "http_serde::uri")]
    uri: http::Uri,
}

impl Uri {
    pub fn new(uri: http::Uri) -> Self {
        Self { uri }
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
pub struct TonicEngine<I> {
    socket_addr: SocketAddr,
    addr: Uri,
    new_conn_tx: Option<UnboundedSender<Connection<I, Uri>>>,
    new_conn_rx: UnboundedReceiver<Connection<I, Uri>>,
}

impl<I> TonicEngine<I> {
    /// `socket_addr` is the connection on the server side
    /// `addr` is the address clients use
    pub fn new(socket_addr: SocketAddr, addr: http::Uri) -> Self {
        let (new_conn_tx, new_conn_rx) = mpsc::unbounded_channel();
        Self {
            socket_addr,
            addr: Uri { uri: addr },
            new_conn_tx: Some(new_conn_tx),
            new_conn_rx,
        }
    }
}

impl<I> Engine<I> for TonicEngine<I>
where
    I: Send + Sync + 'static + Serialize + for<'a> Deserialize<'a>,
{
    type Addr = Uri;

    fn addr(&self) -> &Self::Addr {
        &self.addr
    }

    fn create_conn(&mut self, addr: Uri) -> Connection<I, Self::Addr> {
        let (tx0, rx0) = mpsc::unbounded_channel();
        let (tx1, rx1) = mpsc::unbounded_channel();
        tokio::task::spawn(async move {
            // TODO: Deal with failure
            let mut client = GossipClient::connect(addr.clone().uri).await.unwrap();

            let in_stream = UnboundedReceiverStream::new(rx0).map(|x: PollinationMessage<_, _>| {
                TonicReqWrapper {
                    raw: bincode::serde::encode_to_vec(x, bincode::config::standard())
                        .expect("Unable to serialize message"),
                }
            });
            // TODO: Deal with failure
            let res = client.gossip(in_stream).await.unwrap();

            let mut out_stream = res.into_inner();

            loop {
                match out_stream.next().await {
                    Some(Ok(val)) => {
                        if let Ok((val, _)) =
                            bincode::serde::decode_from_slice(&val.raw, bincode::config::standard())
                        {
                            if let Err(err) = tx1.send(val) {
                                debug!("Internal mpsc errored: {err}");
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    Some(Err(err)) => {
                        debug!("Receiving stream errored: {err}");
                        break;
                    }
                    None => {
                        debug!("Receiving stream empty.");
                        break;
                    }
                }
            }
        });

        Connection { tx: tx0, rx: rx1 }
    }

    fn get_new_conns(&mut self) -> Vec<Connection<I, Self::Addr>> {
        let mut new_conns = vec![];
        loop {
            match self.new_conn_rx.try_recv() {
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => panic!("New connection tx closed."),
                Ok(val) => new_conns.push(val),
            }
        }
        new_conns
    }

    fn start(&mut self) {
        let gossiper = Handler::new(self.new_conn_tx.take().expect("start called twice."));

        let socket_addr = self.socket_addr;
        tokio::task::spawn(async move {
            Server::builder()
                .add_service(GossipServer::new(gossiper))
                .serve(socket_addr)
                .await
                .expect("TonicRPC internal failure.")
        });
    }
}

struct Handler<I, A> {
    tx: UnboundedSender<Connection<I, A>>,
}

impl<I, A> Handler<I, A> {
    pub fn new(tx: UnboundedSender<Connection<I, A>>) -> Self {
        Self { tx }
    }
}

type ResponseStream = Pin<Box<dyn Stream<Item = Result<TonicReqWrapper, Status>> + Send>>;

#[tonic::async_trait]
impl<I, A> Gossip for Handler<I, A>
where
    I: Send + Sync + 'static + Serialize + for<'a> Deserialize<'a>,
    A: Send + Sync + 'static + Serialize + for<'a> Deserialize<'a>,
{
    type GossipStream = ResponseStream;
    async fn gossip(
        &self,
        request: Request<Streaming<TonicReqWrapper>>,
    ) -> Result<Response<ResponseStream>, Status> {
        // TODO: This must be coordinated with the EngineCore
        let (tx0, rx0) = mpsc::unbounded_channel();
        let (tx1, rx1) = mpsc::unbounded_channel();

        if let Err(err) = self.tx.send(Connection::new(tx0, rx1)) {
            error!("New connection rx is closed: {err}");
            panic!("New connection rx is closed");
        }

        let mut in_stream = request.into_inner();

        tokio::spawn(async move {
            loop {
                let res = in_stream.message().await;
                match res {
                    Ok(None) => {
                        // Stream is closed by peer
                        todo!()
                    }
                    Ok(Some(val)) => {
                        if let Ok((val, _)) =
                            bincode::serde::decode_from_slice(&val.raw, bincode::config::standard())
                        {
                            if let Err(err) = tx1.send(val) {
                                debug!("Internal mpsc errored: {err}");
                                break;
                            }
                        } else {
                            // TODO: Log error
                            error!("Unable to deserialize the request");
                        }
                    }
                    Err(_err) => {
                        todo!()
                    }
                }
            }
        });

        let out_stream = UnboundedReceiverStream::new(rx0).map(|x: PollinationMessage<_, _>| {
            Ok(TonicReqWrapper {
                raw: bincode::serde::encode_to_vec(x, bincode::config::standard())
                    .expect("Unable to serialize message"),
            })
        });
        Ok(Response::new(Box::pin(out_stream) as Self::GossipStream))
    }
}
