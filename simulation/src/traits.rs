///  `Simulatee` is each individual node in the simulation.
pub trait Simulatee {
    type Snapshot;
    type Event;
    type HistoricalEvent;

    /// Returns an Iterator over (event, probability). The probabilities
    /// do not need to add up to 1., and will be evaluated in-order.
    fn events(&self) -> impl Iterator<Item = (Self::Event, f64)>;
    fn step(&mut self, event: Self::Event) -> Option<Self::HistoricalEvent>;
    fn snapshot(&self) -> Self::Snapshot;
}
