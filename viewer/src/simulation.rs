use petgraph::{Graph, graph::NodeIndex};
use pollination::{PollinationMessage, PollinationNode, Topic};
use rand::{Rng, SeedableRng, rngs::StdRng};
use std::collections::VecDeque;
use uuid::Uuid;

pub struct SimConfig {
    pub timeout_propagativity: u64,
    pub timeout_heartbeat: u64,
    pub timeout_reap: u64,
}

pub struct Simulation {
    nodes: Graph<SimNode, ()>,
    timestamp: u64,
    rng: StdRng,
}

impl Simulation {
    pub fn new(node_count: usize, seed: u64, connections: usize) -> Simulation {
        let mut rng = StdRng::seed_from_u64(seed);

        let mut nodes = Graph::new();

        for id in 0..node_count {
            nodes.add_node(SimNode {
                inner: PollinationNode::new(
                    Uuid::from_u128(id as u128),
                    Topic::new("b".to_string()),
                    NodeIndex::from(id as u32),
                ),
                mailbox: VecDeque::new(),
                last_heartbeat: 0,
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

        Self {
            nodes,
            timestamp: 0,
            rng,
        }
    }

    pub fn graph(&self) -> &Graph<SimNode, ()> {
        &self.nodes
    }

    pub fn step(&mut self, config: &SimConfig) -> StepResponse {
        self.timestamp += 1;

        let active_id = NodeIndex::from(self.rng.random_range(0..self.nodes.node_count()) as u32);

        let in_msg = self.get_node_mut(active_id).mailbox.pop_front();

        if let Some((peer_id, in_message)) = in_msg {
            let out = self
                .get_node_mut(active_id)
                .inner
                .handle_message(in_message.clone())
                .expect("Unable to handle message");

            let out_msg = out.response;
            if let Some(out_message) = out_msg {
                self.get_node_mut(peer_id)
                    .mailbox
                    .push_back((active_id, out_message.clone()));
                StepResponse {
                    active_id,
                    peers: vec![peer_id],
                    in_message: Some(in_message),
                    out_message: Some(out_message),
                    ..Default::default()
                }
            } else {
                StepResponse {
                    active_id,
                    in_message: Some(in_message),
                    ..Default::default()
                }
            }
        } else {
            let last_heartbeat = self.get_node_last_heartbeat(active_id);
            if last_heartbeat < self.timestamp.saturating_sub(config.timeout_heartbeat) {
                let active_node = self
                    .nodes
                    .node_weight_mut(active_id)
                    .expect("Can't find Node associated with ID");
                active_node.inner.bump();
                active_node.last_heartbeat = self.timestamp;

                let out_message = active_node
                    .inner
                    .msg_heartbeat()
                    .or_else(|| active_node.inner.msg_new_member())
                    .expect("Message should not be None");

                let mut peers = vec![];
                for peer_id in self.nodes.neighbors(active_id) {
                    peers.push(peer_id);
                }

                for peer_id in &peers {
                    self.get_node_mut(*peer_id)
                        .mailbox
                        .push_back((active_id, out_message.clone()));
                }

                StepResponse {
                    active_id,
                    out_message: Some(out_message),
                    peers,
                    ..Default::default()
                }
            } else {
                // Nothing to do
                StepResponse {
                    active_id,
                    ..Default::default()
                }
            }
        }
    }

    pub fn get_node_last_heartbeat(&self, id: NodeIndex) -> u64 {
        self.get_node(id).last_heartbeat
    }

    pub fn get_node(&self, id: NodeIndex) -> &SimNode {
        self.nodes
            .node_weight(id)
            .expect("Can't find Node associated with ID")
    }

    pub fn get_node_mut(&mut self, id: NodeIndex) -> &mut SimNode {
        self.nodes
            .node_weight_mut(id)
            .expect("Can't find Node associated with ID")
    }
}

#[derive(Default, Debug)]
pub struct StepResponse {
    active_id: NodeIndex,
    peers: Vec<NodeIndex>,
    in_message: Option<PollinationMessage>,
    out_message: Option<PollinationMessage>,
    core_dump: bool,
}

pub struct SimNode {
    pub inner: PollinationNode<NodeIndex>,
    pub last_heartbeat: u64,
    pub mailbox: VecDeque<(NodeIndex, PollinationMessage)>,
}
