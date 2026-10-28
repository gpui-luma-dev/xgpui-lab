use gpui::Hsla;

use super::super::color::interpolate_rgb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientType {
    Linear,
    Radial,
    Angular,
}

impl GradientType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Radial => "radial",
            Self::Angular => "angular",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewRenderer {
    Quads,
    RenderImageSync,
    RenderImageAsync,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshPoint {
    pub row: u8,
    pub col: u8,
    pub u: f32,
    pub v: f32,
    pub color: Hsla,
}

pub fn color_at_position(stops: &[(f32, Hsla)], position: f32) -> Hsla {
    if stops.is_empty() {
        return gpui::hsla(0.0, 0.0, 0.0, 1.0);
    }
    if stops.len() == 1 {
        return stops[0].1;
    }

    let position = position.clamp(0.0, 1.0);
    if position <= stops[0].0 {
        return stops[0].1;
    }
    if position >= stops[stops.len() - 1].0 {
        return stops[stops.len() - 1].1;
    }

    for window in stops.windows(2) {
        let (left_pos, left_color) = window[0];
        let (right_pos, right_color) = window[1];
        if position >= left_pos && position <= right_pos {
            let span = (right_pos - left_pos).max(f32::EPSILON);
            let t = (position - left_pos) / span;
            return interpolate_rgb(left_color, right_color, t);
        }
    }

    stops[stops.len() - 1].1
}

pub fn sorted_stops(stops: &[(f32, Hsla)]) -> Vec<(f32, Hsla)> {
    let mut sorted = stops.to_vec();
    sorted.sort_by(|left, right| left.0.total_cmp(&right.0));
    sorted
}
