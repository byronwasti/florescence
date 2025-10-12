use egui::{
    Color32, Frame, Painter, Pos2, Rect, Response, Scene, Sense, Shape, Stroke, Ui, Vec2, Widget,
    emath, pos2, vec2,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ForceGraphConfig {
    pub velocity_decay_enabled: bool,
    pub velocity_decay: f64,

    pub link_distance_enabled: bool,
    pub link_distance: f64,

    pub link_strength_enabled: bool,
    pub link_strength: f64,

    pub ring_color: Color32,
    pub node_color: Color32,
    pub edge_color: Color32,
}

impl Default for ForceGraphConfig {
    fn default() -> Self {
        Self {
            velocity_decay_enabled: false,
            velocity_decay: 0.,
            link_distance_enabled: false,
            link_distance: 0.,
            link_strength_enabled: false,
            link_strength: 0.,

            ring_color: Color32::LIGHT_GRAY,
            node_color: Color32::LIGHT_BLUE,
            edge_color: Color32::LIGHT_RED,
        }
    }
}
