use petgraph::graph::NodeIndex;
use std::collections::HashMap;

/// Everything preceding the current moment in time of the Simulation is
/// contained in the History. There are two timestamps which are used, to allow
/// for parallel execution and correct timeout behavior. The `event_time` is
/// derived from the length of events preceding. The `wall_time` is for having
/// parallel execution and timeouts work nicely together.
#[derive(Debug)]
pub struct History<Snapshot, HistoricalEvent> {
    records: Vec<Option<HistoricalRecord<Snapshot, HistoricalEvent>>>,
    wall_time: u64,
    nodes_index: HashMap<NodeIndex, Vec<usize>>,
    //stats: Stats,
}

impl<Snapshot, HistoricalEvent> History<Snapshot, HistoricalEvent> {
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
    /// Increments the `.wall_time` when given `None`.
    pub fn record(&mut self, record: Option<HistoricalRecord<Snapshot, HistoricalEvent>>) {
        if record.is_none() {
            self.wall_time += 1;
        }
        self.records.push(record);
    }
}

impl<S, E> Default for History<S, E> {
    fn default() -> Self {
        Self {
            records: vec![],
            wall_time: 0,
            nodes_index: HashMap::new(),
            //stats: Stats::default(),
        }
    }
}

#[derive(Debug)]
pub struct HistoricalRecord<Snapshot, HistoricalEvent> {
    pub node_snapshot: Snapshot,
    pub event: HistoricalEvent,
}

/*
#[derive(Debug)]
pub enum HistoricalEvent {
    NewMember {
        msg: PollinationMessage,
    },
    Heartbeat {
        msg: PollinationMessage,
    },
    GrimTheReaper,
    HandleMessage {
        in_msg: PollinationMessage,
        out_msg: Option<PollinationMessage>,
    },
    HandleMessageError {
        msg: PollinationMessage,
        error: PollinationError,
    },
    Panic {
        err: String,
    },
}
*/
