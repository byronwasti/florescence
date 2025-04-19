use crate::message::PollinationMessage;
use tokio::sync::mpsc::{Receiver, Sender};

//pub type Connection = (Sender<PollinationMessage>, Receiver<PollinationMessage>);

pub struct Connection {
    prev_msg: Option<PollinationMessage>,
    tx: Sender<PollinationMessage>,
    rx: Option<Receiver<PollinationMessage>>,
}

impl Connection {
    pub fn new(tx: Sender<PollinationMessage>, rx: Receiver<PollinationMessage>) -> Self {
        Self {
            prev_msg: None,
            tx,
            rx: Some(rx),
        }
    }

    pub fn take_rx(&mut self) -> Option<Receiver<PollinationMessage>> {
        self.rx.take()
    }

    pub fn tx(&self) -> &Sender<PollinationMessage> {
        &self.tx
    }
}
