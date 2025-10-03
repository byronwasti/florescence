use crate::fruchterman_reingold as fr;
use egui::{Color32, Frame, Pos2, Rect, Scene, Sense, Shape, Stroke, Vec2, emath, pos2, vec2};
use fdg::{
    Force, ForceGraph,
    fruchterman_reingold::{FruchtermanReingold, FruchtermanReingoldConfiguration},
    nalgebra::Rotation2,
    petgraph::Graph,
    simple::Center,
};
use std::f32::consts::TAU;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct PollinationViewer {
    label: String,

    #[serde(skip)]
    value: f32,

    #[serde(skip)]
    point: Pos2,

    #[serde(skip)]
    scene: Rect,

    #[serde(skip)]
    graph: fr::NodeGraph,

    #[serde(skip)]
    config: fr::Config,

    #[serde(skip)]
    time: f32,

    #[serde(skip)]
    applied: f32,

    node_color: egui::Color32,
    edge_color: egui::Color32,
    border_color: egui::Color32,

    cooling: bool,
    cool_factor: f32,

    set_temp: f32,
}

impl Default for PollinationViewer {
    fn default() -> Self {
        /*
        let mut graph = Graph::<&str, ()>::new();
        let pg = graph.add_node("petgraph");
        let fb = graph.add_node("fixedbitset");
        let qc = graph.add_node("quickcheck");
        let rand = graph.add_node("rand");
        let libc = graph.add_node("libc");
        let gobo = graph.add_node("gobo");
        let sobo = graph.add_node("sobo");
        let lobo = graph.add_node("lobo");
        graph.extend_with_edges(&[(pg, fb), (pg, qc), (qc, rand), (rand, libc), (qc, libc)]);
        graph.extend_with_edges(&[(gobo, sobo), (gobo, lobo)]);
        let graph: ForceGraph<f32, 2, &str, ()> = fdg::init_force_graph_uniform(graph, 200.0);
        */

        let graph = fr::graph();
        let config = fr::Config {
            area: (10000., 10000.),
            c: 0.03,
            temp: 25.,
        };

        Self {
            label: "Hello World!".to_owned(),
            value: 2.7,
            point: Pos2::new(50., 100.),
            scene: Rect::ZERO,
            graph,
            set_temp: config.temp,
            config,
            time: 0.,
            applied: 0.,
            node_color: egui::Color32::LIGHT_BLUE,
            edge_color: egui::Color32::LIGHT_RED,
            border_color: egui::Color32::WHITE,
            cooling: false,
            cool_factor: 1.1,
        }
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

    pub fn reset_temp(&mut self) {
        self.config.temp = self.set_temp;
    }
}

impl eframe::App for PollinationViewer {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // NOTE: no File->Quit on web pages!
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

        egui::Window::new("Settings").show(ctx, |ui| {
            if ui.button("Step").clicked() {
                self.time += 1.0;
                if self.cooling {
                    self.config.temp = self.config.temp / self.cool_factor;
                }
            }

            ui.add(egui::Slider::new(&mut self.config.c, 0.0..=1.0).text("c"));
            ui.add(egui::Slider::new(&mut self.set_temp, 0.0..=10_000.0).text(format!("temp ({})", self.config.temp)));

            if ui.button("Reset").clicked() {
                self.graph = fr::graph();
                self.config.temp = self.set_temp;
            }

            ui.color_edit_button_srgba(&mut self.node_color);
            ui.color_edit_button_srgba(&mut self.edge_color);
            ui.color_edit_button_srgba(&mut self.border_color);

            ui.checkbox(&mut self.cooling, "Cooling");
            if self.cooling {
                ui.add(egui::Slider::new(&mut self.cool_factor, 0.0..=5.0).text("cool factor"));
            }
        });

        /*
        egui::SidePanel::left("Side panel").show(ctx, |ui| {
            ui.label("Hello world");
            if ui.button("Step").clicked() {
                self.time += 1.0;
            }
            ui.allocate_space(ui.available_size());
        });

        egui::SidePanel::right("Right panel").show(ctx, |ui| {
            ui.label("Settings");
            ui.allocate_space(ui.available_size());
        });
        */

