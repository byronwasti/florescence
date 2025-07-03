use crate::{ds::WalkieTalkie, engine::EngineMessage};
use axum::{
    Router,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender};
use url::Url;

use super::Engine;

pub struct AxumEngine {
    app: Router,
    socket_addr: SocketAddr,
}

impl AxumEngine {
    fn new(socket_addr: SocketAddr) -> Self {
        let app = Router::new();
        Self { app, socket_addr }
    }
}

impl Engine for AxumEngine {
    type Addr = Url;
    type Error = AxumEngineError;

    async fn run(
        self,
        addr: Self::Addr,
    ) -> Result<WalkieTalkie<EngineMessage<Self::Addr>>, Self::Error> {
        let app = self.app;

        let (wt0, wt1) = WalkieTalkie::pair();

        let (tx, mut rx) = wt1.split();

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(msg) => {
                        let client = reqwest::Client::new();
                        let res = client.post(msg.addr).body(msg.pollination_msg.serialize());
                    }
                    None => {
                        info!("Channel closed")
                    }
                }
            }
        });

        let state = Arc::new(AppState { tx, addr });

        let app = app.route("/", post(handle_message));

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

async fn handle_message() -> impl IntoResponse {
    "Pong!"
}

#[derive(Debug, Error)]
pub enum AxumEngineError {
    #[error("Axum error: {0}")]
    Axum(#[from] axum::Error),

    #[error("StdIO error: {0}")]
    StdIo(#[from] std::io::Error),
}

struct AppState<A> {
    tx: Sender<EngineMessage<A>>,
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
