#![allow(unused)]
use crate::widgets::{ForceGraphSettingsWidget, ForceGraphState, ForceGraphWidget};
use egui::{
    Color32, Frame, Painter, Pos2, Rect, Scene, ScrollArea, Sense, Shape, Stroke, Ui, Vec2, emath,
    pos2, vec2,
};
use egui_plot::{Legend, Line, Plot, PlotPoints, HoverPosition};
use pollination_simulation::core::{
    PollinationConfig, PollinationEvent, PollinationMessage, SimulatedPollinationCore, PollinationCore,
};
use pollination_simulator::{Config, Mail, NodeIndex, Sim, SimNode, history::HistoricalRecord};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

pub struct PollinationViewer {
    d: DurableState,
    e: EphemeralState,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct DurableState {
    sim_config: Config<PollinationConfig>,
    step_count: usize,
    truncate_history: bool,
    truncate_to: usize,
}

impl Default for DurableState {
    fn default() -> DurableState {
        Self {
            step_count: 1,
            sim_config: Config {
                node_count: 3,
                seed: 1234,
                custom: PollinationConfig {
                    rand_robin_count: 2,
                },
            },
            truncate_history: true,
            truncate_to: 1000,
        }
    }
}

struct EphemeralState {
    sim: Sim<SimulatedPollinationCore>,
    step: bool,
    scene: Rect,
    force_graph_state: ForceGraphState,
    run_to_convergence: bool,
}

impl EphemeralState {
    fn new(saved: &DurableState) -> Self {
        let sim = Sim::new(saved.sim_config.clone());
        let force_graph_state = ForceGraphState::new(sim.graph());
        Self {
            sim,
            step: false,
            scene: Rect::from_two_pos(Pos2::new(-500.0, -300.0), Pos2::new(500.0, 300.0)),
            force_graph_state,
            run_to_convergence: false,
        }
    }
}

impl eframe::App for PollinationViewer {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.d)
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.e.step {
            println!("Simulation Step ({}x)", self.d.step_count);
            for _ in 0..self.d.step_count {
                self.e.sim.step();
            }
        }

        if self.d.truncate_history {
            let history = self.e.sim.history_mut();
            if history.records().len() >= 2 * self.d.truncate_to {
                history.truncate(self.d.truncate_to);
            }
        }

        if self.e.run_to_convergence && !has_converged(&self.e.sim) {
            for _ in 0..self.d.step_count {
                self.e.sim.step();
            }
            ctx.request_repaint();
        } else {
            self.e.run_to_convergence = false;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.draw_header(ui, frame);
        self.draw_history(ui, frame);
        self.draw_controls(ui, frame);
        self.draw_membership_hash_distribution(ui, frame);
        self.draw_scene(ui, frame);
    }
}

