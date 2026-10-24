use crate::sub_app::{SimulatorApp, SubApp};

const ACTIVE_TAB_KEY: &str = "active_tab";

#[derive(
    Clone, Copy, PartialEq, Eq, Hash, Debug, Default, serde::Serialize, serde::Deserialize,
)]
enum Tab {
    #[default]
    Simulator,
}

impl Tab {
    const ALL: [Self; 1] = [Self::Simulator];
}

pub struct PollinationViewer {
    tab: Tab,
    simulator: SimulatorApp,
}

impl PollinationViewer {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            tab: cc
                .storage
                .and_then(|storage| eframe::get_value(storage, ACTIVE_TAB_KEY))
                .unwrap_or_default(),
            simulator: SimulatorApp::new(cc.storage),
        }
    }

    fn active_mut(&mut self) -> &mut dyn SubApp {
        match self.tab {
            Tab::Simulator => &mut self.simulator,
        }
    }

    fn all_mut(&mut self) -> [&mut dyn SubApp; 1] {
        [&mut self.simulator]
    }

    fn name_of(&self, tab: Tab) -> &'static str {
        match tab {
            Tab::Simulator => self.simulator.name(),
        }
    }

    fn draw_nav_bar(&mut self, ui: &mut egui::Ui) {
        let tabs = Tab::ALL.map(|tab| (tab, self.name_of(tab)));

        egui::Panel::top("nav_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // No File->Quit on web pages
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("🔧", |ui| {
                        if ui.button("Quit").clicked() {
                            ui.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        egui::widgets::global_theme_preference_buttons(ui);
                    });

                    ui.separator();
                }

                for (tab, name) in tabs {
                    ui.selectable_value(&mut self.tab, tab, name);
                }
            });
        });
    }
}

impl eframe::App for PollinationViewer {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, ACTIVE_TAB_KEY, &self.tab);
        for app in self.all_mut() {
            app.save(storage);
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        for app in self.all_mut() {
            app.logic(ctx);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw_nav_bar(ui);

        let tab = self.tab;
        ui.push_id(tab, |ui| self.active_mut().ui(ui));
    }
}
