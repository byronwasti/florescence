#![allow(unused)]
use crate::widgets::{ForceGraph, ForceGraphConfig, ForceGraphSettingsWidget, ForceGraphWidget};
use egui::{
    Color32, Frame, Painter, Pos2, Rect, Scene, ScrollArea, Sense, Shape, Stroke, Ui, Vec2, emath,
    pos2, vec2,
};
use pollination_simulation::SimulatedPollinationNode;
use pollination_simulator::{Config, Sim};

#[derive(Default)]
pub struct PollinationViewer {
    durable: DurableState,
    ephemeral: EphemeralState,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct DurableState {}

#[derive(Default)]
struct EphemeralState {
    sim: Option<Sim<SimulatedPollinationNode>>,
}

impl eframe::App for PollinationViewer {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.durable)
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.draw_header(ctx, frame);
        self.draw_overview(ctx, frame);
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
            durable: saved,
            ..Default::default()
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

    fn draw_overview(&self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Overview").show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink(true).show(ui, |ui| {
                ui.label("Node Overview");
            })
        });
    }
}
