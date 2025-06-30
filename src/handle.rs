use crate::{ds::WalkieTalkie, nucleus::Nucleus};
use tokio::task::JoinHandle;

pub struct FlowerHandle<A> {
    flower_comm: WalkieTalkie<(), Nucleus<A>>,
    handle: JoinHandle<anyhow::Result<()>>,
}

impl<A> FlowerHandle<A> {
    /*
    pub fn pollinator<P: Pollinator + 'static>(&self, interval: Duration) -> P {
        let (pollinator, inner) = P::from_conn(EngineConnection {});
        pollinator
    }
    */

    pub(crate) fn new(
        flower_comm: WalkieTalkie<(), Nucleus<A>>,
        handle: JoinHandle<anyhow::Result<()>>,
    ) -> Self {
        Self {
            flower_comm,
            handle,
        }
    }

    pub async fn data(&mut self) -> Option<Nucleus<A>> {
        self.flower_comm.send_recv(()).await
    }

    pub async fn runtime(self) -> anyhow::Result<()> {
        self.handle.await??;
        Ok(())
    }
}
