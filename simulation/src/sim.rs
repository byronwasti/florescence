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

use crate::history::*;
use crate::sim_node::*;
use crate::traits::*;

pub struct Sim<S: Simulee> {
    pub history: History<S::Snapshot, S::Event>,
    nodes: Graph<SimNode<S>, ()>,
    rng: StdRng,
}

impl<S: Simulee> Sim<S> {
    pub fn new(config: StartupConfig) -> Self {
        let history = History::default();
        let mut rng = StdRng::seed_from_u64(config.seed);
        //let nodes = new_graph(&mut rng, config.node_count, config.connections);
        let nodes = todo!();

        Self {
            history,
            rng,
            nodes,
        }
    }

    pub fn step(&mut self, config: &StepConfig) {
        /*
        let record = self.step_inner(&config);
        self.history.record(record);
        */
    }

    /*
    fn step_inner(&mut self, config: &StepConfig) -> Option<HistoricalRecord<S::Snapshot, S::Event>> {
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
    ) -> Option<HistoricalRecord<S::Snapshot, S::Event>> {
        let mut node = self
            .nodes
            .node_weight_mut(node)
            .expect("Can't find Node associated with NodexIndex");
        //let snapshot = node.snapshot();

        //let event = node.step(&mut self.rng, self.history.wall_time(), &config);

        event.map(|event| HistoricalRecord {
            node: todo!(),
            event,
        })
    }
    */

    fn random_ordering(&mut self) -> Vec<NodeIndex> {
        let mut node_ids: Vec<_> = (0..self.nodes.node_count())
            .map(|id| NodeIndex::new(id))
            .collect();
        node_ids.shuffle(&mut self.rng);
        node_ids
    }
}

/*
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
*/

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

    /// Only used if connections == 0
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
