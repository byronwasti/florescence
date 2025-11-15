use petgraph::{Graph, graph::NodeIndex};
use pollination::{
    PollinationError, PollinationMessage, PollinationNode, PollinationResponse, Topic,
};
use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet},
};
use uuid::Uuid;

pub struct Sim {
    pub history: History,
    nodes: Graph<SimNode, ()>,
    rng: StdRng,
}

impl Sim {
    pub fn new(config: StartupConfig) -> Self {
        let history = History::default();
        let mut rng = StdRng::seed_from_u64(config.seed);
        let nodes = new_graph(&mut rng, config.node_count, config.connections);

        Self {
            history,
            rng,
            nodes,
        }
    }

    pub fn step(&mut self, config: &StepConfig) {
        let record = self.step_inner(&config);
        self.history.record(record);
    }

    fn step_inner(&mut self, config: &StepConfig) -> Option<HistoricalRecord> {
        let nodes = self.random_ordering();
        for node in nodes {
            println!("Stepping node_id={node:?}");
            if let Some(record) = self.step_node_safely(config, node) {
                return Some(record);
            }
        }

        None
    }

    fn step_node_safely(
        &mut self,
        config: &StepConfig,
        node: NodeIndex,
    ) -> Option<HistoricalRecord> {
        let mut node = self
            .nodes
            .node_weight_mut(node)
            .expect("Can't find Node associated with NodexIndex");
        let saved_core = node.inner.clone();

        let event = node.step(&mut self.rng, self.history.wall_time(), &config);

        event.map(|event| HistoricalRecord {
            node: saved_core,
            event,
        })
    }

    fn random_ordering(&mut self) -> Vec<NodeIndex> {
        let mut node_ids: Vec<_> = (0..self.nodes.node_count())
            .map(|id| NodeIndex::new(id))
            .collect();
        node_ids.shuffle(&mut self.rng);
        node_ids
    }
}

fn new_graph<R: Rng + ?Sized>(
    rng: &mut R,
    node_count: usize,
    connections: usize,
) -> Graph<SimNode, ()> {
    let mut nodes = Graph::new();

    for id in 0..node_count {
        nodes.add_node(SimNode {
            inner: PollinationNode::new(
                Uuid::from_u128(rng.random()),
                Topic::new("b".to_string()),
                NodeIndex::from(id as u32),
            ),
            mailbox: BinaryHeap::new(),
            last_heartbeat: 0,
            last_propagation: 0,
            last_reap: 0,
        });
    }

    for i in 0..node_count {
        for _ in 0..connections {
            let j = rng.random_range(0..node_count - 1);
            let j = if j >= i { j + 1 } else { j };
            let (i, j) = (i as u32, j as u32);

            nodes.add_edge(i.into(), j.into(), ());
        }
    }

    nodes
}

/** Configs **/

pub struct StartupConfig {
    pub node_count: usize,
    pub seed: u64,
    pub connections: usize,
}

pub struct StepConfig {
    pub timeout_propagativity: u64,
    pub timeout_heartbeat: u64,
    pub timeout_reap: u64,

    // Only used if connections == 0
    pub rand_robin_count: usize,
}

impl Default for StepConfig {
    fn default() -> StepConfig {
        StepConfig {
            timeout_propagativity: 13,
            timeout_heartbeat: 5,
            timeout_reap: 8,
            rand_robin_count: 2,
        }
    }
}

/** SimNode **/

#[derive(Debug)]
pub struct SimNode {
    mailbox: BinaryHeap<Mail>,
    inner: PollinationNode<NodeIndex>,
    last_heartbeat: u64,
    last_propagation: u64,
    last_reap: u64,
}

impl Default for SimNode {
    fn default() -> SimNode {
        SimNode {
            inner: PollinationNode::new(
                Uuid::from_u128(0),
                Topic::new("Test".to_string()),
                NodeIndex::new(0),
            ),
            ..Default::default()
        }
    }
}

impl SimNode {
    /// Time is only `peace_time`; we don't want to trigger timeouts on normal prop of events
    /// TODO: Allow more propagation timing shenanigans
    pub fn step<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        time: u64,
        config: &StepConfig,
    ) -> Option<HistoricalEvent> {
        if rng.random_bool(1. / (1. + self.mailbox.len() as f64)) {
            if let h @ Some(_) = self.step_timeout(rng, time, config) {
                return h;
            }
        }

        self.step_mailbox(rng, time, config)
    }

    fn step_timeout<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        time: u64,
        config: &StepConfig,
    ) -> Option<HistoricalEvent> {
        println!("Step timeout");

        if time - self.last_reap > config.timeout_reap {
            self.last_reap = time;

            if self.inner.reap_souls() {
                return Some(HistoricalEvent::GrimTheReaper);
            }
        }

        if time - self.last_heartbeat > config.timeout_heartbeat || self.last_heartbeat == 0 {
            self.last_heartbeat = time;

            if let Some(msg) = self.inner.msg_heartbeat() {
                return Some(HistoricalEvent::Heartbeat { msg });
            }

            let msg = self.inner.msg_new_member().unwrap();
            Some(HistoricalEvent::NewMember { msg })
        } else {
            None
        }
    }

    fn step_mailbox<R: Rng + ?Sized>(
        &mut self,
        rng: &mut R,
        time: u64,
        config: &StepConfig,
    ) -> Option<HistoricalEvent> {
        println!("Step mailbox");

        let in_msg = self.mailbox.pop()?.msg;

        let out = self.inner.handle_message(in_msg.clone());
        match out {
            Ok(PollinationResponse { response, .. }) => Some(HistoricalEvent::HandleMessage {
                in_msg,
                out_msg: response,
            }),

            Err(error) => Some(HistoricalEvent::HandleMessageError { msg: in_msg, error }),
        }
    }
}

#[derive(Debug)]
struct Mail {
    sort: u64,
    msg: PollinationMessage,
}

impl PartialEq for Mail {
    fn eq(&self, other: &Self) -> bool {
        self.sort.eq(&other.sort)
    }
}

impl Eq for Mail {}

impl PartialOrd for Mail {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.sort.partial_cmp(&other.sort)
    }
}

impl Ord for Mail {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort.cmp(&other.sort)
    }
}

/** History **/

/// Everything preceding the current moment in time of the Simulation is
/// contained in the History. There are two timestamps which are used, to allow
/// for parallel execution and correct timeout behavior. The `event_time` is
/// derived from the length of events preceding. The `wall_time` is for having
/// parallel execution and timeouts work nicely together.
#[derive(Debug)]
pub struct History {
    records: Vec<Option<HistoricalRecord>>,
    wall_time: u64,
    nodes_index: HashMap<NodeIndex, Vec<usize>>,
    //stats: Stats,
}

impl History {
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
    pub fn record(&mut self, record: Option<HistoricalRecord>) {
        if record.is_none() {
            self.wall_time += 1;
        }
        self.records.push(record);
    }
}

impl Default for History {
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
pub struct HistoricalRecord {
    pub node: PollinationNode<NodeIndex>,
    pub event: HistoricalEvent,
}

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

#[derive(Debug, Default)]
struct Stats {
    time_to_convergence: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step() {
        let mut sim = Sim::new(StartupConfig {
            node_count: 5,
            seed: 123,
            connections: 2,
        });

        sim.step(&StepConfig::default());

        println!("History: {:?}", sim.history);
    }
}