impl PollinationViewer {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let saved = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        PollinationViewer {
            e: EphemeralState::new(&saved),
            d: saved,
        }
    }

    fn reset(&mut self) {
        let scene = self.e.scene;
        self.e = EphemeralState::new(&self.d);
        self.e.scene = scene;
    }

    fn draw_header(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top_panel").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // No File->Quit on web pages
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ui.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });
    }

    fn draw_history(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Window::new("History").show(ui, |ui| {
            ui.checkbox(&mut self.d.truncate_history, "Truncate history");
            if self.d.truncate_history {
                ui.add(egui::Slider::new(&mut self.d.truncate_to, 1..=10000).text("Truncate to"));
            }
            ScrollArea::vertical()
                .stick_to_bottom(true)
                //.auto_shrink(true)
                .show(ui, |ui| {
                    let history = self.e.sim.history();
                    ui.label(format!("Event time {}", history.time()));
                    ui.label(format!("Wall time {}", history.wall_time()));

                    for (offset, record) in history.records_iter().enumerate() {
                        match record {
                            HistoricalRecord::NodeEvent(record) => {
                                let from_node =
                                    if matches!(record.event, PollinationEvent::HandleMessage) {
                                        let msg = record.msg_in.as_ref().expect("Some message");
                                        format!("from={:?}", msg.from)
                                    } else {
                                        "".to_string()
                                    };

                                // TODO: Move this into the historical record, this is a silly thing
                                // to do
                                let event_time = history.time() - history.records().len() as u64 + offset as u64;
                                ui.collapsing(
                                    format!(
                                        "{event_time} NodeId={:?} event={:?} {}",
                                        record.id, record.event, from_node,
                                    ),
                                    |ui| {
                                        ui.collapsing("Pre Node State", |ui| {
                                            draw_node_info(ui, record.snapshot.inner());
                                        });
                                        ui.collapsing("Msg In", |ui| {
                                            self.draw_msg_in(ui, record.msg_in.as_ref());
                                        });
                                        ui.collapsing("Msgs Out", |ui| {
                                            for msg in record.msgs_out.iter() {
                                                self.draw_msg_out(ui, msg);
                                            }
                                        });
                                    },
                                );
                            }
                            HistoricalRecord::NoEvent => {
                                ui.label("No event took place.");
                            }
                            HistoricalRecord::Error(node_id, error) => {
                                ui.label(format!("{node_id:?} had an error {error}"));
                            }
                        }
                    }
                })
        });
    }

    fn draw_msg_in(&self, ui: &mut egui::Ui, msg: Option<&Mail<PollinationMessage<NodeIndex>>>) {
        let Some(msg) = msg else {
            ui.label("None");
            return;
        };

        ui.label(format!("{:?} => {}", &msg.from, &msg.msg));
    }

    fn draw_msg_out(
        &self,
        ui: &mut egui::Ui,
        (to, msg): &(NodeIndex, PollinationMessage<NodeIndex>),
    ) {
        ui.label(format!("{:?} => {}", to, &msg));
    }

    fn draw_controls(&mut self, ui: &egui::Ui, _frame: &mut eframe::Frame) {
        egui::Window::new("Sim Controls").show(ui, |ui| {
            ScrollArea::vertical().auto_shrink(true).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Seed");
                    ui.add(egui::DragValue::new(&mut self.d.sim_config.seed).speed(1));
                });
                ui.add(
                    egui::Slider::new(&mut self.d.sim_config.node_count, 0..=1000)
                        .text("Node count"),
                );
                if ui.button("Reset").clicked() {
                    self.reset();
                }

                ui.separator();

                ui.add(egui::Slider::new(&mut self.d.step_count, 0..=10000).text("Step Count"));
                self.e.step = ui.button("Step").clicked();
                if let Some(panic) = self.e.sim.panic_msg() {
                    ui.label(format!("PANIC {panic}"));
                }

                ui.toggle_value(&mut self.e.run_to_convergence, "Run to convergence");
                if self.e.run_to_convergence {
                    ui.spinner();
                }
            })
        });
    }

    fn draw_membership_hash_distribution(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Window::new("Membership Hash Distribution").show(ui, |ui| {
            draw_membership_hash_distribution_plot(ui, &self.e.sim);
        });
    }

    fn draw_scene(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let frame = egui::containers::Frame::new()
            .inner_margin(egui::Margin::ZERO)
            .outer_margin(egui::Margin::ZERO);

        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            self.draw_scene_stats(ui);

            let mut rect = self.e.scene;
            Scene::new()
                .max_inner_size([350.0, 1000.0])
                .zoom_range(0.1..=10.0)
                .show(ui, &mut rect, |ui| {
                    ui.add(
                        ForceGraphWidget::new(&mut self.e.force_graph_state)
                            .with_node_color_provider(&|id: u32| {
                                let node = self.e.sim.get_node(id.into()).expect("node");
                                let membership_hash = node.inner().membership_hash();
                                let timestamp = node.inner().timestamp();
                                (
                                    //hashable_to_color(timestamp),
                                    hashable_to_color(membership_hash),
                                    hashable_to_color(membership_hash),
                                )
                            })
                            .with_node_info_provider(&|ui, id| {
                                let Some(node) = self.e.sim.get_node(id) else {
                                    return;
                                };
                                draw_sim_node_info(ui, node);
                            }),
                    )
                });
            self.e.scene = rect;
        });
    }

    fn draw_scene_stats(&self, ui: &mut Ui) {
        ui.label(format!("Seconds since start: {:#?}", &ui.input(|i| i.time)));
        ui.label(format!("Rect: {:#?}", &self.e.scene));
        ui.label(format!("Event Time: {:#?}", &self.e.sim.history.time()));
        ui.label(format!("Wall Time: {:#?}", &self.e.sim.history.wall_time()));
    }
}

fn hashable_to_color<T: Hash>(hashable: T) -> Color32 {
    let mut hasher = DefaultHasher::new();
    hashable.hash(&mut hasher);
    let hash = hasher.finish();
    let red = hash as u8;
    let green = (hash >> 8) as u8;
    let blue = (hash >> 16) as u8;
    Color32::from_rgb(red, green, blue)
}

