use petgraph::{graph::NodeIndex, stable_graph::StableGraph};
use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};
use thiserror::Error;

use crate::{config::*, history::*, sim_node::*, traits::*};

pub struct Sim<S: Simulee> {
    pub history: History<S>,
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
        if let Some(panic_msg) = &self.panic_msg {
            return Err(SimError::Panic(panic_msg.clone()));
        }
        let record = self.step_inner(config)?;
        self.history.record(record);

        Ok(())
    }

    fn step_inner(
        &mut self,
        config: &Config<S::Config>,
    ) -> Result<Option<HistoricalRecord<S>>, SimError> {
        let nodes = self.random_ordering();
        for node in nodes {
            let node = self.nodes.node_weight_mut(node).expect("Node to exist");
            match node.step(&mut self.rng, self.history.wall_time(), config) {
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
        let mut node_ids: Vec<_> = (0..self.nodes.node_count()).map(NodeIndex::new).collect();
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
        let id = NodeIndex::new(index);
        nodes.add_node(SimNode::new(rng, config, id));
    }

    nodes
}

#[derive(Debug, Error)]
pub enum SimError {
    #[error("Panic occurred: {0}")]
    Panic(String),
}
