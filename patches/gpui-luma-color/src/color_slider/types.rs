pub use super::color_thumb::ThumbShape;

pub mod sizing {
    pub const TRACK_THICKNESS_XSMALL: f32 = 4.0;
    pub const TRACK_THICKNESS_SMALL: f32 = 14.0;
    pub const TRACK_THICKNESS_MEDIUM: f32 = 24.0;
    pub const TRACK_THICKNESS_LARGE: f32 = 34.0;

    pub const THUMB_SIZE_XSMALL: f32 = 10.0;
    pub const THUMB_SIZE_SMALL: f32 = 12.0;
    pub const THUMB_SIZE_MEDIUM: f32 = 20.0;
    pub const THUMB_SIZE_LARGE: f32 = 30.0;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ColorInterpolation {
    #[default]
    Rgb,
    Hsl,
    Lab,
}

pub trait ColorSliderDelegate: Send + Sync + 'static {
    fn paint_domain_track(
        &self,
        context: &super::track_context::ColorSliderTrackContext,
        bounds: gpui::Bounds<gpui::Pixels>,
        window: &mut gpui::Window,
    );

    fn get_color_for_context(
        &self,
        context: &super::track_context::ColorSliderTrackContext,
        position: f32,
    ) -> gpui::Hsla;
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ThumbPosition {
    #[default]
    InsideSlider,
    EdgeToEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ThumbSize {
    XSmall,
    Small,
    #[default]
    Medium,
    Large,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ThumbConfig {
    pub position: ThumbPosition,
    pub size: ThumbSize,
    pub shape: ThumbShape,
}

pub fn normalized_value_percent(value: f32, start: f32, end: f32) -> f32 {
    let span = end - start;
    if span.abs() <= f32::EPSILON {
        return 0.0;
    }

    ((value - start) / span).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::normalized_value_percent;

    fn approx_eq(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-6, "expected {a} ~= {b}");
    }

    #[test]
    fn normalized_value_percent_handles_regular_and_degenerate_ranges() {
        approx_eq(normalized_value_percent(25.0, 0.0, 100.0), 0.25);
        approx_eq(normalized_value_percent(-10.0, 0.0, 100.0), 0.0);
        approx_eq(normalized_value_percent(150.0, 0.0, 100.0), 1.0);
        approx_eq(normalized_value_percent(10.0, 5.0, 5.0), 0.0);
    }

    #[test]
    fn normalized_value_percent_supports_descending_ranges() {
        approx_eq(normalized_value_percent(1.0, 1.0, 0.0), 0.0);
        approx_eq(normalized_value_percent(0.5, 1.0, 0.0), 0.5);
        approx_eq(normalized_value_percent(0.0, 1.0, 0.0), 1.0);
    }
}
