mod tab_simulator;

pub use tab_simulator::SimulatorApp;

pub trait SubApp {
    fn name(&self) -> &'static str;
    fn logic(&mut self, _ctx: &egui::Context) {}
    fn ui(&mut self, ui: &mut egui::Ui);
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}
