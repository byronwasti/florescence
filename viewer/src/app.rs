use crate::{
    simulation::{SimConfig, Simulation},
    widgets::{ForceGraph, ForceGraphConfig, ForceGraphSettingsWidget, ForceGraphWidget},
};
use egui::{
    Color32, Frame, Painter, Pos2, Rect, Scene, Sense, Shape, Stroke, Ui, Vec2, emath, pos2, vec2,
};
use fjadra::force::SimulationBuilder;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct PollinationViewer {
    config: ForceGraphConfig,

    #[serde(skip)]
    scene: Rect,

    #[serde(skip)]
    graph: ForceGraph,

    #[serde(skip)]
    simulation: Simulation,

    //#[serde(skip)]
    //event_log: Vec<StepResponse>,

    #[serde(skip)]
    last_step_time: f64,

    simulation_speed: f64,
}

impl Default for PollinationViewer {
    fn default() -> Self {
        let simulation = Simulation::new(10, 1023, 2);
        let graph = ForceGraph::from_graph(&simulation.graph());
        Self {
            scene: Rect::ZERO,
            graph,
            config: ForceGraphConfig::default(),
            simulation,
            last_step_time: 0.,
            simulation_speed: 1.,
        }
    }
}

impl eframe::App for PollinationViewer {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.run_simulation(ctx, frame);
        self.draw_header(ctx, frame);
        self.draw_settings(ctx, frame);
        self.draw_scene(ctx, frame);
    }
}

impl PollinationViewer {
    /// Must be called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }

    fn run_simulation(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let time = ctx.input(|i| i.time);
        if self.last_step_time < (time - self.simulation_speed) {
            println!("Running sim step: {}", self.simulation_speed);
            self.last_step_time = time;
            let res = self.simulation.step(&SimConfig {
                timeout_propagativity: 10,
                timeout_heartbeat: 10,
                timeout_reap: 10,
            });
            //println!("RES: {res:?}");
            //self.event_log.push(res)
        }

        // TODO: More efficient way to do this?
        ctx.request_repaint();
    }

    fn draw_settings(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Settings").show(ctx, |ui| {
            /*
            if ui.button("Step").clicked() {
                self.time += 1.0;
            }

            ui.add(egui::Slider::new(&mut self.config.c, 0.0..=1.0).text("c"));

            if ui.button("Reset").clicked() {
                self.graph = ForceGraph::random();
            }
            */

            //ui.color_edit_button_srgba(&mut self.config.node_color);
            //ui.color_edit_button_srgba(&mut self.config.edge_color);

            ui.add(egui::Slider::new(&mut self.simulation_speed, 0.0..=10.).text("Simulation speed"));
            ui.add(ForceGraphSettingsWidget::new(
                &mut self.graph,
                &mut self.config,
            ));
        });
    }

    fn draw_event_log(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Event Log").show(ctx, |ui| {
        });
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

    fn draw_scene(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let frame = egui::containers::Frame::new()
            .inner_margin(egui::Margin::ZERO)
            .outer_margin(egui::Margin::ZERO);

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            self.draw_scene_stats(ui);

            let mut rect = self.scene;
            Scene::new()
                .max_inner_size([350.0, 1000.0])
                .zoom_range(0.1..=10.0)
                .show(ui, &mut rect, |ui| {
                    ui.add(
                        ForceGraphWidget::new(&mut self.graph, &mut self.config)
                            .with_node_color_provider(&|id: u32| {
                                let reality_token =
                                    self.simulation.get_node(id.into()).inner.reality_token();
                                let timestamp =
                                    self.simulation.get_node(id.into()).inner.timestamp();
                                (
                                    hashable_to_color(reality_token),
                                    hashable_to_color(timestamp),
                                )
                            }),
                    )
                });
            self.scene = rect;
        });
    }

    fn draw_scene_stats(&self, ui: &mut Ui) {
        ui.label(format!("Scene rect: {:#?}", &self.scene));
        ui.label(format!("Time: {:#?}", &ui.input(|i| i.time)));
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
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
