use crate::constants::MPSC_CHANNEL_SIZE;
use crate::engine::Engine;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{self, Receiver, Sender};

type FloraConn<T> = (Sender<T>, Receiver<T>);
type EngineConn<T> = Sender<FloraConn<T>>;

pub type World<T> = Arc<RwLock<Vec<EngineConn<T>>>>;

pub fn new_world<T>() -> Arc<RwLock<Vec<EngineConn<T>>>> {
    Arc::new(RwLock::new(vec![]))
}

pub struct MpscEngine<T> {
    world_view: World<T>,
    own_idx: usize,
    receiver: Receiver<FloraConn<T>>,
}

impl<T> MpscEngine<T> {
    pub fn new(world_view: World<T>) -> Self {
        let (own_idx, rx) = {
            let mut world = world_view.write().expect("poisoned lock");
            let (tx, rx) = mpsc::channel(MPSC_CHANNEL_SIZE);
            world.push(tx);
            (world.len() - 1, rx)
        };

        Self {
            own_idx,
            world_view,
            receiver: rx,
        }
    }
}

impl<T> Engine<T> for MpscEngine<T>
where
    T: Serialize + for<'a> Deserialize<'a> + Clone + Send,
{
    type Addr = usize;

    fn start(&mut self) {
        // Nothing to do.
    }

    fn addr(&self) -> &Self::Addr {
        &self.own_idx
    }

    async fn create_conn(&mut self, addr: usize) -> (Sender<T>, Receiver<T>) {
        let peer_tx = {
            let world = self.world_view.read().expect("poisoned lock");

            world[addr].clone()
        };

        let (tx0, rx0) = mpsc::channel(MPSC_CHANNEL_SIZE);
        let (tx1, rx1) = mpsc::channel(MPSC_CHANNEL_SIZE);

        peer_tx.send((tx0, rx1)).await;

        (tx1, rx0)
    }

    async fn get_new_conn(&mut self) -> Option<(Sender<T>, Receiver<T>)> {
        self.receiver.recv().await
    }
}
