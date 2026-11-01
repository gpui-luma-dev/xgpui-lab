use super::common::{mirrored_lightness, mirrored_saturation};
use super::track_context::ColorRingTrackContext;
use super::types::ColorRingTrackDelegate;
use crate::color_slider::color_spec::Hsv;
use crate::shape::{Arc, ArcData};
use crate::style::Size;
use gpui::*;
use std::f32::consts::TAU;

fn normalize_hue_degrees(hue: f32) -> f32 {
    // Delegates receive hue in degrees (see ColorRingState::hue max=360 and callers).
    // Treat small degree values near 0 the same as any other degree input.
    hue.rem_euclid(360.0)
}

fn segments_for_size(size: Size) -> usize {
    match size {
        Size::XSmall | Size::Small => 180,
        Size::Medium => 360,
        Size::Large => 720,
        Size::Size(px) => {
            if px.as_f32() < 220.0 {
                180
            } else if px.as_f32() < 280.0 {
                360
            } else {
                720
            }
        }
    }
}

fn paint_segmented_ring(
    bounds: Bounds<Pixels>,
    window: &mut Window,
    ring_thickness: f32,
    rotation_turns: f32,
    segments: usize,
    mut color_at_start: impl FnMut(f32) -> Hsla,
) {
    let radius = bounds.size.width.min(bounds.size.height).as_f32() / 2.0;
    let inner_radius = (radius - ring_thickness).max(0.0);
    let arc = Arc::new().inner_radius(inner_radius).outer_radius(radius);
    let step = TAU / segments as f32;
    let overlap = step * 0.35;

    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let color = color_at_start(t0);

        arc.paint(
            &ArcData {
                data: &(),
                index: i,
                value: 1.0,
                start_angle: (t0 + rotation_turns) * TAU - overlap,
                end_angle: (t1 + rotation_turns) * TAU + overlap,
                pad_angle: 0.0,
            },
            color,
            None,
            None,
            &bounds,
            window,
        );
    }
}

#[allow(dead_code)] // Kept as an alternate delegate example.
pub struct SaturationRingDelegate {
    pub hue: f32,
    pub hsv_value: f32,
}

impl ColorRingTrackDelegate for SaturationRingDelegate {
    fn paint_domain_track(&self, context: &ColorRingTrackContext, bounds: Bounds<Pixels>, window: &mut Window) {
        let hue_deg = normalize_hue_degrees(self.hue);
        let segments = segments_for_size(context.size);
        let ring_thickness = context.ring_thickness_px();
        let rotation_turns = context.rotation_turns();
        let hsv_value = self.hsv_value.clamp(0.0, 1.0);

        paint_segmented_ring(bounds, window, ring_thickness, rotation_turns, segments, |t0| {
            let sat = mirrored_saturation(t0);
            Hsv { h: hue_deg, s: sat, v: hsv_value, a: 1.0 }.to_hsla_ext()
        });
    }

    fn get_color_for_context(&self, _context: &ColorRingTrackContext, position: f32) -> Hsla {
        Hsv {
            h: normalize_hue_degrees(self.hue),
            s: mirrored_saturation(position),
            v: self.hsv_value.clamp(0.0, 1.0),
            a: 1.0,
        }
        .to_hsla_ext()
    }
}

pub struct LightnessRingDelegate {
    pub hue: f32,
    pub saturation: f32,
}

impl ColorRingTrackDelegate for LightnessRingDelegate {
    fn paint_domain_track(&self, context: &ColorRingTrackContext, bounds: Bounds<Pixels>, window: &mut Window) {
        let hue = normalize_hue_degrees(self.hue) / 360.0;
        let saturation = self.saturation.clamp(0.0, 1.0);
        let segments = segments_for_size(context.size);
        let ring_thickness = context.ring_thickness_px();
        let rotation_turns = context.rotation_turns();

        paint_segmented_ring(bounds, window, ring_thickness, rotation_turns, segments, |t0| {
            hsla(hue, saturation, mirrored_lightness(t0), 1.0)
        });
    }

    fn get_color_for_context(&self, _context: &ColorRingTrackContext, position: f32) -> Hsla {
        hsla(
            normalize_hue_degrees(self.hue) / 360.0,
            self.saturation.clamp(0.0, 1.0),
            mirrored_lightness(position),
            1.0,
        )
    }
}

pub struct HueRingDelegate {
    pub saturation: f32,
    pub lightness: f32,
}

impl ColorRingTrackDelegate for HueRingDelegate {
    fn paint_domain_track(&self, context: &ColorRingTrackContext, bounds: Bounds<Pixels>, window: &mut Window) {
        let saturation = self.saturation;
        let lightness = self.lightness;
        let segments = segments_for_size(context.size);
        let ring_thickness = context.ring_thickness_px();
        let rotation_turns = context.rotation_turns();

        paint_segmented_ring(bounds, window, ring_thickness, rotation_turns, segments, |t0| {
            let t1 = t0 + (1.0 / segments as f32);
            let t_mid = (t0 + t1) * 0.5;
            hsla(t_mid, saturation, lightness, 1.0)
        });
    }

    fn get_color_for_context(&self, _context: &ColorRingTrackContext, position: f32) -> Hsla {
        let hue = position.rem_euclid(1.0);
        hsla(hue, self.saturation, self.lightness, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_hue_degrees;

    #[::core::prelude::v1::test]
    fn normalize_hue_degrees_wraps_angles_to_zero_to_360() {
        assert_eq!(normalize_hue_degrees(0.0), 0.0);
        assert_eq!(normalize_hue_degrees(360.0), 0.0);
        assert_eq!(normalize_hue_degrees(450.0), 90.0);
        assert_eq!(normalize_hue_degrees(-30.0), 330.0);
    }
}
