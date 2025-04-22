use tokio::task::JoinHandle;

pub struct FlowerHandle {
    pub(crate) handle: JoinHandle<anyhow::Result<()>>,
}

impl FlowerHandle {
    /*
    pub fn pollinator<P: Pollinator + 'static>(&self, interval: Duration) -> P {
        let (pollinator, inner) = P::from_conn(EngineConnection {});
        pollinator
    }
    */

    pub async fn runtime(self) -> anyhow::Result<()> {
        self.handle.await??;
        Ok(())
    }
}
