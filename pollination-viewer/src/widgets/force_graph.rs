use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Shape, Ui, Widget, vec2};
use petgraph::{graph::NodeIndex, stable_graph::StableGraph as Graph};
use std::collections::HashSet;

mod config;
mod graph;

pub use config::ForceGraphConfig;
pub use graph::ForceGraph;

pub struct ForceGraphState {
    graph: ForceGraph,
    config: ForceGraphConfig,
    open_node_windows: HashSet<NodeIndex>,
}

impl ForceGraphState {
    pub fn new<T>(graph: &Graph<T, ()>) -> Self {
        Self {
            graph: ForceGraph::from_graph(graph),
            config: ForceGraphConfig::default(),
            open_node_windows: HashSet::new(),
        }
    }
}

pub struct ForceGraphWidget<'a> {
    state: &'a mut ForceGraphState,
    node_color_provider: Option<&'a dyn Fn(u32) -> (Color32, Color32)>,
    edge_color_provider: Option<&'a dyn Fn(u32, u32) -> Color32>,
    info_provider: Option<&'a dyn Fn(NodeIndex, &mut egui::Ui)>,
}

impl Widget for ForceGraphWidget<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let response = ui.allocate_response(ui.available_size(), Sense::hover());
        let painter = ui.painter().with_clip_rect(ui.clip_rect());

        // TODO: Move simulation to a "logic" step
        let (pos_map, fixed) = self.position_map(ui, &response);
        self.state
            .graph
            .run_force_simulation(&self.state.config, &fixed);

        self.draw_graph(ui, &painter, &response, &pos_map);
        self.draw_open_node_windows(ui);
        response
    }
}

impl<'a> ForceGraphWidget<'a> {
    pub fn new(state: &'a mut ForceGraphState) -> ForceGraphWidget<'a> {
        Self {
            state,
            node_color_provider: None,
            edge_color_provider: None,
            info_provider: None,
        }
    }

    pub fn with_node_color_provider(
        mut self,
        node_colors: &'a dyn Fn(u32) -> (Color32, Color32),
    ) -> Self {
        self.node_color_provider = Some(node_colors);
        self
    }

    pub fn with_edge_color_provider(
        mut self,
        edge_colors: &'a dyn Fn(u32, u32) -> Color32,
    ) -> Self {
        self.edge_color_provider = Some(edge_colors);
        self
    }

    pub fn with_node_info_provider(
        mut self,
        info_provider: &'a dyn Fn(NodeIndex, &mut egui::Ui),
    ) -> Self {
        self.info_provider = Some(info_provider);
        self
    }

    fn position_map(&mut self, ui: &mut Ui, response: &Response) -> (Vec<Pos2>, Vec<usize>) {
        let mut out = vec![];
        let mut fixed = vec![];

        let mut interact = false;
        for (idx, node) in self.state.graph.inner_mut().node_weights_mut().enumerate() {
            let point_rect = Rect::from_center_size(node.pos, vec2(20., 20.));
            let point_id = response.id.with(idx);
            let point_response = ui.interact(point_rect, point_id, Sense::drag());
            node.pos += point_response.drag_delta();

            let pos = if point_response.dragged() {
                fixed.push(idx);
                self.state.open_node_windows.insert(NodeIndex::new(idx));
                interact = true;
                node.pos
            } else {
                if point_response.drag_stopped() {
                    interact = true;
                    //ui.ctx().clear_animations();
                }
                node.pos
            };

            out.push(pos)
        }

        self.state.graph.state.interact = interact;

        (out, fixed)
    }

    fn draw_graph(&self, _ui: &mut Ui, painter: &Painter, _response: &Response, pos_map: &[Pos2]) {
        for node in self.state.graph.inner().node_weights() {
            for neighbor in self.state.graph.inner().neighbors((node.id as u32).into()) {
                let neighbor = self.state.graph.inner().node_weight(neighbor).unwrap();

                let color = if let Some(edge_color_fn) = &self.edge_color_provider {
                    edge_color_fn(node.id as u32, neighbor.id as u32)
                } else {
                    self.state.config.edge_color
                };
                painter.add(Shape::line_segment(
                    [pos_map[node.id], pos_map[neighbor.id]],
                    (3., color),
                ));
            }
        }

        for (idx, _node) in self.state.graph.inner().node_weights().enumerate() {
            let (ring_color, node_color) = if let Some(color_fn) = &self.node_color_provider {
                color_fn(idx as u32)
            } else {
                (self.state.config.ring_color, self.state.config.node_color)
            };

            painter.add(Shape::circle_filled(pos_map[idx], 15., ring_color));
            painter.add(Shape::circle_filled(pos_map[idx], 13., node_color));
            painter.text(
                pos_map[idx],
                egui::Align2::CENTER_CENTER,
                format!("{idx}"),
                egui::FontId::proportional(20.0),
                egui::Color32::WHITE,
            );
        }
    }

    fn draw_open_node_windows(&mut self, ui: &egui::Ui) {
        let Some(info_provider) = self.info_provider else {
            return;
        };

        let mut remove_id = None;
        for node_idx in self.state.open_node_windows.iter() {
            egui::Window::new(format!("Node {}", node_idx.index())).show(ui, |ui| {
                if ui.button("Close").clicked() {
                    remove_id = Some(*node_idx)
                }

                info_provider(*node_idx, ui);
            });
        }

        if let Some(remove_id) = remove_id {
            self.state.open_node_windows.remove(&remove_id);
        }
    }
}

pub struct ForceGraphSettingsWidget<'a> {
    config: &'a mut ForceGraphConfig,
}

impl<'a> ForceGraphSettingsWidget<'a> {
    pub fn new(config: &'a mut ForceGraphConfig) -> ForceGraphSettingsWidget<'a> {
        Self { config }
    }
}

impl Widget for ForceGraphSettingsWidget<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.checkbox(&mut self.config.velocity_decay_enabled, "velocity_decay");
        if self.config.velocity_decay_enabled {
            ui.add(egui::Slider::new(
                &mut self.config.velocity_decay,
                0.0..=100.0,
            ));
        }

        ui.checkbox(&mut self.config.link_distance_enabled, "link_distance");
        if self.config.link_distance_enabled {
            ui.add(egui::Slider::new(
                &mut self.config.link_distance,
                0.0..=1000.0,
            ));
        }

        ui.checkbox(&mut self.config.link_strength_enabled, "link_strength");
        if self.config.link_strength_enabled {
            ui.add(egui::Slider::new(
                &mut self.config.link_strength,
                0.0..=1000.0,
            ));
        }

        ui.add(
            egui::Slider::new(&mut self.config.centering_strength, 0.0..=5.0)
                .text("Centering strength"),
        );

        ui.color_edit_button_srgba(&mut self.config.ring_color);
        ui.color_edit_button_srgba(&mut self.config.node_color);
        ui.color_edit_button_srgba(&mut self.config.edge_color)
    }
}
