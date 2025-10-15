use crate::{
    simulation::{SimConfig, Simulation},
    widgets::{ForceGraph, ForceGraphConfig, ForceGraphSettingsWidget, ForceGraphWidget},
};
use egui::{
    Color32, Frame, Painter, Pos2, Rect, Scene, ScrollArea, Sense, Shape, Stroke, Ui, Vec2, emath,
    pos2, vec2,
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
    per_step: usize,
    node_count: usize,
    connection_count: usize,
    rand_robin_count: usize,
    seed: u64,

    #[serde(skip)]
    play: bool,

    #[serde(skip)]
    first: bool,
}

impl Default for PollinationViewer {
    fn default() -> Self {
        let simulation = Simulation::new(10, 1234, 2, 2);
        let graph = ForceGraph::from_graph(&simulation.graph());
        Self {
            scene: Rect::ZERO,
            graph,
            config: ForceGraphConfig::default(),
            simulation,
            last_step_time: 0.,
            simulation_speed: 1.,
            per_step: 1,
            node_count: 10,
            connection_count: 2,
            rand_robin_count: 2,
            seed: 1234,

            play: false,
            first: true,
        }
    }
}

impl eframe::App for PollinationViewer {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.first {
            self.first = false;
            self.reset();
        }
        self.run_simulation(ctx, frame);
        self.draw_header(ctx, frame);
        self.draw_settings(ctx, frame);
        self.draw_overview(ctx, frame);
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
        if !self.play {
            return;
        }

        let time = ctx.input(|i| i.time);
        if self.last_step_time < (time - self.simulation_speed) {
            self.last_step_time = time;

            for _ in 0..self.per_step {
                let mut simulation = std::mem::take(&mut self.simulation);
                let res = std::panic::catch_unwind(|| {
                    let res = simulation.step(&SimConfig {
                        timeout_propagativity: 10,
                        timeout_heartbeat: 10,
                        timeout_reap: 10,
                    });
                    (res, simulation)
                });
                match res {
                    Ok((res, simulation)) => {
                        self.simulation = simulation;
                    }
                    Err(err) => {
                        println!("Caught a panic; restarting sim");

                        self.reset();
                    }
                }

                // TODO: Do something with res, log it??
            }
            //println!("RES: {res:?}");
            //self.event_log.push(res)
        }

        // TODO: More efficient way to do this?
        ctx.request_repaint();
    }

    fn draw_settings(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Settings").show(ctx, |ui| {
            ui.label(format!("Simulation Time: {:#?}", self.simulation.time));
            if let Some(converge_time) = self.simulation.converge_time() {
                ui.label(format!("Converged at: {converge_time}"));
            }

            if ui.button("Reset").clicked() {
                self.reset();
            }

            ui.horizontal(|ui| {
                let word = if self.play { "Pause" } else { "Play" };
                if ui.button(word).clicked() {
                    self.play = !self.play;
                }

                if ui.button("Step").clicked() {
                    self.play = true;
                    self.run_simulation(ctx, _frame);
                    self.play = false;
                }
            });

            ui.add(
                egui::Slider::new(&mut self.simulation_speed, 0.001..=10.).text("Simulation speed"),
            );
            ui.add(egui::Slider::new(&mut self.per_step, 0..=100).text("Iterations per step"));
            ui.add(egui::Slider::new(&mut self.node_count, 1..=100).text("Node count"));
            ui.add(egui::Slider::new(&mut self.connection_count, 0..=100).text("Connection count"));
            if self.connection_count == 0 {
                ui.add(
                    egui::Slider::new(&mut self.rand_robin_count, 0..=100)
                        .text("Rand robing count"),
                );
            }

            let mut s = self.seed.to_string();
            if ui.text_edit_singleline(&mut s).changed() {
                if let Ok(seed) = s.parse::<u64>() {
                    self.seed = seed;
                }
            }

            ui.add(ForceGraphSettingsWidget::new(
                &mut self.graph,
                &mut self.config,
            ));
        });
    }

    fn draw_event_log(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Event Log").show(ctx, |ui| {});
    }

    fn draw_overview(&self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Overview").show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink(true).show(ui, |ui| {
                for (idx, node) in self.simulation.nodes.node_weights().enumerate() {
                    ui.label(format!("Node: {}", idx));
                    ui.label(format!("Uuid: {}", node.inner.uuid()));
                    ui.label(format!("\tReality Token: {}", node.inner.reality_token()));
                    if let Some(id) = node.inner.id() {
                        ui.label(format!("\tid: {id}"));
                    }
                    ui.label(format!("\tTimestamp: {}", node.inner.timestamp()));
                }
            })
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
        ui.label(format!("Seconds since start: {:#?}", &ui.input(|i| i.time)));
    }

    fn reset(&mut self) {
        self.play = false;
        self.simulation = Simulation::new(
            self.node_count,
            self.seed,
            self.connection_count,
            self.rand_robin_count,
        );
        self.graph = ForceGraph::from_graph(self.simulation.graph());
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