        let frame = egui::containers::Frame::new()
            .inner_margin(egui::Margin::ZERO)
            .outer_margin(egui::Margin::ZERO);

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {

            let id = egui::Id::new("animation");
            if self.config.temp > 1. {
                //self.config.temp = ui.ctx().animate_bool(id, false) * self.set_temp;
                ui.ctx().request_repaint();
                fr::fruchterman_reingold(&mut self.graph, &self.config);
            } else {
                _ = ui.ctx().animate_bool(id, true);
            }

            /*
            if self.time > self.applied {
                //ui.ctx().request_repaint();
                self.applied = self.time;
                fr::fruchterman_reingold(&mut self.graph, &self.config);
            }
            */

            ui.label(format!("Scene rect: {:#?}", &mut self.scene));
            ui.label(format!(
                "k: {:#?}",
                &mut self.config.k(self.graph.node_count() as f32)
            ));

            Scene::new()
                .max_inner_size([350.0, 1000.0])
                .zoom_range(0.1..=10.0)
                .show(ui, &mut self.scene, |ui| {
                    let response = ui.allocate_response(ui.available_size(), Sense::hover());
                    let painter = ui.painter().with_clip_rect(ui.clip_rect());

                    // Draw the Border
                    painter.add(Shape::rect_stroke(Rect::from_two_pos(
                        pos2(-self.config.area.0 as f32 / 2., -self.config.area.1 as f32 / 2.),
                        pos2(self.config.area.0 as f32 / 2., self.config.area.1 as f32 / 2.),
                    ), 0., (3., self.border_color), egui::StrokeKind::Outside));

                    /* Random Garbage
                    painter.add(Shape::circle_filled(
                        Pos2::new(0., 0.),
                        50.,
                        Color32::DARK_GREEN,
                    ));

                    let point_rect = Rect::from_center_size(self.point, vec2(100., 100.));
                    let point_id = response.id.with(0);
                    let point_response = ui.interact(point_rect, point_id, Sense::drag());
                    self.point += point_response.drag_delta();

                    painter.add(Shape::circle_filled(self.point, 50., Color32::DARK_RED));
                    */

                    for (i, node) in self.graph.node_weights().enumerate() {
                        for (j, neighbor) in self.graph.neighbors((i as u32).into()).enumerate() {
                            let neighbor = self.graph.node_weight(neighbor).unwrap();

                            /*
                            ui.ctx().animate_value_with_time(egui::Id::new(format!("edge-{}.{}", i, j)),
                                
                                )
                            */

                            painter.add(Shape::line_segment(
                                [
                                    animate_pos(
                                        ui,
                                        egui::Id::new(format!("edge-a-{i}-{j}-x")),
                                        egui::Id::new(format!("edge-a-{i}-{j}-y")),
                                        node.pos,
                                        ui.input(|i| i.time as f32),
                                    ),
                                    animate_pos(
                                        ui,
                                        egui::Id::new(format!("edge-b-{i}-{j}-x")),
                                        egui::Id::new(format!("edge-b-{i}-{j}-y")),
                                        neighbor.pos,
                                        ui.input(|i| i.time as f32),
                                    )
                                ],
                                (3., self.edge_color),
                            ));
                        }
                    }
                    for (idx, node) in self.graph.node_weights_mut().enumerate() {
                        let point_rect = Rect::from_center_size(node.pos, vec2(20., 20.));
                        let point_id = response.id.with(idx);
                        let point_response = ui.interact(point_rect, point_id, Sense::drag());
                        node.pos += point_response.drag_delta();
                        painter.add(Shape::circle_filled(
                            animate_pos(
                                ui,
                                egui::Id::new(format!("node-{idx}-x")),
                                egui::Id::new(format!("node-{idx}-y")),
                                node.pos,
                                ui.input(|i| i.time as f32),
                            ),
                            10.,
                            self.node_color,
                        ));
                    }
                });
        });
    }
}

fn animate_pos(ui: &mut egui::Ui, id0: egui::Id, id1: egui::Id, pos: Pos2, time: f32) -> Pos2 {
    let x = ui.ctx().animate_value_with_time(
        id0,
        pos.x,
        time,
    );
    let y = ui.ctx().animate_value_with_time(
        id1,
        pos.y,
        time,
    );

    pos2(x, y)
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
