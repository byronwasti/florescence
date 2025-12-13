use crate::config::Config;
use std::panic::UnwindSafe;

///  `Simulee` is each individual node in the simulation.
pub trait Simulee: UnwindSafe {
    type Snapshot;
    type Config: Clone + UnwindSafe;
    type Message: Clone + UnwindSafe;
    type Action: Action + UnwindSafe;
    type HistoricalEvent;

    fn new(config: &Config<Self::Config>, index: usize, rng: u64) -> Self;

    /// Returns an Iterator over (event, probability). The probabilities
    /// do not need to add up to 1., and will be evaluated in-order which necessarily introduces
    /// bias towards earlier events.
    fn list_actions(
        &self,
        wall_time: u64,
        mail_available: bool,
        config: &Self::Config,
    ) -> impl Iterator<Item = (Self::Action, f64)>;

    fn step(
        &mut self,
        event: Self::Action,
        message: Option<Self::Message>,
        wall_time: u64,
        config: &Self::Config,
    ) -> Self::HistoricalEvent;

    fn snapshot(&self) -> Self::Snapshot;
}

pub trait Action {
    fn takes_mail(&self) -> bool;
}
