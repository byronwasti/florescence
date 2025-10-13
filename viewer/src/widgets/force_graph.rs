use egui::{
    Color32, Frame, Painter, Pos2, Rect, Response, Scene, Sense, Shape, Stroke, Ui, Vec2, Widget,
    emath, pos2, vec2,
};

mod config;
mod graph;

pub use config::ForceGraphConfig;
pub use graph::ForceGraph;

pub struct ForceGraphWidget<'a> {
    graph: &'a mut ForceGraph,
    config: &'a mut ForceGraphConfig,
}

impl Widget for ForceGraphWidget<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let response = ui.allocate_response(ui.available_size(), Sense::hover());
        let painter = ui.painter().with_clip_rect(ui.clip_rect());

        let (pos_map, fixed) = self.position_map(ui, &response);
        self.graph.run_force_simulation(self.config, &fixed);
        //self.ui.ctx().request_repaint();

        self.draw_graph(ui, &painter, &response, &pos_map);
        response
    }
}

impl<'a> ForceGraphWidget<'a> {
    pub fn new(
        graph: &'a mut ForceGraph,
        config: &'a mut ForceGraphConfig,
    ) -> ForceGraphWidget<'a> {
        Self { graph, config }
    }

    fn position_map(&mut self, ui: &mut Ui, response: &Response) -> (Vec<Pos2>, Vec<usize>) {
        let mut out = vec![];
        let mut fixed = vec![];

        let mut interact = false;
        for (idx, node) in self.graph.inner_mut().node_weights_mut().enumerate() {
            let point_rect = Rect::from_center_size(node.pos, vec2(20., 20.));
            let point_id = response.id.with(idx);
            let point_response = ui.interact(point_rect, point_id, Sense::drag());
            node.pos += point_response.drag_delta();

            let pos = if point_response.dragged() {
                fixed.push(idx);
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

        self.graph.state.interact = interact;

        (out, fixed)
    }

    fn draw_graph(&self, ui: &mut Ui, painter: &Painter, response: &Response, pos_map: &[Pos2]) {
        for node in self.graph.inner().node_weights() {
            for neighbor in self.graph.inner().neighbors((node.id as u32).into()) {
                let neighbor = self.graph.inner().node_weight(neighbor).unwrap();

                painter.add(Shape::line_segment(
                    [pos_map[node.id], pos_map[neighbor.id]],
                    (3., self.config.edge_color),
                ));
            }
        }

        for (idx, node) in self.graph.inner().node_weights().enumerate() {
            painter.add(Shape::circle_filled(
                pos_map[idx],
                15.,
                self.config.ring_color,
            ));
            painter.add(Shape::circle_filled(
                pos_map[idx],
                10.,
                self.config.node_color,
            ));
        }
    }
}

pub struct ForceGraphSettingsWidget<'a> {
    graph: &'a mut ForceGraph,
    config: &'a mut ForceGraphConfig,
}

impl<'a> ForceGraphSettingsWidget<'a> {
    pub fn new(
        graph: &'a mut ForceGraph,
        config: &'a mut ForceGraphConfig,
    ) -> ForceGraphSettingsWidget<'a> {
        Self { graph, config }
    }
}

impl Widget for ForceGraphSettingsWidget<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        if ui.button("Reset").clicked() {
            *self.graph = ForceGraph::random();
        }

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

        ui.color_edit_button_srgba(&mut self.config.ring_color);
        ui.color_edit_button_srgba(&mut self.config.node_color);
        ui.color_edit_button_srgba(&mut self.config.edge_color)
    }
}
