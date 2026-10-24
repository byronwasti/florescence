mod simulator;

pub use simulator::SimulatorApp;

pub trait SubApp {
    fn name(&self) -> &'static str;
    fn logic(&mut self, _ctx: &egui::Context) {}
    fn ui(&mut self, ui: &mut egui::Ui);
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}
