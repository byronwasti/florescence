use crate::{mailbox::Mail, traits::Simulee};
use petgraph::graph::NodeIndex;
use std::collections::HashMap;

/// Everything preceding the current moment in time of the Simulation is
/// contained in the History. There are two timestamps which are used, to allow
/// for parallel execution and correct timeout behavior. The `event_time` is
/// derived from the length of events preceding. The `wall_time` is for having
/// parallel execution and timeouts work nicely together.
#[derive(Debug)]
pub struct History<S: Simulee> {
    records: Vec<Option<HistoricalRecord<S>>>,
    wall_time: u64,
    index: HistoricalIndex,
}

impl<S: Simulee> History<S> {
    /// Returns the event time
    pub fn time(&self) -> u64 {
        self.records.len() as u64
    }

    /// Returns the wall time
    ///
    /// The wall
    pub fn wall_time(&self) -> u64 {
        self.wall_time
    }

    /// Record a new event.
    /// Increments the `event_time` always.
    /// Increments the `wall_time` when given `None`.
    pub fn record(&mut self, record: Option<HistoricalRecord<S>>) {
        if let Some(record) = record.as_ref() {
            self.index.insert(record.id, self.records.len() as u64)
        } else {
            // No event in all of the nodes, increment the wall time.
            self.wall_time += 1;
        }
        self.records.push(record);
    }
}

impl<S: Simulee> Default for History<S> {
    fn default() -> Self {
        Self {
            records: vec![],
            wall_time: 0,
            index: HistoricalIndex::default(),
        }
    }
}

#[derive(Debug)]
pub struct HistoricalRecord<S: Simulee> {
    pub id: NodeIndex,
    pub snapshot: S,
    pub event: S::HistoricalEvent,
    pub msg_in: Option<Mail<S::Message>>,
    pub msgs_out: Vec<(NodeIndex, S::Message)>,
}

#[derive(Debug, Default)]
struct HistoricalIndex {
    inner: HashMap<NodeIndex, Vec<u64>>,
}

impl HistoricalIndex {
    fn insert(&mut self, node_index: NodeIndex, event_time: u64) {
        if let Some(v) = self.inner.get_mut(&node_index) {
            v.push(event_time);
        } else {
            self.inner.insert(node_index, vec![event_time]);
        }
    }
}
