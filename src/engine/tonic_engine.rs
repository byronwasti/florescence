use crate::constants::MPSC_CHANNEL_SIZE;
use crate::message::PollinationMessage;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;
use std::pin::Pin;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream};
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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct Uri {
    #[serde(with = "http_serde::uri")]
    uri: http::Uri,
}

impl Uri {
    pub fn new(uri: http::Uri) -> Self {
        Self { uri }
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self.uri)
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
pub struct TonicEngine {
    socket_addr: SocketAddr,
    addr: Uri,
    new_conn_tx: Option<Sender<Connection>>,
    new_conn_rx: Receiver<Connection>,
}

impl TonicEngine {
    /// `socket_addr` is the connection on the server side
    /// `addr` is the address clients use
    pub fn new(socket_addr: SocketAddr, addr: http::Uri) -> Self {
        let (new_conn_tx, new_conn_rx) = mpsc::channel(MPSC_CHANNEL_SIZE);
        Self {
            socket_addr,
            addr: Uri { uri: addr },
            new_conn_tx: Some(new_conn_tx),
            new_conn_rx,
        }
    }
}

impl Engine for TonicEngine {
    type Addr = Uri;

    fn addr(&self) -> &Self::Addr {
        &self.addr
    }

    fn create_conn(&mut self, addr: Uri) -> Connection {
        let (tx0, rx0) = mpsc::channel(MPSC_CHANNEL_SIZE);
        let (tx1, rx1) = mpsc::channel(MPSC_CHANNEL_SIZE);
        tokio::task::spawn(async move {
            // TODO: Deal with failure
            let mut client = loop {
                if let Ok(client) = GossipClient::connect(addr.clone().uri).await {
                    break client;
                } else {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            };

            let in_stream = ReceiverStream::new(rx0).map(|x: PollinationMessage| TonicReqWrapper {
                raw: bincode::serde::encode_to_vec(x, bincode::config::standard())
                    .expect("Unable to serialize message"),
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
                            if let Err(err) = tx1.try_send(val) {
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

        Connection::new(tx0, rx1)
    }

    async fn get_new_conn(&mut self) -> Option<Connection> {
        self.new_conn_rx.recv().await
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

struct Handler {
    tx: Sender<Connection>,
}

impl Handler {
    pub fn new(tx: Sender<Connection>) -> Self {
        Self { tx }
    }
}

type ResponseStream = Pin<Box<dyn Stream<Item = Result<TonicReqWrapper, Status>> + Send>>;

#[tonic::async_trait]
impl Gossip for Handler {
    type GossipStream = ResponseStream;
    async fn gossip(
        &self,
        request: Request<Streaming<TonicReqWrapper>>,
    ) -> Result<Response<ResponseStream>, Status> {
        // TODO: This must be coordinated with the EngineCore
        let (tx0, rx0) = mpsc::channel(MPSC_CHANNEL_SIZE);
        let (tx1, rx1) = mpsc::channel(MPSC_CHANNEL_SIZE);

        if let Err(err) = self.tx.send(Connection::new(tx0, rx1)).await {
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
                            if let Err(err) = tx1.send(val).await {
                                debug!("Internal mpsc errored: {err}");
                                break;
                            }
                        } else {
                            // TODO: Log error
                            error!("Unable to deserialize the request");
                        }
                    }
                    Err(err) => {
                        error!("gRPC Status: {err}");
                    }
                }
            }
        });

        let out_stream = ReceiverStream::new(rx0).map(|x: PollinationMessage| {
            Ok(TonicReqWrapper {
                raw: bincode::serde::encode_to_vec(x, bincode::config::standard())
                    .expect("Unable to serialize message"),
            })
        });
        Ok(Response::new(Box::pin(out_stream) as Self::GossipStream))
    }
}
