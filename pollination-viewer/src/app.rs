#![allow(unused)]
use crate::widgets::{ForceGraph, ForceGraphConfig, ForceGraphSettingsWidget, ForceGraphWidget};
use egui::{
    Color32, Frame, Painter, Pos2, Rect, Scene, ScrollArea, Sense, Shape, Stroke, Ui, Vec2, emath,
    pos2, vec2,
};
use pollination_simulation::{PollinationConfig, SimulatedPollinationNode};
use pollination_simulator::{Config, Sim};

pub struct PollinationViewer {
    durable: DurableState,
    ephemeral: EphemeralState,
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
                    timeout_reap: 5,
                    timeout_heartbeat: 5,
                    timeout_propagativity: 5,
                    rand_robin_count: 2,
                },
            },
        }
    }
}

struct EphemeralState {
    sim: Sim<SimulatedPollinationNode>,
    step: bool,
}

impl EphemeralState {
    fn new(saved: &DurableState) -> Self {
        Self {
            sim: Sim::new(saved.sim_config.clone()),
            step: false,
        }
    }
}

impl eframe::App for PollinationViewer {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.durable)
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.ephemeral.step {
            println!("Simulation Step");
            self.ephemeral.sim.step();
        }
        self.draw_header(ctx, frame);
        self.draw_history(ctx, frame);
        self.draw_controls(ctx, frame);
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
            ephemeral: EphemeralState::new(&saved),
            durable: saved,
        }
    }

    fn draw_header(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // No File->Quit on web pages
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });
    }

    fn draw_history(&self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("History").show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink(true).show(ui, |ui| {
                let history = self.ephemeral.sim.history();
                ui.label(format!("Event time {}", history.time()));
                ui.label(format!("Wall time {}", history.wall_time()));
                for (time, record) in history.records().enumerate() {
                    if let Some(record) = record {
                        ui.collapsing(
                            format!(
                                "{time} NodeId={} event={:?}",
                                record.id.index(),
                                record.event
                            ),
                            |ui| {
                                ui.label(format!("msg_in={:?}", record.msg_in));
                                ui.label(format!("msgs_out={:?}", record.msgs_out));
                            },
                        );
                    } else {
                        ui.label("No event took place.");
                    }
                }
            })
        });
    }

    fn draw_controls(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Sim Controls").show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink(true).show(ui, |ui| {
                self.ephemeral.step = ui.button("Step").clicked();
                if let Some(panic) = self.ephemeral.sim.panic_msg() {
                    ui.label(format!("PANIC {panic}"));
                }
            })
        });
    }
}
