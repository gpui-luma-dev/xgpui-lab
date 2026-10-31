use std::f32::consts::{FRAC_PI_2, TAU};

use gpui::Hsla;

use crate::style::{Size, active_color_control_theme};

pub mod sizing {
    pub const RING_THICKNESS_XSMALL: f32 = 7.0;
    pub const RING_THICKNESS_SMALL: f32 = 14.0;
    pub const RING_THICKNESS_MEDIUM: f32 = 20.0;
    pub const RING_THICKNESS_LARGE: f32 = 28.0;

    pub const THUMB_SIZE_SMALL: f32 = 12.0;
    pub const THUMB_SIZE_MEDIUM: f32 = 16.0;
    pub const THUMB_SIZE_LARGE: f32 = 20.0;
}
use super::common::size_px;

#[derive(Clone, Debug, PartialEq)]
pub struct ColorRingTrackContext {
    pub size: Size,
    pub ring_thickness: Option<f32>,
    pub ring_thickness_size: Option<Size>,
    pub thumb_size: Option<f32>,
    pub reversed: bool,
    pub range: std::ops::Range<f32>,
    pub rotation_degrees: f32,
    pub allow_inner_target: bool,
    pub ring_inner_border: bool,
    pub ring_outer_border: bool,
    pub ring_border_color: Hsla,
}

impl Default for ColorRingTrackContext {
    fn default() -> Self {
        let theme = active_color_control_theme();
        Self {
            size: Size::Medium,
            ring_thickness: None,
            ring_thickness_size: None,
            thumb_size: None,
            reversed: false,
            range: 0.0..1.0,
            rotation_degrees: 0.0,
            allow_inner_target: false,
            ring_inner_border: true,
            ring_outer_border: true,
            ring_border_color: theme.border,
        }
    }
}

impl ColorRingTrackContext {
    pub fn size_px(&self) -> f32 {
        size_px(self.size)
    }

    pub fn ring_thickness_px(&self) -> f32 {
        if let Some(thickness) = self.ring_thickness {
            return thickness.max(1.0);
        }

        let thickness_size = self.ring_thickness_size.unwrap_or(self.size);
        match thickness_size {
            Size::XSmall => sizing::RING_THICKNESS_XSMALL,
            Size::Small => sizing::RING_THICKNESS_SMALL,
            Size::Medium => sizing::RING_THICKNESS_MEDIUM,
            Size::Large => sizing::RING_THICKNESS_LARGE,
            Size::Size(px) => (px.as_f32() * 0.1).max(8.0),
        }
    }

    pub fn thumb_size_px(&self) -> f32 {
        self.thumb_size.unwrap_or(match self.size {
            Size::XSmall | Size::Small => sizing::THUMB_SIZE_SMALL,
            Size::Medium => sizing::THUMB_SIZE_MEDIUM,
            Size::Large => sizing::THUMB_SIZE_LARGE,
            Size::Size(px) => (px.as_f32() * 0.08).max(10.0),
        })
    }

    pub fn rotation_turns(&self) -> f32 {
        self.rotation_degrees.rem_euclid(360.0) / 360.0
    }

    pub fn angle_range(&self) -> (f32, f32) {
        let start = self.rotation_turns() * TAU - FRAC_PI_2;
        (start, start + TAU)
    }
}
