use petgraph::graph::{Graph, UnGraph};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type NodeGraph = Graph<Node, ()>;

#[derive(Debug, Default)]
pub struct Node {
    pub id: usize,
    pub x: f64,
    pub y: f64,
}

pub fn graph() -> Graph<Node, ()> {
    let mut g = Graph::<Node, ()>::new();

    let a = g.add_node(Node {
        id: 0,
        x: 12.,
        y: 13.,
    });

    let b = g.add_node(Node {
        id: 1,
        x: 1.,
        y: 32.,
    });

    let c = g.add_node(Node {
        id: 2,
        x: 5.,
        y: 5.,
    });

    let d = g.add_node(Node {
        id: 3,
        x: 4.,
        y: 100.,
    });

    g.extend_with_edges(&[(a, b), (b, c), (b, d)]);

    g
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub area: (f64, f64),
    pub d: f64,
    pub c: f64,
    pub temp: f64,
}

impl Config {
    pub fn k(&self, count: f64) -> f64 {
        let area = (self.area.0 * self.area.1);
        k(self.c, area, count)
    }
}

pub fn fruchterman_reingold(g: &mut NodeGraph, config: &Config) {
    let count = g.node_count() as f64;
    let k = config.k(count);

    let mut net_forces = vec![];

    for (idx, node) in g.node_weights().enumerate() {
        let mut force = (0., 0.);

        println!("Force for {idx}:");
        for neighbor in g.neighbors((idx as u32).into()) {
            let neighbor = g.node_weight(neighbor).unwrap();

            let fax = force_attraction(k, (node.x - neighbor.x).abs()).clamp(0., config.temp);
            let fay = force_attraction(k, (node.y - neighbor.y).abs()).clamp(0., config.temp);

            println!("\tComponents: +({fax}, {fay})");
            force = (force.0 + fax, force.1 + fay);
        }

        for (jdx, other_node) in g.node_weights().enumerate() {
            let frx = force_repulsion(k, (node.x - other_node.x).abs()).clamp(0., config.temp);
            let fry = force_repulsion(k, (node.y - other_node.y).abs()).clamp(0., config.temp);
            println!("\tComponents: -({frx}, {fry})");
            force = (force.0 + frx, force.1 + fry);
        }
        println!("\tTotal: ({}, {})", force.0, force.1);

        // Wall
        if force.0 > 0. && node.x >= config.area.0 {
            println!("Wall X > for {idx}");
            force.0 = 0.
        } else if force.0 < 0. && node.x <= config.area.0 {
            println!("Wall X < for {idx}");
            force.0 = 0.
        }

        if force.1 > 0. && node.y >= config.area.1 {
            println!("Wall Y > for {idx}");
            force.1 = 0.
        } else if force.1 < 0. && node.y <= config.area.1 {
            println!("Wall Y < for {idx}");
            force.1 = 0.
        }

        net_forces.push(force);
    }

    for (idx, node) in g.node_weights_mut().enumerate() {
        let force = net_forces[idx];

        node.x += force.0;
        node.y += force.1;

        if node.x >= config.area.0 {
            node.x = config.area.0
        } else if node.x <= 0.0 {
            node.x = 0.0
        }

        if node.y >= config.area.1 {
            node.y = config.area.1
        } else if node.y <= 0.0 {
            node.y = 0.0
        }

        println!("Applying force {force:?} to {idx}");
    }

    println!("Positions");
    for node in g.node_weights() {
        println!("\t{}: ({}, {})", node.id, node.x, node.y);
    }
}

fn k(c: f64, area: f64, count: f64) -> f64 {
    c * (area / count).sqrt()
}

fn force_attraction(k: f64, d: f64) -> f64 {
    d.powi(2) / k
}

fn force_repulsion(k: f64, d: f64) -> f64 {
    -k.powi(2) / d
}
