use crate::{mailbox::Mail, traits::Simulee};
use petgraph::graph::NodeIndex;
use std::collections::{HashMap, VecDeque};

/// Everything preceding the current moment in time of the Simulation is
/// contained in the History. There are two timestamps which are used, to allow
/// for parallel execution and correct timeout behavior. The `event_time` is
/// derived from the length of events preceding. The `wall_time` is for having
/// parallel execution and timeouts work nicely together.
#[derive(Debug)]
pub struct History<S: Simulee> {
    records: VecDeque<HistoricalRecord<S>>,
    wall_time: u64,
    event_time: u64,
    index: HistoricalIndex,
    truncate: Option<usize>,
}

impl<S: Simulee> History<S> {
    /// Returns the event time
    pub fn time(&self) -> u64 {
        self.event_time
    }

    /// Returns the wall time
    ///
    /// The wall
    pub fn wall_time(&self) -> u64 {
        self.wall_time
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn records_iter(&self) -> impl Iterator<Item = &HistoricalRecord<S>> {
        self.records.iter()
    }

    pub fn truncate(&mut self, count: usize) {
        self.truncate = Some(count);
        self.records.retain_back(count);
        //let to_cut = self.records.len() - count;
        //self.records.drain(..to_cut);
    }

    pub fn last(&self) -> Option<&HistoricalRecord<S>> {
        self.records.back()
    }

    /// Record a record. English is fun.
    /// Increments the `event_time` always.
    /// Increments the `wall_time` when given `None`.
    pub(crate) fn record(&mut self, record: HistoricalRecord<S>) {
        match &record {
            HistoricalRecord::NodeEvent(node_record) => {
                self.index.insert(node_record.id, self.records.len() as u64)
            }
            HistoricalRecord::NoEvent => {
                self.wall_time += 1;
            }
            _ => {}
        }

        if let Some(truncate) = self.truncate
            && self.records.len() == truncate
        {
            self.records.pop_front();
        }
        self.records.push_back(record);
        self.event_time += 1;
    }
}

impl<S: Simulee> Default for History<S> {
    fn default() -> Self {
        Self {
            records: VecDeque::new(),
            wall_time: 0,
            event_time: 0,
            index: HistoricalIndex::default(),
            truncate: None,
        }
    }
}

#[derive(Debug)]
pub enum HistoricalRecord<S: Simulee> {
    NodeEvent(NodeRecord<S>),
    Error(NodeIndex, String),
    NoEvent,
}

impl<S: Simulee> HistoricalRecord<S> {
    pub fn node_id(&self) -> Option<NodeIndex> {
        match self {
            HistoricalRecord::NodeEvent(rec) => Some(rec.id),
            HistoricalRecord::Error(id, _) => Some(*id),
            HistoricalRecord::NoEvent => None,
        }
    }
}

#[derive(Debug)]
pub struct NodeRecord<S: Simulee> {
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
