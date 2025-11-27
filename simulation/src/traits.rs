///  `Simulee` is each individual node in the simulation.
pub trait Simulee {
    type Snapshot;
    type Message: Clone;
    type Event: Event;
    type HistoricalEvent;

    /// Returns an Iterator over (event, probability). The probabilities
    /// do not need to add up to 1., and will be evaluated in-order which necessarily introduces
    /// bias towards earlier events.
    fn events(
        &self,
        wall_time: u64,
        mail_available: bool,
    ) -> impl Iterator<Item = (Self::Event, f64)>;
    fn step(
        &mut self,
        event: Self::Event,
        message: Option<Self::Message>,
        wall_time: u64,
    ) -> Self::HistoricalEvent;
    fn snapshot(&self) -> Self::Snapshot;
}

pub trait Event {
    fn take_mail(&self) -> bool;
}
