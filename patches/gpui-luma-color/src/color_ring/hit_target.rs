use gpui::{Bounds, Pixels, Point};

use gpui_luma::controls::slider::RadialHitTarget;

use super::common::{pointer_hits_active_target, position_to_theta, ring_geometry};
use super::track_context::ColorRingTrackContext;

pub struct ColorRingHitTarget {
    context: ColorRingTrackContext,
}

impl ColorRingHitTarget {
    pub fn new(context: ColorRingTrackContext) -> Self {
        Self { context }
    }
}

impl RadialHitTarget for ColorRingHitTarget {
    fn accepts_pointer(
        &self,
        bounds: Bounds<Pixels>,
        pointer: Point<Pixels>,
        thumb_position: f32,
        reversed: bool,
    ) -> bool {
        let Some(geometry) = ring_geometry(bounds, self.context.ring_thickness_px()) else {
            return false;
        };
        let thumb_position = if reversed { 1.0 - thumb_position } else { thumb_position };
        let theta = position_to_theta(thumb_position, self.context.rotation_turns());
        pointer_hits_active_target(
            pointer,
            geometry,
            theta,
            self.context.thumb_size_px(),
            self.context.allow_inner_target,
        )
    }
}
