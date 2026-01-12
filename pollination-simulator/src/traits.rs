use crate::{config::Config, mailbox::Delivery};
use petgraph::graph::NodeIndex;
use rand::Rng;
use std::{fmt::Debug, panic::UnwindSafe};

///  `Simulee` is each individual node in the simulation.
pub trait Simulee: Clone + UnwindSafe {
    type Config: Clone + UnwindSafe;
    type Message: Debug + Clone + UnwindSafe;
    type HistoricalEvent: Debug;

    fn new<R: Rng + ?Sized>(rng: &mut R, config: &Config<Self::Config>, addr: NodeIndex) -> Self;

    fn step<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        config: &Config<Self::Config>,
        wall_time: u64,
        delivery: &mut Option<Delivery<Self::Message>>,
    ) -> (Self::HistoricalEvent, Vec<(NodeIndex, Self::Message)>);
}
