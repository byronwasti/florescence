use crate::{
    ds::WalkieTalkie,
    message::PollinationMessage,
    serialization::{deserialize, serialize},
};
use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use url::Url;

use super::{DEFAULT_CHANNEL_SIZE, Engine, EngineEvent, EngineRequest};

pub struct AxumEngine {
    socket_addr: SocketAddr,
}

impl AxumEngine {
    fn new(socket_addr: SocketAddr) -> Self {
        Self { socket_addr }
    }
}

impl Engine for AxumEngine {
    type Addr = Url;
    type Error = AxumEngineError;

    async fn run(
        self,
        addr: Self::Addr,
    ) -> Result<WalkieTalkie<EngineRequest<Self::Addr>, EngineEvent>, Self::Error> {
        let (wt0, wt1) = WalkieTalkie::pair();

        let (tx, rx) = wt1.split();

        tokio::spawn(sender_task(rx));

        let state = Arc::new(AppState { tx, addr });

        let app = Router::new()
            .route("/", post(handle_message))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(self.socket_addr).await?;
        tokio::spawn(async move {
            let res = axum::serve(listener, app).await;
            if let Err(err) = res {
                error!("Error running Axum: {err:?}");
            }
        });

        Ok(wt0)
    }
}

async fn handle_message(State(state): State<Arc<AppState>>, bytes: Bytes) -> impl IntoResponse {
    match handle_message_inner(&state.tx, bytes).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, Bytes::new()),
    }
}

async fn handle_message_inner(
    tx: &Sender<EngineEvent>,
    bytes: Bytes,
) -> Result<Bytes, AxumEngineError> {
    let pollination_msg: PollinationMessage = deserialize(bytes.to_vec())?;
    let (res_tx, mut rx) = channel(DEFAULT_CHANNEL_SIZE);
    tx.send(EngineEvent {
        pollination_msg,
        tx: res_tx,
    });

    if let Some(res) = rx.recv().await {
        Ok(serialize(res)?.into())
    } else {
        Ok(Bytes::new())
    }
}

async fn sender_task(mut rx: Receiver<EngineRequest<Url>>) {
    loop {
        match rx.recv().await {
            Some(req) => {
                let EngineRequest {
                    pollination_msg,
                    addr,
                    tx,
                } = req;

                match send_and_recv(addr, pollination_msg).await {
                    Ok(res) => {
                        tx.send(res);
                    }
                    Err(err) => {
                        error!("Error sending request: {err}");
                    }
                }
            }
            None => {
                info!("Channel closed")
            }
        }
    }
}

async fn send_and_recv(
    addr: Url,
    pollination_msg: PollinationMessage,
) -> Result<PollinationMessage, AxumEngineError> {
    let client = reqwest::Client::new();
    let res = client
        .post(addr)
        .body(serialize(pollination_msg)?)
        .send()
        .await?;

    let bytes = res.bytes().await?.to_vec();
    let pollination_msg = crate::serialization::deserialize(bytes)?;

    Ok(pollination_msg)
}

#[derive(Debug, Error)]
pub enum AxumEngineError {
    #[error("Axum error: {0}")]
    Axum(#[from] axum::Error),

    #[error("StdIO error: {0}")]
    StdIo(#[from] std::io::Error),

    #[error("Deserialize error: {0}")]
    Deserialize(#[from] crate::serialization::DeserializeError),

    #[error("Serialize error: {0}")]
    Serialize(#[from] crate::serialization::SerializeError),

    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

struct AppState {
    tx: Sender<EngineEvent>,
    addr: Url,
}

#[derive(Serialize, Deserialize)]
struct AxumMessage<A> {
    return_addr: A,
    pollination_msg: PollinationMessage,
}

impl<A> AxumMessage<A> {
    fn new(return_addr: A, pollination_msg: PollinationMessage) -> Self {
        Self {
            return_addr,
            pollination_msg,
        }
    }
}