fn has_converged(sim: &Sim<SimulatedPollinationCore>) -> bool {
    sim.has_converged(|s: &SimulatedPollinationCore| s.membership_hash())
}

/* Pseudo Components */

fn draw_sim_node_info(ui: &mut Ui, node: &SimNode<SimulatedPollinationCore>) {
    egui::ScrollArea::vertical()
        .auto_shrink(true)
        .show(ui, |ui| {
            ui.label(format!("Node Index: {}", node.id.index()));

            ui.collapsing("State", |ui| {
                let node = node.inner().inner();
                draw_node_info(ui, &node);
            });

            ui.collapsing("Mailbox", |ui| {
                for mail in node.mailbox.iter() {
                    ui.label(format!(
                            "{} -> {:?}",
                            mail.from.index(),
                            mail.msg
                    ));
                }
            });
        });
}

fn draw_node_info(ui: &mut Ui, node: &PollinationCore<NodeIndex>) {
    ui.label(format!("UUID: {}", node.uuid()));
    ui.label(format!("Membership Hash: {:?}", node.membership_hash()));
    ui.label(format!("ItcId: {}", node.id()));
    ui.label(format!("Timestamp: {}", node.timestamp()));
    ui.label(format!("Own Info: {:?}", node.own_info()));
    ui.collapsing(format!("Map ({})", node.core_map().len()), |ui| {
        for (id, d) in node.core_map().iter() {
            ui.label(format!("{id} -> {d:?}"));
        }
    });
}


/// Plots, for every distinct membership hash that has appeared in the (possibly
/// truncated) history, how many nodes currently carry that hash, over event time.
/// A converging simulation shows its hash groups merging into a single line that
/// climbs to the total node count.
fn draw_membership_hash_distribution_plot(ui: &mut Ui, sim: &Sim<SimulatedPollinationCore>) {
    let history = sim.history();
    let node_count = sim.nodes().count();

    // Event time of the first record still retained in the (possibly truncated) history.
    let start_time = history.time() - history.records().len() as u64;

    // Replay history, tracking each node's latest known membership hash so we can
    // derive, at every state change, how many nodes currently belong to each group.
    let mut latest: HashMap<NodeIndex, u64> = HashMap::new();
    let mut counts: HashMap<u64, usize> = HashMap::new();
    let mut series: HashMap<u64, Vec<(f64, f64)>> = HashMap::new();

    for (offset, record) in history.records_iter().enumerate() {
        let HistoricalRecord::NodeEvent(node_record) = record else {
            continue;
        };

        let new_hash = node_record.snapshot.membership_hash().u64();
        let old_hash = latest.insert(node_record.id, new_hash);
        if old_hash == Some(new_hash) {
            continue;
        }

        let event_time = (start_time + offset as u64) as f64;

        if let Some(old_hash) = old_hash {
            let count = counts.entry(old_hash).or_insert(0);
            *count -= 1;
            series.entry(old_hash).or_default().push((event_time, *count as f64));
        }

        let count = counts.entry(new_hash).or_insert(0);
        *count += 1;
        series.entry(new_hash).or_default().push((event_time, *count as f64));
    }

    ui.label("Membership Hash Distribution");

    if series.is_empty() {
        ui.label("No history yet.");
        return;
    }

    let mut hashes: Vec<u64> = series.keys().copied().collect();
    hashes.sort_unstable();

    Plot::new("membership_hash_distribution")
        .height(200.0)
        //.legend(Legend::default())
        .include_y(0.0)
        .include_y(node_count as f64)
        .show_crosshair(true)
        .x_axis_label("Event Time")
        .y_axis_label("Node Count")
        .label_formatter(|pos| match pos {
            HoverPosition::NearDataPoint { plot_name, position, .. } if !plot_name.is_empty() => {
                Some(format!("{}: {}", plot_name, position.y))
            }
            _ => None,
        })
        .show(ui, |plot_ui| {
            for hash in hashes {
                let points: Vec<[f64; 2]> = series[&hash]
                    .iter()
                    .map(|&(time, count)| [time, count])
                    .collect();
                let line = Line::new(format!("{hash}"), PlotPoints::new(points))
                    .color(hashable_to_color(hash));
                plot_ui.line(line);
            }
        });
}
