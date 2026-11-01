use gpui::{Bounds, Pixels, Point};

use gpui_luma::controls::slider::RadialHitTarget;

use super::common::{arc_geometry, pointer_hits_arc_target, position_to_turn};
use super::track_context::ColorArcTrackContext;

pub struct ColorArcHitTarget {
    context: ColorArcTrackContext,
    thumb_size: Option<f32>,
}

impl ColorArcHitTarget {
    pub fn new(context: ColorArcTrackContext, thumb_size: Option<f32>) -> Self {
        Self { context, thumb_size }
    }
}

impl RadialHitTarget for ColorArcHitTarget {
    fn accepts_pointer(
        &self,
        bounds: Bounds<Pixels>,
        pointer: Point<Pixels>,
        thumb_position: f32,
        reversed: bool,
    ) -> bool {
        let Some(geometry) = arc_geometry(bounds, self.context.arc_thickness_px()) else {
            return false;
        };
        let thumb_position = if reversed { 1.0 - thumb_position } else { thumb_position };
        let thumb_turn = position_to_turn(thumb_position, self.context.start_turns(), self.context.sweep_turns());
        let thumb_size = self.context.thumb_size_px(self.thumb_size);
        pointer_hits_arc_target(
            pointer,
            geometry,
            self.context.start_turns(),
            self.context.sweep_turns(),
            thumb_turn,
            thumb_size,
        )
    }
}
