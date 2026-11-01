use gpui::{AbsoluteLength, CornersRefinement};

use gpui_luma::controls::slider::SliderOrientation;

use super::types::{Axis, ColorInterpolation};

#[derive(Clone, Debug)]
pub struct ColorSliderTrackContext {
    pub range: std::ops::Range<f32>,
    pub reversed: bool,
    pub axis: Axis,
    pub interpolation: ColorInterpolation,
    pub corner_radii: CornersRefinement<AbsoluteLength>,
    pub theme_is_dark: bool,
}

impl ColorSliderTrackContext {
    pub fn orientation(&self) -> SliderOrientation {
        match self.axis {
            Axis::Horizontal => SliderOrientation::Horizontal,
            Axis::Vertical => SliderOrientation::Vertical,
        }
    }
}

pub fn axis_from_orientation(orientation: SliderOrientation) -> Axis {
    match orientation {
        SliderOrientation::Horizontal => Axis::Horizontal,
        SliderOrientation::Vertical => Axis::Vertical,
    }
}

pub fn value_at_position(context: &ColorSliderTrackContext, position: f32) -> f32 {
    if context.reversed {
        context.range.end - (context.range.end - context.range.start) * position
    } else {
        context.range.start + (context.range.end - context.range.start) * position
    }
}

pub fn display_position(context: &ColorSliderTrackContext, value: f32) -> f32 {
    let span = context.range.end - context.range.start;
    if span.abs() <= f32::EPSILON {
        return 0.0;
    }

    let mut position = ((value - context.range.start) / span).clamp(0.0, 1.0);
    if context.reversed {
        position = 1.0 - position;
    }
    position
}
