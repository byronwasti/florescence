use tokio::sync::mpsc::{Receiver, Sender, channel, error::SendError};

pub struct WalkieTalkie<T, U> {
    tx: Sender<T>,
    rx: Receiver<U>,
}

impl<T, U> WalkieTalkie<T, U> {
    pub fn pair() -> (WalkieTalkie<T, U>, WalkieTalkie<U, T>) {
        let (tx0, rx0): (Sender<T>, Receiver<T>) = channel(1);
        let (tx1, rx1): (Sender<U>, Receiver<U>) = channel(1);

        (
            WalkieTalkie { tx: tx0, rx: rx1 },
            WalkieTalkie { tx: tx1, rx: rx0 },
        )
    }

    pub async fn send_recv(&mut self, value: T) -> Option<U> {
        self.send(value).await.ok()?;
        self.recv().await
    }

    pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.tx.send(value).await
    }

    pub async fn recv(&mut self) -> Option<U> {
        self.rx.recv().await
    }
}
