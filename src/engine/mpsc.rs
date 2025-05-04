use crate::{ds::WalkieTalkie, engine::Engine};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{Receiver, Sender, channel};

pub struct MpscEngine<T> {
    inner: Arc<RwLock<Inner<T>>>,
    new_conn_rx: Receiver<WalkieTalkie<T, T>>,
    addr: usize,
}

impl<T> MpscEngine<T> {
    pub fn new(size: usize) -> Self {
        let (_, rx) = channel(1);
        Self {
            inner: Arc::new(RwLock::new(Inner::new(size))),
            new_conn_rx: rx,
            addr: 0,
        }
    }

    pub fn with_addr(&mut self, addr: usize) -> Self {
        let mut inner = self.inner.write().expect("poisoned lock");
        let new_conn_rx = inner.conns[addr].1.take().expect("addr reused");
        info!("New at {addr}");
        Self {
            inner: self.inner.clone(),
            new_conn_rx,
            addr,
        }
    }
}

impl<T> Engine<T> for MpscEngine<T>
where
    T: Serialize + for<'a> Deserialize<'a> + Clone + Send,
{
    type Addr = usize;

    async fn start(&mut self) {
        // NOP
    }

    fn addr(&self) -> &Self::Addr {
        &self.addr
    }

    async fn create_conn(&mut self, addr: usize) -> (Sender<T>, Receiver<T>) {
        let (tx, w0, w1) = {
            let inner = self.inner.read().expect("poisoned lock");
            let tx = inner.conns[addr].0.clone();
            let (w0, w1) = WalkieTalkie::pair_with_buffer(inner.conns.len());
            (tx, w0, w1)
        };

        tx.send(w0).await.expect("Channel closed");

        w1.split()
    }

    async fn get_new_conn(&mut self) -> Option<(Sender<T>, Receiver<T>)> {
        let w = self.new_conn_rx.recv().await?;
        Some(w.split())
    }
}

struct Inner<T> {
    #[allow(clippy::type_complexity)]
    conns: Vec<(
        Sender<WalkieTalkie<T, T>>,
        Option<Receiver<WalkieTalkie<T, T>>>,
    )>,
}

impl<T> Inner<T> {
    fn new(size: usize) -> Self {
        let conns = (0..size)
            .map(|_| {
                let (tx, rx) = channel(10);
                (tx, Some(rx))
            })
            .collect();

        Self { conns }
    }
}
