use egui::{Pos2, Vec2, pos2, vec2};
use fjadra::{Center, Link, ManyBody, Node as FNode, SimulationBuilder};
use petgraph::{
    graph::{Graph, UnGraph},
    visit::EdgeRef,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type NodeGraph = Graph<Node, ()>;

#[derive(Debug, Default)]
pub struct Node {
    pub id: usize,
    pub pos: Pos2,
}

pub fn graph() -> Graph<Node, ()> {
    //k_graph(8)
    rand_graph()
}

pub fn k_graph(k: usize) -> Graph<Node, ()> {
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

    g
}

pub fn rand_graph() -> Graph<Node, ()> {
    let mut g = Graph::<Node, ()>::new();

    let sqrt_count = 3;

    for i in 0..sqrt_count {
        for j in 0..sqrt_count {
            let _ = g.add_node(Node {
                id: 10 * i + j,
                pos: pos2(i as f32 - 5., j as f32 - 5.),
            });
        }
    }

    let mut rng = rand::thread_rng();
    let count = sqrt_count.pow(4);
    for _ in 0..count {
        let a = rng.gen_range(0..count);
        let b = rng.gen_range(0..count);
        if a == b {
            continue;
        }

        g.extend_with_edges(&[(a as u32, b as u32)]);
    }

    g
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub area: (f32, f32),
    pub c: f32,
    pub temp: f32,
}

impl Config {
    pub fn k(&self, count: f32) -> f32 {
        let area = (self.area.0 * self.area.1);
        k(self.c, area, count)
    }
}

pub fn fjadra(g: &mut NodeGraph) {
    let nodes = g.node_weights().map(|_| FNode::default());
    let edges = g
        .edge_references()
        .clone()
        .into_iter()
        .map(|e| (e.source().index(), e.target().index()));

    let mut sim = SimulationBuilder::default()
        //.with_velocity_decay(0.1)
        .build(nodes)
        .add_force(
            "link",
            //Link::new(edges).strength(1.0).distance(60.0),
            Link::new(edges),
        )
        .add_force("charge", ManyBody::new())
        .add_force("center", Center::new());

    let positions = sim.iter().last().expect("Sim should always return");

    for (idx, node) in g.node_weights_mut().enumerate() {
        node.pos = pos2(positions[idx][0] as f32, positions[idx][1] as f32)
    }
}

pub fn fruchterman_reingold(g: &mut NodeGraph, config: &Config) {
    let count = g.node_count() as f32;
    let k = config.k(count);

    let mut net_forces = vec![];

    for (idx, node) in g.node_weights().enumerate() {
        let mut force = vec2(0., 0.);

        println!("Force for {idx}:");
        for neighbor in g.neighbors((idx as u32).into()) {
            let neighbor = g.node_weight(neighbor).unwrap();

            let d = node.pos.distance(neighbor.pos);
            let v = (neighbor.pos.to_vec2() - node.pos.to_vec2()).normalized();

            let fa = force_attraction(k, d).clamp(0., config.temp);

            println!("\tComponents: +{fa}");
            force += (v * fa);
        }

        for (jdx, other) in g.node_weights().enumerate() {
            let d = node.pos.distance(other.pos);
            let v = (other.pos.to_vec2() - node.pos.to_vec2()).normalized();

            let fr = force_repulsion(k, d).clamp(-config.temp, 0.);
            println!("\tComponents: -{fr}");
            force += (v * fr);
        }
        println!("\tTotal: {force}");

        // Wall
        /*
        let hwidth = config.area.0 / 2.;
        if force.x > 0. && node.pos.x >= hwidth {
            println!("Wall X > for {idx}");
            force.x = 0.
        } else if force.x < 0. && node.pos.x <= -hwidth {
            println!("Wall X < for {idx}");
            force.y = 0.
        }

        let hheight = config.area.1 / 2.;
        if force.1 > 0. && node.y >= hheight {
            println!("Wall Y > for {idx}");
            force.1 = 0.
        } else if force.1 < 0. && node.y <= -hheight {
            println!("Wall Y < for {idx}");
            force.1 = 0.
        }
        */

        net_forces.push(force);
    }

    for (idx, node) in g.node_weights_mut().enumerate() {
        let force = net_forces[idx];
        println!("Applying force {force:?} to {idx}");

        node.pos += force;

        let hwidth = config.area.0 / 2.;
        let hheight = config.area.1 / 2.;

        let pos = node
            .pos
            .min(pos2(hwidth, hheight))
            .max(pos2(-hwidth, -hheight));
        node.pos = pos;
    }

    println!("Positions");
    for node in g.node_weights() {
        println!("\t{}: {}", node.id, node.pos);
    }
}

fn k(c: f32, area: f32, count: f32) -> f32 {
    c * (area / count).sqrt()
}

fn force_attraction(k: f32, d: f32) -> f32 {
    d.powi(2) / k
}

fn force_repulsion(k: f32, d: f32) -> f32 {
    -k.powi(2) / d
}
