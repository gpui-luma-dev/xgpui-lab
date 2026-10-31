use gpui::{AbsoluteLength, Bounds, Corners, CornersRefinement, Pixels, px};

use super::types::Axis;

/// Matches [`ColorSliderTemplate`](super::template::ColorSliderTemplate) track `border_1()`.
pub const TRACK_BORDER_WIDTH: Pixels = px(1.0);

pub fn axis_default_corner_radius(bounds: Bounds<Pixels>, axis: Axis) -> Pixels {
    if axis == Axis::Vertical {
        bounds.size.width / 2.0
    } else {
        bounds.size.height / 2.0
    }
}

pub fn resolve_track_corner_radii(
    corner_radii: &CornersRefinement<AbsoluteLength>,
    rem_size: Pixels,
    fallback: Pixels,
) -> Corners<Pixels> {
    Corners {
        top_left: corner_radii.top_left.map(|v| v.to_pixels(rem_size)).unwrap_or(fallback),
        top_right: corner_radii.top_right.map(|v| v.to_pixels(rem_size)).unwrap_or(fallback),
        bottom_left: corner_radii.bottom_left.map(|v| v.to_pixels(rem_size)).unwrap_or(fallback),
        bottom_right: corner_radii.bottom_right.map(|v| v.to_pixels(rem_size)).unwrap_or(fallback),
    }
}

pub fn resolve_track_corner_radius(
    corner_radii: &CornersRefinement<AbsoluteLength>,
    rem_size: Pixels,
    fallback: Pixels,
) -> Pixels {
    resolve_track_corner_radii(corner_radii, rem_size, fallback).top_left
}

pub fn inset_corner_radius(outer: Pixels, border_width: Pixels) -> Pixels {
    px((f32::from(outer) - f32::from(border_width)).max(0.0))
}

pub fn inset_corner_radii(outer: Corners<Pixels>, border_width: Pixels) -> Corners<Pixels> {
    Corners {
        top_left: inset_corner_radius(outer.top_left, border_width),
        top_right: inset_corner_radius(outer.top_right, border_width),
        bottom_left: inset_corner_radius(outer.bottom_left, border_width),
        bottom_right: inset_corner_radius(outer.bottom_right, border_width),
    }
}

/// Radius for fill/clip layers inside a bordered track shell.
pub fn inner_track_corner_radius(outer: Pixels) -> Pixels {
    inset_corner_radius(outer, TRACK_BORDER_WIDTH)
}

/// Radius for domain-track painting inside a bordered track shell.
pub fn inner_track_paint_corner_radius(
    corner_radii: &CornersRefinement<AbsoluteLength>,
    rem_size: Pixels,
    bounds: Bounds<Pixels>,
    axis: Axis,
) -> Pixels {
    let inner_fallback = axis_default_corner_radius(bounds, axis);
    if corner_radii.top_left.is_some() {
        inset_corner_radius(resolve_track_corner_radius(corner_radii, rem_size, inner_fallback), TRACK_BORDER_WIDTH)
    } else {
        inner_fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn resolve_track_corner_radius_uses_configured_value() {
        let corner_radii = CornersRefinement {
            top_left: Some(px(8.0).into()),
            top_right: Some(px(8.0).into()),
            bottom_left: Some(px(8.0).into()),
            bottom_right: Some(px(8.0).into()),
        };

        let resolved = resolve_track_corner_radius(&corner_radii, px(16.0), px(12.0));
        assert_eq!(f32::from(resolved), 8.0);
    }

    #[test]
    fn resolve_track_corner_radius_falls_back_when_unset() {
        let corner_radii = CornersRefinement::default();
        let resolved = resolve_track_corner_radius(&corner_radii, px(16.0), px(12.0));
        assert_eq!(f32::from(resolved), 12.0);
    }

    #[test]
    fn inner_track_corner_radius_insets_for_border() {
        assert_eq!(f32::from(inner_track_corner_radius(px(8.0))), 7.0);
        assert_eq!(f32::from(inner_track_corner_radius(px(12.0))), 11.0);
    }

    #[test]
    fn inner_track_paint_corner_radius_uses_inset_when_configured() {
        let corner_radii = CornersRefinement { top_left: Some(px(8.0).into()), ..Default::default() };

        let bounds = Bounds { origin: gpui::point(px(0.0), px(0.0)), size: gpui::size(px(100.0), px(22.0)) };
        let resolved = inner_track_paint_corner_radius(&corner_radii, px(16.0), bounds, Axis::Horizontal);
        assert_eq!(f32::from(resolved), 7.0);
    }

    #[test]
    fn inner_track_paint_corner_radius_uses_bounds_fallback_when_unset() {
        let corner_radii = CornersRefinement::default();
        let bounds = Bounds { origin: gpui::point(px(0.0), px(0.0)), size: gpui::size(px(100.0), px(22.0)) };
        let resolved = inner_track_paint_corner_radius(&corner_radii, px(16.0), bounds, Axis::Horizontal);
        assert_eq!(f32::from(resolved), 11.0);
    }
}
