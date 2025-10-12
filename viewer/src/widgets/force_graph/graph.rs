use egui::{Pos2, Vec2, pos2, vec2};
use fjadra::{Center, Link, ManyBody, Node as FNode, PositionX, PositionY, SimulationBuilder};
use petgraph::{
    dot::{Config, Dot},
    graph::{Graph, UnGraph},
    visit::EdgeRef,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Node {
    pub id: usize,
    pub pos: Pos2,
}

pub struct ForceGraph {
    inner: Graph<Node, ()>,
    pub(crate) state: State,
}

pub(crate) struct State {
    pub first: bool,
    pub interact: bool,
}

impl ForceGraph {
    pub fn from_inner(inner: Graph<Node, ()>) -> Self {
        Self {
            inner,
            state: State {
                first: true,
                interact: false,
            },
        }
    }

    pub fn inner(&self) -> &Graph<Node, ()> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut Graph<Node, ()> {
        &mut self.inner
    }

    pub fn random() -> ForceGraph {
        let mut g = Graph::<Node, ()>::new();

        let count = 20;

        for i in 0..count {
            let _ = g.add_node(Node {
                id: i,
                pos: pos2(0., 0.),
            });
        }

        let mut rng = rand::thread_rng();

        let conns = 2;
        for i in 0..count {
            for _ in 0..conns {
                let j = rng.random_range(0..count - 1);
                let j = if j >= i { j + 1 } else { j };

                println!("Extending {i} to {j}");
                g.extend_with_edges(&[(i as u32, j as u32)]);
            }
        }

        let dot = Dot::new(&g);
        std::fs::write("graph.dot", format!("{:?}", dot)).unwrap();

        ForceGraph::from_inner(g)
    }

    pub fn k_graph(k: usize) -> ForceGraph {
        let mut g = Graph::<Node, ()>::new();

        let mut rng = rand::thread_rng();
        for i in 0..k {
            g.add_node(Node {
                id: i,
                pos: pos2(rng.random(), rng.random()),
            });
        }

        for i in 0..k {
            for j in 0..k {
                if i == j {
                    continue;
                }

                g.extend_with_edges(&[(i as u32, j as u32)]);
            }
        }

        ForceGraph::from_inner(g)
    }

    pub fn run_force_simulation(&mut self, config: &super::ForceGraphConfig, fixed: &[usize]) {
        if !self.state.first && !self.state.interact {
            return;
        }
        println!("Running force sim");

        let g = &mut self.inner;
        let nodes = g.node_weights().enumerate().map(|(idx, n)| {
            if fixed.contains(&idx) {
                FNode::default().fixed_position(n.pos.x as f64, n.pos.y as f64)
            } else {
                if self.state.first {
                    FNode::default()
                } else {
                    FNode::default().position(n.pos.x as f64, n.pos.y as f64)
                }
            }
        });
        let edges = g
            .edge_references()
            .clone()
            .into_iter()
            .map(|e| (e.source().index(), e.target().index()));

        let mut sim = SimulationBuilder::default();

        if !self.state.first {
            sim = sim.with_alpha(0.1);
            sim = sim.with_velocity_decay(config.velocity_decay);
        }

        let mut link = Link::new(edges);
        if config.link_strength_enabled {
            link = link.strength(config.link_strength);
        }
        if config.link_distance_enabled {
            link = link.distance(config.link_distance);
        }

        let mut sim = sim
            .build(nodes)
            .add_force("link", link)
            .add_force("charge", ManyBody::new())
            //.add_force("positionx", PositionY::new().strength(0.001))
            //.add_force("positiony", PositionX::new().strength(0.1))
            .add_force("center", Center::new());

        let positions = sim.iter().last().expect("Sim should always return");

        for (idx, node) in g.node_weights_mut().enumerate() {
            node.pos = pos2(positions[idx][0] as f32, positions[idx][1] as f32)
        }

        self.state.first = false;
    }
}
