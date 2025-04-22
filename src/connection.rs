use crate::constants;
use crate::message::PollinationMessage;
use std::time::Instant;
use tokio::sync::mpsc::{Receiver, Sender, error::SendError};
use tracing::debug;

#[derive(Debug)]
pub struct Connection {
    pub(crate) prev_msg: Option<(PollinationMessage, Instant)>,
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

    pub async fn send(
        &mut self,
        msg: PollinationMessage,
    ) -> Result<(), SendError<PollinationMessage>> {
        if self.debounce(&msg) {
            return Ok(());
        }

        self.prev_msg = Some((msg.light_clone(), Instant::now()));
        self.tx.send(msg).await
    }

    fn debounce(&self, msg: &PollinationMessage) -> bool {
        if let Some((prev_msg, timeout)) = &self.prev_msg {
            if timeout.elapsed() > constants::DEBOUNCE_TIMEOUT {
                return false;
            }

            use PollinationMessage::*;
            match (&prev_msg, msg) {
                (
                    Heartbeat {
                        timestamp: ts_old, ..
                    }
                    | Update {
                        timestamp: ts_old, ..
                    },
                    Heartbeat {
                        timestamp: ts_new, ..
                    },
                ) if ts_new <= ts_old => {
                    debug!("Skipping heartbeat since one already sent");
                    true
                }
                (
                    Update {
                        timestamp: ts_old, ..
                    },
                    Update {
                        timestamp: ts_new, ..
                    },
                ) if ts_new <= ts_old => {
                    debug!("Skipping update since one already sent");
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }
}
