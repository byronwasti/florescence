use petgraph::{graph::NodeIndex, stable_graph::StableGraph};
use pollination::{
    PollinationError, PollinationMessage, PollinationNode, PollinationResponse, Topic,
};
use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet},
};
use thiserror::Error;
use uuid::Uuid;

use crate::{config::*, history::*, sim_node::*, traits::*};

pub struct Sim<S: Simulee> {
    pub history: History<S::Snapshot, S::HistoricalEvent>,
    // TODO: The usage of PetGraph for this is entirely unnecessary
    nodes: StableGraph<SimNode<S>, ()>,
    rng: StdRng,
    panic_msg: Option<String>,
}

impl<S: Simulee> Sim<S> {
    pub fn new(config: &Config<S::Config>) -> Self {
        let history = History::default();
        let mut rng = StdRng::seed_from_u64(config.seed);
        let nodes = new_graph(&mut rng, config);

        Self {
            history,
            rng,
            nodes,
            panic_msg: None,
        }
    }

    pub fn step(&mut self, config: &Config<S::Config>) -> Result<(), SimError> {
        if let Some(panicMsg) = &self.panic_msg {
            return Err(SimError::Panic(panicMsg.clone()));
        }
        let record = self.step_inner(config)?;
        self.history.record(record);

        Ok(())
    }

    fn step_inner(
        &mut self,
        config: &Config<S::Config>,
    ) -> Result<Option<HistoricalRecord<S::Snapshot, S::HistoricalEvent>>, SimError> {
        let nodes = self.random_ordering();
        for node in nodes {
            let node = self.nodes.node_weight_mut(node).expect("Node to exist");
            match node.step(&mut self.rng, self.history.wall_time(), &config.custom) {
                Ok(record) => {
                    return Ok(Some(record));
                }
                Err(SimNodeError::NoAction) => continue,
                Err(err) => {
                    let err_msg = err.to_string();
                    self.panic_msg = Some(err_msg.clone());
                    return Err(SimError::Panic(err_msg));
                }
            }
        }

        Ok(None)
    }

    fn random_ordering(&mut self) -> Vec<NodeIndex> {
        let mut node_ids: Vec<_> = (0..self.nodes.node_count())
            .map(|id| NodeIndex::new(id))
            .collect();
        node_ids.shuffle(&mut self.rng);
        node_ids
    }
}

fn new_graph<R: Rng + ?Sized, S: Simulee>(
    rng: &mut R,
    config: &Config<S::Config>,
) -> StableGraph<SimNode<S>, ()> {
    let mut nodes = StableGraph::new();

    for index in 0..config.node_count {
        nodes.add_node(SimNode::new(rng, config, index));
        /*
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
        */
    }

    /*
    for i in 0..node_count {
        for _ in 0..connections {
            let j = rng.random_range(0..node_count - 1);
            let j = if j >= i { j + 1 } else { j };
            let (i, j) = (i as u32, j as u32);

            nodes.add_edge(i.into(), j.into(), ());
        }
    }
    */

    nodes
}

#[derive(Debug, Error)]
enum SimError {
    #[error("Panic occurred: {0}")]
    Panic(String),
}

#[derive(Debug, Default)]
struct Stats {
    time_to_convergence: Option<u64>,
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step() {
    }
}
*/
