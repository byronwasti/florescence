#![allow(unused)]
use crate::widgets::{ForceGraph, ForceGraphConfig, ForceGraphSettingsWidget, ForceGraphWidget};
use egui::{
    Color32, Frame, Painter, Pos2, Rect, Scene, ScrollArea, Sense, Shape, Stroke, Ui, Vec2, emath,
    pos2, vec2,
};
use pollination_simulation::core::{PollinationConfig, SimulatedPollinationCore};
use pollination_simulator::{Config, Sim, history::HistoricalRecord};
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
}

impl Default for DurableState {
    fn default() -> DurableState {
        Self {
            sim_config: Config {
                node_count: 5,
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
    force_graph_state: ForceGraph,
    force_graph_config: ForceGraphConfig,
}

impl EphemeralState {
    fn new(saved: &DurableState) -> Self {
        let sim = Sim::new(saved.sim_config.clone());
        let force_graph_state = ForceGraph::from_graph(sim.graph());
        Self {
            sim,
            step: false,
            scene: Rect::ZERO,
            force_graph_state,
            force_graph_config: ForceGraphConfig::default(),
        }
    }
}

impl eframe::App for PollinationViewer {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.d)
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.e.step {
            println!("Simulation Step");
            self.e.sim.step();
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
            ScrollArea::vertical().auto_shrink(true).show(ui, |ui| {
                let history = self.e.sim.history();
                ui.label(format!("Event time {}", history.time()));
                ui.label(format!("Wall time {}", history.wall_time()));
                for (time, record) in history.records().enumerate() {
                    match record {
                        HistoricalRecord::NodeEvent(record) => {
                            ui.collapsing(
                                format!(
                                    "{time} NodeId={:?} event={:?}",
                                    record.id,
                                    record.event
                                ),
                                |ui| {
                                    ui.label(format!("msg_in={:?}", record.msg_in));
                                    ui.label(format!("msgs_out={:?}", record.msgs_out));
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

    fn draw_controls(&mut self, ui: &egui::Ui, _frame: &mut eframe::Frame) {
        egui::Window::new("Sim Controls").show(ui, |ui| {
            ScrollArea::vertical().auto_shrink(true).show(ui, |ui| {
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
                        ForceGraphWidget::new(&mut self.e.force_graph_state, &mut self.e.force_graph_config)
                            .with_node_color_provider(&|id: u32| {
                                let node = self.e.sim.get_node(id.into()).expect("node");
                                let membership_hash = node.inner().membership_hash();
                                let timestamp = node.inner().timestamp();
                                (
                                    hashable_to_color(membership_hash),
                                    hashable_to_color(timestamp),
                                )
                            })
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
