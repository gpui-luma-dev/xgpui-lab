use std::f32::consts::TAU;

use gpui::Hsla;

use crate::style::{Size, active_color_control_theme};

use super::common::{size_px, start_turns, sweep_turns};

pub mod sizing {
    pub const ARC_THICKNESS_XSMALL: f32 = 7.0;
    pub const ARC_THICKNESS_SMALL: f32 = 14.0;
    pub const ARC_THICKNESS_MEDIUM: f32 = 20.0;
    pub const ARC_THICKNESS_LARGE: f32 = 28.0;

    pub const THUMB_SIZE_SMALL: f32 = 12.0;
    pub const THUMB_SIZE_MEDIUM: f32 = 16.0;
    pub const THUMB_SIZE_LARGE: f32 = 20.0;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColorArcTrackContext {
    pub start_degrees: f32,
    pub sweep_degrees: f32,
    pub arc_thickness: Option<f32>,
    pub arc_thickness_size: Option<Size>,
    pub size: Size,
    pub reversed: bool,
    pub border_color: Hsla,
    pub range: std::ops::Range<f32>,
}

impl Default for ColorArcTrackContext {
    fn default() -> Self {
        let theme = active_color_control_theme();
        Self {
            start_degrees: 0.0,
            sweep_degrees: 180.0,
            arc_thickness: None,
            arc_thickness_size: None,
            size: Size::Medium,
            reversed: false,
            border_color: theme.border,
            range: 0.0..1.0,
        }
    }
}

impl ColorArcTrackContext {
    pub fn start_turns(&self) -> f32 {
        start_turns(self.start_degrees)
    }

    pub fn sweep_turns(&self) -> f32 {
        sweep_turns(self.sweep_degrees)
    }

    pub fn dial_size_px(&self) -> f32 {
        size_px(self.size)
    }

    pub fn arc_thickness_px(&self) -> f32 {
        if let Some(thickness) = self.arc_thickness {
            return thickness.max(1.0);
        }

        let thickness_size = self.arc_thickness_size.unwrap_or(self.size);
        match thickness_size {
            Size::XSmall => sizing::ARC_THICKNESS_XSMALL,
            Size::Small => sizing::ARC_THICKNESS_SMALL,
            Size::Medium => sizing::ARC_THICKNESS_MEDIUM,
            Size::Large => sizing::ARC_THICKNESS_LARGE,
            Size::Size(px) => (px.as_f32() * 0.1).max(8.0),
        }
    }

    pub fn thumb_size_px(&self, thumb_size: Option<f32>) -> f32 {
        thumb_size.unwrap_or(match self.size {
            Size::XSmall | Size::Small => sizing::THUMB_SIZE_SMALL,
            Size::Medium => sizing::THUMB_SIZE_MEDIUM,
            Size::Large => sizing::THUMB_SIZE_LARGE,
            Size::Size(px) => (px.as_f32() * 0.08).max(10.0),
        })
    }
}

pub fn arc_angle_range(start_degrees: f32, sweep_degrees: f32) -> (f32, f32) {
    let start_turn = start_degrees / 360.0;
    let end_turn = start_turn + sweep_degrees / 360.0;
    (turn_to_angle(start_turn), turn_to_angle(end_turn))
}

pub fn turn_to_angle(turn: f32) -> f32 {
    (turn - 0.25) * TAU
}
