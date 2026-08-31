#![allow(unused)]
use crate::widgets::{ForceGraphSettingsWidget, ForceGraphState, ForceGraphWidget};
use egui::{
    Color32, Frame, Painter, Pos2, Rect, Scene, ScrollArea, Sense, Shape, Stroke, Ui, Vec2, emath,
    pos2, vec2,
};
use pollination_simulation::core::{PollinationConfig, SimulatedPollinationCore, PollinationMessage, PollinationEvent};
use pollination_simulator::{Config, Sim, history::HistoricalRecord, Mail, NodeIndex};
use std::{
    collections::hash_map::DefaultHasher,
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
        }
    }
}

struct EphemeralState {
    sim: Sim<SimulatedPollinationCore>,
    step: bool,
    scene: Rect,
    force_graph_state: ForceGraphState,
}

impl EphemeralState {
    fn new(saved: &DurableState) -> Self {
        let sim = Sim::new(saved.sim_config.clone());
        let force_graph_state = ForceGraphState::new(sim.graph());
        Self {
            sim,
            step: false,
            scene: Rect::ZERO,
            force_graph_state,
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
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.draw_header(ui, frame);
        self.draw_history(ui, frame);
        self.draw_controls(ui, frame);
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

    fn draw_history(&self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Window::new("History").show(ui, |ui| {
            ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink(true)
                .show(ui, |ui| {
                    let history = self.e.sim.history();
                    ui.label(format!("Event time {}", history.time()));
                    ui.label(format!("Wall time {}", history.wall_time()));
                    for (time, record) in history.records().enumerate() {
                        match record {
                            HistoricalRecord::NodeEvent(record) => {
                                let from_node = if matches!(record.event, PollinationEvent::HandleMessage) {
                                    let msg = record.msg_in.as_ref().expect("Some message");
                                    format!("from={:?}", msg.from)
                                } else {
                                    "".to_string()
                                };
                                ui.collapsing(
                                    format!(
                                        "{time} NodeId={:?} event={:?} {}",
                                        record.id, record.event, from_node,
                                    ),
                                    |ui| {
                                        ui.collapsing("Msg In", |ui| {
                                            self.draw_msg_in(ui, record.msg_in.as_ref());
                                        });
                                        ui.collapsing("Msgs Out", |ui| {
                                            for msg in record.msgs_out.iter() {
                                                self.draw_msg_out(ui, msg);
                                            }
                                            //ui.label(format!("msgs_out={:?}", record.msgs_out));
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

        ui.label(format!("{:?} => {} ({:?})", &msg.from, &msg.msg, msg.sort));
    }

    fn draw_msg_out(&self, ui: &mut egui::Ui, (to, msg): &(NodeIndex, PollinationMessage<NodeIndex>)) {
        ui.label(format!("{:?} => {}", to, &msg));
    }

    fn draw_controls(&mut self, ui: &egui::Ui, _frame: &mut eframe::Frame) {
        egui::Window::new("Sim Controls").show(ui, |ui| {
            ScrollArea::vertical().auto_shrink(true).show(ui, |ui| {
                ui.add(
                    egui::Slider::new(&mut self.d.sim_config.node_count, 0..=1000).text("Node count"),
                );
                if ui.button("Reset").clicked() {
                    self.e = EphemeralState::new(&self.d);
                }

                ui.separator();

                ui.add(egui::Slider::new(&mut self.d.step_count, 0..=10000).text("Step Count"));
                self.e.step = ui.button("Step").clicked();
                if let Some(panic) = self.e.sim.panic_msg() {
                    ui.label(format!("PANIC {panic}"));
                }
            })
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
                                    hashable_to_color(timestamp),
                                    hashable_to_color(membership_hash),
                                )
                            })
                            .with_node_info_provider(&|id, ui| {
                                let Some(node) = self.e.sim.get_node(id) else {
                                    return;
                                };
                                egui::ScrollArea::vertical()
                                    .auto_shrink(true)
                                    .show(ui, |ui| {
                                        ui.label(format!("Node Index: {}", node.id.index()));
                                        ui.label(format!(
                                            "Membership Hash: {:?}",
                                            node.inner().membership_hash()
                                        ));

                                        ui.collapsing("State", |ui| {
                                            let node = node.inner().inner();
                                            ui.label(format!("UUID: {}", node.uuid()));
                                            ui.label(format!("ItcId: {}", node.id()));
                                            ui.label(format!("Timestamp: {}", node.timestamp()));
                                            ui.label(format!("Own Info: {:?}", node.own_info()));
                                            ui.collapsing("Map", |ui| {
                                                for (id, d) in node.core_map().iter() {
                                                    ui.label(format!("{id} -> {d:?}"));
                                                }
                                            });
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
