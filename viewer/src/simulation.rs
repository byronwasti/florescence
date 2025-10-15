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
    pub nodes: Graph<SimNode, ()>,
    pub time: u64,
    seed: u64,
    rng: StdRng,
    rand_robin_count: usize,
    converge_time: Option<u64>,
}

impl Default for Simulation {
    fn default() -> Simulation {
        let seed = rand::rng().random();
        Self {
            nodes: Graph::new(),
            time: 0,
            seed,
            rng: StdRng::seed_from_u64(seed),
            rand_robin_count: 2,
            converge_time: None,
        }
    }
}

impl Simulation {
    pub fn new(
        node_count: usize,
        seed: u64,
        connections: usize,
        rand_robin_count: usize,
    ) -> Simulation {
        let mut rng = StdRng::seed_from_u64(seed);

        let mut nodes = Graph::new();

        for id in 0..node_count {
            nodes.add_node(SimNode {
                inner: PollinationNode::new(
                    Uuid::new_v4(),
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
            time: 0,
            seed,
            rng,
            rand_robin_count,
            converge_time: None,
        }
    }

    pub fn graph(&self) -> &Graph<SimNode, ()> {
        &self.nodes
    }

    pub fn converge_time(&self) -> Option<u64> {
        self.converge_time
    }

    fn converged(&self) -> bool {
        let mut iter = self.nodes.node_weights();
        let first = iter.next().unwrap();
        iter.all(|n| n.inner.reality_token() == first.inner.reality_token())
    }

    pub fn step(&mut self, config: &SimConfig) -> StepResponse {
        self.time += 1;

        let res = self.step_inner(config);

        if self.converge_time.is_none() && self.converged() {
            self.converge_time = Some(self.time)
        }

        res
    }
    pub fn step_inner(&mut self, config: &SimConfig) -> StepResponse {
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
            if last_heartbeat < self.time.saturating_sub(config.timeout_heartbeat) {
                let active_node = self
                    .nodes
                    .node_weight_mut(active_id)
                    .expect("Can't find Node associated with ID");
                active_node.inner.bump();
                active_node.last_heartbeat = self.time;

                let out_message = active_node
                    .inner
                    .msg_heartbeat()
                    .or_else(|| active_node.inner.msg_new_member())
                    .expect("Message should not be None");

                let mut peers = vec![];
                for peer_id in self.nodes.neighbors(active_id) {
                    peers.push(peer_id);
                }

                if peers.is_empty() {
                    for _ in 0..self.rand_robin_count {
                        let peer_id = self.rng.random_range(0..self.nodes.node_count() - 1);
                        let peer_id = if peer_id >= active_id.index() {
                            peer_id + 1
                        } else {
                            peer_id
                        };
                        self.get_node_mut(NodeIndex::new(peer_id))
                            .mailbox
                            .push_back((active_id, out_message.clone()));
                    }
                } else {
                    for peer_id in &peers {
                        self.get_node_mut(*peer_id)
                            .mailbox
                            .push_back((active_id, out_message.clone()));
                    }
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
