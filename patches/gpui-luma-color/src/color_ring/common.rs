use crate::style::Size;
use gpui::{Bounds, Pixels, Point, px};
use std::f32::consts::TAU;

#[derive(Clone, Copy)]
pub(crate) struct RingGeometry {
    pub center_x: f32,
    pub center_y: f32,
    pub outer_radius: f32,
    pub inner_radius: f32,
    pub track_radius: f32,
}

pub(crate) fn size_px(size: Size) -> f32 {
    match size {
        Size::XSmall => 140.0,
        Size::Small => 180.0,
        Size::Medium => 220.0,
        Size::Large => 280.0,
        Size::Size(px) => px.as_f32(),
    }
}

pub(crate) fn mirrored_saturation(position: f32) -> f32 {
    (1.0 - (position.rem_euclid(1.0) - 0.5).abs() * 2.0).clamp(0.0, 1.0)
}

pub(crate) fn mirrored_lightness(position: f32) -> f32 {
    ((position.rem_euclid(1.0) - 0.5).abs() * 2.0).clamp(0.0, 1.0)
}

pub(crate) fn position_from_mirrored_saturation(value: f32) -> f32 {
    (1.0 - value.clamp(0.0, 1.0) * 0.5).rem_euclid(1.0)
}

pub(crate) fn position_from_mirrored_lightness(value: f32) -> f32 {
    (0.5 + value.clamp(0.0, 1.0) * 0.5).rem_euclid(1.0)
}

pub(crate) fn ring_geometry(bounds: Bounds<Pixels>, ring_thickness: f32) -> Option<RingGeometry> {
    if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return None;
    }

    let width: f32 = bounds.size.width.into();
    let height: f32 = bounds.size.height.into();
    let origin_x: f32 = bounds.origin.x.into();
    let origin_y: f32 = bounds.origin.y.into();
    let center_x = origin_x + width / 2.0;
    let center_y = origin_y + height / 2.0;
    let outer_radius = width.min(height) / 2.0;
    let inner_radius = (outer_radius - ring_thickness).max(0.0);
    let track_radius = (outer_radius - ring_thickness / 2.0).max(0.0);

    Some(RingGeometry { center_x, center_y, outer_radius, inner_radius, track_radius })
}

pub(crate) fn position_to_theta(position: f32, rotation_turns: f32) -> f32 {
    (position - 0.25 + rotation_turns) * TAU
}

pub(crate) fn thumb_center(geometry: RingGeometry, theta: f32) -> (f32, f32) {
    (
        geometry.center_x + geometry.track_radius * theta.cos(),
        geometry.center_y + geometry.track_radius * theta.sin(),
    )
}

pub(crate) fn pointer_in_thumb_box(
    pointer: Point<Pixels>,
    geometry: RingGeometry,
    theta: f32,
    thumb_size: f32,
) -> bool {
    let (thumb_center_x, thumb_center_y) = thumb_center(geometry, theta);
    let half = thumb_size / 2.0;
    let pointer_x: f32 = pointer.x.into();
    let pointer_y: f32 = pointer.y.into();
    pointer_x >= thumb_center_x - half
        && pointer_x <= thumb_center_x + half
        && pointer_y >= thumb_center_y - half
        && pointer_y <= thumb_center_y + half
}

pub(crate) fn pointer_hits_active_target(
    pointer: Point<Pixels>,
    geometry: RingGeometry,
    theta: f32,
    thumb_size: f32,
    allow_inner_target: bool,
) -> bool {
    let pointer_x: f32 = pointer.x.into();
    let pointer_y: f32 = pointer.y.into();
    let dx = pointer_x - geometry.center_x;
    let dy = pointer_y - geometry.center_y;
    let radius = (dx * dx + dy * dy).sqrt();
    let on_ring = radius >= geometry.inner_radius && radius <= geometry.outer_radius;
    let on_active_disk = allow_inner_target && radius <= geometry.outer_radius;
    let on_thumb = pointer_in_thumb_box(pointer, geometry, theta, thumb_size);
    on_ring || on_active_disk || on_thumb
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-5, "expected {a} ~= {b}, delta={}", (a - b).abs());
    }

    #[test]
    fn mirrored_mappings_round_trip_via_inverse_position_functions() {
        for value in [0.0, 0.2, 0.5, 0.8, 1.0] {
            let sat_pos = position_from_mirrored_saturation(value);
            approx_eq(mirrored_saturation(sat_pos), value);

            let light_pos = position_from_mirrored_lightness(value);
            approx_eq(mirrored_lightness(light_pos), value);
        }
    }

    #[test]
    fn theta_and_thumb_geometry_helpers_are_consistent() {
        let geometry =
            RingGeometry { center_x: 10.0, center_y: 20.0, outer_radius: 30.0, inner_radius: 20.0, track_radius: 25.0 };

        let theta = position_to_theta(0.25, 0.0);
        approx_eq(theta, 0.0);

        let (cx, cy) = thumb_center(geometry, theta);
        approx_eq(cx, 35.0);
        approx_eq(cy, 20.0);
    }

    #[test]
    fn pointer_hits_active_target_checks_ring_inner_and_thumb_areas() {
        let geometry =
            RingGeometry { center_x: 50.0, center_y: 50.0, outer_radius: 40.0, inner_radius: 30.0, track_radius: 35.0 };
        let theta = 0.0;
        let thumb_size = 12.0;

        let ring_point = Point { x: px(85.0), y: px(50.0) };
        assert!(pointer_hits_active_target(ring_point, geometry, theta, thumb_size, false));

        let center_point = Point { x: px(50.0), y: px(50.0) };
        assert!(!pointer_hits_active_target(center_point, geometry, theta, thumb_size, false));
        assert!(pointer_hits_active_target(center_point, geometry, theta, thumb_size, true));
    }
}
