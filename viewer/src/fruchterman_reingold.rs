use petgraph::graph::{Graph, UnGraph};
use rand::Rng;
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

    for i in 0..10 {
        for j in 0..10 {
            let _ = g.add_node(Node {
                id: 10 * i + j,
                x: i as f64 - 5.,
                y: j as f64 - 5.,
            });
        }
    }

    let mut rng = rand::thread_rng();
    for _ in 0..100 {
        let a = rng.gen_range(0..100);
        let b = rng.gen_range(0..100);
        if a == b {
            continue;
        }

        g.extend_with_edges(&[(a, b)]);
    }

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
        let hwidth = config.area.0 / 2.;
        if force.0 > 0. && node.x >= hwidth {
            println!("Wall X > for {idx}");
            force.0 = 0.
        } else if force.0 < 0. && node.x <= -hwidth {
            println!("Wall X < for {idx}");
            force.0 = 0.
        }

        let hheight = config.area.1 / 2.;
        if force.1 > 0. && node.y >= hheight {
            println!("Wall Y > for {idx}");
            force.1 = 0.
        } else if force.1 < 0. && node.y <= -hheight {
            println!("Wall Y < for {idx}");
            force.1 = 0.
        }

        net_forces.push(force);
    }

    for (idx, node) in g.node_weights_mut().enumerate() {
        let force = net_forces[idx];

        node.x += force.0;
        node.y += force.1;

        let hwidth = config.area.0 / 2.;
        if node.x >= hwidth {
            node.x = hwidth
        } else if node.x <= -hwidth {
            node.x = -hwidth
        }

        let hheight = config.area.1 / 2.;
        if node.y >= hheight {
            node.y = hheight
        } else if node.y <= -hheight {
            node.y = -hheight
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
