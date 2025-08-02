use egui::{Color32, Frame, Pos2, Rect, Scene, Sense, Shape, Stroke, Vec2, emath, vec2};
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
}

impl Default for PollinationViewer {
    fn default() -> Self {
        Self {
            label: "Hello World!".to_owned(),
            value: 2.7,
            point: Pos2::new(50., 100.),
            scene: Rect::ZERO,
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
                    ui.allocate_space(ui.available_size());
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::SidePanel::left("Side panel").show(ctx, |ui| {
            ui.label("Hello world");
            ui.allocate_space(ui.available_size());
        });

        egui::SidePanel::right("Right panel").show(ctx, |ui| {
            ui.label("Settings");
            ui.allocate_space(ui.available_size());
        });

        let frame = egui::containers::Frame::new()
            .inner_margin(egui::Margin::ZERO)
            .outer_margin(egui::Margin::ZERO);

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.label(format!("Scene rect: {:#?}", &mut self.scene));
            Scene::new()
                .max_inner_size([350.0, 1000.0])
                .zoom_range(0.1..=2.0)
                .show(ui, &mut self.scene, |ui| {
                    let response = ui.allocate_response(ui.available_size(), Sense::hover());
                    let painter = ui.painter().with_clip_rect(ui.clip_rect());

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
                });
        });
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
