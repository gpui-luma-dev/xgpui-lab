use gpui_luma::color::{ColorValue, GamutMapping, gpui_bridge::SrgbRenderCache};
use gpui::*;

use super::color_spec::{self, ColorChannel, ColorSpecification};
use super::radius::inner_track_paint_corner_radius;
use super::types::{Axis, ColorInterpolation, ColorSliderDelegate};
use super::track_context::ColorSliderTrackContext;
use crate::checkerboard_paint::paint_masked_checkerboard;

/// Calculates the start offset and size of a segment with a slight overlap to prevent sub-pixel gaps.
///
/// This is used because rendering multiple adjacent containers (especially with gradients)
/// can often leave a thin gap between them due to anti-aliasing or sub-pixel positioning.
/// By overlapping the segments slightly (2.0px), we ensure a smooth transition.
fn calculate_overlapping_segment(
    index: usize,
    count: usize,
    item_size: Pixels,
    total_size: Pixels,
) -> (Pixels, Pixels) {
    let start_offset = index as f32 * item_size;
    let end_offset = if index == count - 1 {
        total_size
    } else {
        (index + 1) as f32 * item_size + gpui::px(2.0)
    };
    (start_offset, end_offset - start_offset)
}

fn axis_total_size(bounds: Bounds<Pixels>, axis: Axis) -> Pixels {
    if axis == Axis::Vertical {
        bounds.size.height
    } else {
        bounds.size.width
    }
}

fn size_along_axis(bounds: Bounds<Pixels>, axis: Axis) -> Pixels {
    axis_total_size(bounds, axis)
}

fn axis_gradient_angle(axis: Axis) -> f32 {
    if axis == Axis::Vertical { 180.0 } else { 90.0 }
}

fn axis_segment_bounds(
    bounds: Bounds<Pixels>,
    axis: Axis,
    start_offset: Pixels,
    segment_size: Pixels,
) -> Bounds<Pixels> {
    let (origin, size) = if axis == Axis::Vertical {
        (point(bounds.origin.x, bounds.origin.y + start_offset), size(bounds.size.width, segment_size))
    } else {
        (point(bounds.origin.x + start_offset, bounds.origin.y), size(segment_size, bounds.size.height))
    };

    Bounds { origin, size }
}

fn apply_edge_corner_radii(
    corner_radii: &mut Corners<Pixels>,
    axis: Axis,
    radius: Pixels,
    is_first: bool,
    is_last: bool,
) {
    if is_first {
        if axis == Axis::Vertical {
            corner_radii.top_left = radius;
            corner_radii.top_right = radius;
        } else {
            corner_radii.top_left = radius;
            corner_radii.bottom_left = radius;
        }
    }

    if is_last {
        if axis == Axis::Vertical {
            corner_radii.bottom_left = radius;
            corner_radii.bottom_right = radius;
        } else {
            corner_radii.top_right = radius;
            corner_radii.bottom_right = radius;
        }
    }
}

fn track_corner_radius(context: &ColorSliderTrackContext, bounds: Bounds<Pixels>, rem_size: Pixels) -> Pixels {
    inner_track_paint_corner_radius(&context.corner_radii, rem_size, bounds, context.axis)
}

fn effective_channel_range(ctx: &ColorSliderTrackContext, channel: ColorChannel) -> (f32, f32) {
    let slider_range = (ctx.range.start, ctx.range.end);
    let is_default_slider_range =
        (slider_range.0 - 0.0).abs() <= f32::EPSILON && (slider_range.1 - 1.0).abs() <= f32::EPSILON;
    let is_unit_channel = (channel.min - 0.0).abs() <= f32::EPSILON && (channel.max - 1.0).abs() <= f32::EPSILON;

    if is_default_slider_range && !is_unit_channel {
        (channel.min, channel.max)
    } else {
        (ctx.range.start.min(ctx.range.end), ctx.range.start.max(ctx.range.end))
    }
}

fn spectrum_position(context: &ColorSliderTrackContext, position: f32) -> f32 {
    if context.reversed { 1.0 - position } else { position }
}

const CHANNEL_SPECTRUM_SEGMENT_COUNT: f32 = 10.0;

fn channel_color_at_position<S: ColorSpecification>(
    spec: S,
    channel_name: &str,
    channel: ColorChannel,
    context: &ColorSliderTrackContext,
    position: f32,
) -> Hsla {
    let (range_min, range_max) = effective_channel_range(context, channel);
    let span = range_max - range_min;
    let t = spectrum_position(context, position).clamp(0.0, 1.0);

    let color_at_channel_t = |channel_t: f32| {
        let mut sample = spec;
        sample.set_value(channel_name, range_min + span * channel_t.clamp(0.0, 1.0));
        sample.to_hsla()
    };

    if context.interpolation == ColorInterpolation::Rgb {
        return color_spec::interpolate_rgb(color_at_channel_t(0.0), color_at_channel_t(1.0), t);
    }

    let scaled = t * CHANNEL_SPECTRUM_SEGMENT_COUNT;
    let segment_index = scaled.floor().min(CHANNEL_SPECTRUM_SEGMENT_COUNT - 1.0) as usize;
    let local_t = scaled - segment_index as f32;
    let segment_start_t = segment_index as f32 / CHANNEL_SPECTRUM_SEGMENT_COUNT;
    let segment_end_t = (segment_index + 1) as f32 / CHANNEL_SPECTRUM_SEGMENT_COUNT;
    color_spec::interpolate_rgb(color_at_channel_t(segment_start_t), color_at_channel_t(segment_end_t), local_t)
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HueDelegate;

fn paint_hue_spectrum(context: &ColorSliderTrackContext, bounds: Bounds<Pixels>, window: &mut Window) {
    let reversed = context.reversed;
    let axis = context.axis;
    let rem_size = window.rem_size();
    let total_size = axis_total_size(bounds, axis);
    let band_size = total_size / 6.0;
    let corner_radius = track_corner_radius(context, bounds, rem_size);

    let mut bands = [
        (hsla(0.0, 1.0, 0.5, 1.0), hsla(60.0 / 360.0, 1.0, 0.5, 1.0)),
        (hsla(60.0 / 360.0, 1.0, 0.5, 1.0), hsla(120.0 / 360.0, 1.0, 0.5, 1.0)),
        (hsla(120.0 / 360.0, 1.0, 0.5, 1.0), hsla(180.0 / 360.0, 1.0, 0.5, 1.0)),
        (hsla(180.0 / 360.0, 1.0, 0.5, 1.0), hsla(240.0 / 360.0, 1.0, 0.5, 1.0)),
        (hsla(240.0 / 360.0, 1.0, 0.5, 1.0), hsla(300.0 / 360.0, 1.0, 0.5, 1.0)),
        (hsla(300.0 / 360.0, 1.0, 0.5, 1.0), hsla(1.0, 1.0, 0.5, 1.0)),
    ];

    if reversed {
        bands.reverse();
        for band in bands.iter_mut() {
            let (start, end) = *band;
            *band = (end, start);
        }
    }

    for (i, (start_color, end_color)) in bands.iter().enumerate() {
        let (start_offset, current_band_size) = calculate_overlapping_segment(i, 6, band_size, total_size);
        let band_bounds = axis_segment_bounds(bounds, axis, start_offset, current_band_size);
        let mut corner_radii = Corners::default();
        apply_edge_corner_radii(&mut corner_radii, axis, corner_radius, i == 0, i == bands.len() - 1);
        let angle = axis_gradient_angle(axis);

        window.paint_quad(PaintQuad {
            bounds: band_bounds,
            corner_radii,
            background: linear_gradient(
                angle,
                linear_color_stop(*start_color, 0.0),
                linear_color_stop(*end_color, 1.0),
            ),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: BorderStyle::default(),
        });
    }
}

impl ColorSliderDelegate for HueDelegate {
    fn paint_domain_track(&self, context: &ColorSliderTrackContext, bounds: Bounds<Pixels>, window: &mut Window) {
        paint_hue_spectrum(context, bounds, window);
    }

    fn get_color_for_context(&self, context: &ColorSliderTrackContext, position: f32) -> Hsla {
        hue_spectrum_color_at_position(context, position)
    }
}

fn hue_spectrum_color_at_position(context: &ColorSliderTrackContext, position: f32) -> Hsla {
    const BAND_COUNT: f32 = 6.0;
    let mut bands = [
        (hsla(0.0, 1.0, 0.5, 1.0), hsla(60.0 / 360.0, 1.0, 0.5, 1.0)),
        (hsla(60.0 / 360.0, 1.0, 0.5, 1.0), hsla(120.0 / 360.0, 1.0, 0.5, 1.0)),
        (hsla(120.0 / 360.0, 1.0, 0.5, 1.0), hsla(180.0 / 360.0, 1.0, 0.5, 1.0)),
        (hsla(180.0 / 360.0, 1.0, 0.5, 1.0), hsla(240.0 / 360.0, 1.0, 0.5, 1.0)),
        (hsla(240.0 / 360.0, 1.0, 0.5, 1.0), hsla(300.0 / 360.0, 1.0, 0.5, 1.0)),
        (hsla(300.0 / 360.0, 1.0, 0.5, 1.0), hsla(1.0, 1.0, 0.5, 1.0)),
    ];

    if context.reversed {
        bands.reverse();
        for band in bands.iter_mut() {
            let (start, end) = *band;
            *band = (end, start);
        }
    }

    let t = spectrum_position(context, position).clamp(0.0, 1.0);
    let scaled = t * BAND_COUNT;
    let band_index = scaled.floor().min(BAND_COUNT - 1.0) as usize;
    let local_t = scaled - band_index as f32;
    let (start, end) = bands[band_index];
    color_spec::interpolate_rgb(start, end, local_t)
}

/// An original gradient color; GPUI previews are prepared separately.
#[derive(Clone, Debug, PartialEq)]
pub struct GradientStop {
    pub position: f32,
    pub color: ColorValue,
}
#[derive(Clone, Debug, PartialEq)]
struct PreviewGradientStop {
    position: f32,
    color: Hsla,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GradientDelegate {
    sources: Vec<GradientStop>,
    stops: Vec<PreviewGradientStop>,
    mapping: GamutMapping,
}
impl GradientDelegate {
    /// Validate and resolve immutable sRGB paint stops once.
    pub fn new(mut sources: Vec<GradientStop>, mapping: GamutMapping) -> anyhow::Result<Self> {
        let mut cache = SrgbRenderCache::default();
        for stop in &sources {
            anyhow::ensure!(
                stop.position.is_finite() && (0.0..=1.0).contains(&stop.position),
                "gradient stop position must be in 0..=1"
            );
            stop.color.validate()?;
        }
        sources.sort_by(|a, b| a.position.total_cmp(&b.position));
        let stops = sources
            .iter()
            .map(|stop| {
                Ok(PreviewGradientStop { position: stop.position, color: cache.resolve_hsla(stop.color, mapping)? })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Self { sources, stops, mapping })
    }
    pub fn from_colors(colors: Vec<ColorValue>) -> anyhow::Result<Self> {
        let len = colors.len();
        let sources = colors
            .into_iter()
            .enumerate()
            .map(|(index, color)| GradientStop {
                position: if len <= 1 { 0.0 } else { index as f32 / (len - 1) as f32 },
                color,
            })
            .collect();
        Self::new(sources, GamutMapping::CssLocalMinde)
    }
    pub fn source_stops(&self) -> &[GradientStop] {
        &self.sources
    }
    pub fn mapping(&self) -> GamutMapping {
        self.mapping
    }
    /// Sample the retained sources, without using current-backend preview colors.
    pub fn source_at_position(&self, position: f32, space: ColorInterpolation) -> anyhow::Result<ColorValue> {
        anyhow::ensure!(position.is_finite() && (0.0..=1.0).contains(&position), "gradient position must be in 0..=1");
        let first = self.sources.first().ok_or_else(|| anyhow::anyhow!("cannot sample an empty gradient"))?;
        if position <= first.position {
            return Ok(first.color);
        }
        for pair in self.sources.windows(2) {
            let [start, end] = pair else {
                continue;
            };
            if position <= end.position {
                let span = end.position - start.position;
                if span <= f32::EPSILON {
                    return Ok(end.color);
                }
                return color_spec::interpolate_source(
                    start.color,
                    end.color,
                    (position - start.position) / span,
                    space,
                );
            }
        }
        Ok(self.sources.last().map(|stop| stop.color).unwrap_or(first.color))
    }
}
fn gradient_stops(delegate: &GradientDelegate, context: &ColorSliderTrackContext) -> Vec<PreviewGradientStop> {
    if context.reversed {
        delegate
            .stops
            .iter()
            .rev()
            .map(|stop| PreviewGradientStop { position: 1.0 - stop.position, color: stop.color })
            .collect()
    } else {
        delegate.stops.clone()
    }
}

fn paint_gradient_spectrum(
    delegate: &GradientDelegate,
    context: &ColorSliderTrackContext,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) {
    let axis = context.axis;
    let interpolation = context.interpolation;
    let stops = gradient_stops(delegate, context);

    if stops.len() <= 1 {
        if let Some(stop) = stops.first() {
            let corner_radius = track_corner_radius(context, bounds, window.rem_size());
            window.paint_quad(PaintQuad {
                bounds,
                corner_radii: Corners::all(corner_radius),
                background: stop.color.into(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });
        }
        return;
    }

    let rem_size = window.rem_size();
    let total_size = axis_total_size(bounds, axis);
    let corner_radius = track_corner_radius(context, bounds, rem_size);
    let first = &stops[0];
    let last = &stops[stops.len() - 1];

    if first.position > 0.0 {
        let leading_size = total_size * first.position;
        let mut corner_radii = Corners::default();
        apply_edge_corner_radii(&mut corner_radii, axis, corner_radius, true, false);
        window.paint_quad(PaintQuad {
            bounds: axis_segment_bounds(bounds, axis, px(0.0), leading_size),
            corner_radii,
            background: first.color.into(),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: BorderStyle::default(),
        });
    }

    for pair in stops.windows(2) {
        let start = &pair[0];
        let end = &pair[1];
        let span = (end.position - start.position).max(0.0);
        if span <= f32::EPSILON {
            continue;
        }

        let band_bounds = axis_segment_bounds(bounds, axis, total_size * start.position, total_size * span);
        let mut corner_radii = Corners::default();
        apply_edge_corner_radii(
            &mut corner_radii,
            axis,
            corner_radius,
            start.position <= f32::EPSILON,
            end.position >= 1.0 - f32::EPSILON,
        );
        let angle = axis_gradient_angle(axis);

        if interpolation == ColorInterpolation::Rgb {
            window.paint_quad(PaintQuad {
                bounds: band_bounds,
                corner_radii,
                background: linear_gradient(
                    angle,
                    linear_color_stop(start.color, 0.0),
                    linear_color_stop(end.color, 1.0),
                ),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });
        } else {
            let sub_steps = 10;
            let band_size = size_along_axis(band_bounds, axis);
            let sub_segment_size = band_size / sub_steps as f32;

            for j in 0..sub_steps {
                let t_start = j as f32 / sub_steps as f32;
                let t_end = (j + 1) as f32 / sub_steps as f32;
                let sub_start_color = match interpolation {
                    ColorInterpolation::Hsl => color_spec::interpolate_hsl(start.color, end.color, t_start),
                    ColorInterpolation::Lab => color_spec::interpolate_lab(start.color, end.color, t_start),
                    ColorInterpolation::Rgb => unreachable!(),
                };
                let sub_end_color = match interpolation {
                    ColorInterpolation::Hsl => color_spec::interpolate_hsl(start.color, end.color, t_end),
                    ColorInterpolation::Lab => color_spec::interpolate_lab(start.color, end.color, t_end),
                    ColorInterpolation::Rgb => unreachable!(),
                };
                let (sub_start_offset, current_sub_size) =
                    calculate_overlapping_segment(j, sub_steps, sub_segment_size, band_size);
                let sub_bounds = axis_segment_bounds(band_bounds, axis, sub_start_offset, current_sub_size);
                let mut sub_radii = Corners::default();
                apply_edge_corner_radii(
                    &mut sub_radii,
                    axis,
                    corner_radius,
                    start.position <= f32::EPSILON && j == 0,
                    end.position >= 1.0 - f32::EPSILON && j == sub_steps - 1,
                );

                window.paint_quad(PaintQuad {
                    bounds: sub_bounds,
                    corner_radii: sub_radii,
                    background: linear_gradient(
                        angle,
                        linear_color_stop(sub_start_color, 0.0),
                        linear_color_stop(sub_end_color, 1.0),
                    ),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }
        }
    }

    if last.position < 1.0 {
        let trailing_offset = total_size * last.position;
        let trailing_size = total_size - trailing_offset;
        let mut corner_radii = Corners::default();
        apply_edge_corner_radii(&mut corner_radii, axis, corner_radius, false, true);
        window.paint_quad(PaintQuad {
            bounds: axis_segment_bounds(bounds, axis, trailing_offset, trailing_size),
            corner_radii,
            background: last.color.into(),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: BorderStyle::default(),
        });
    }
}

impl ColorSliderDelegate for GradientDelegate {
    fn paint_domain_track(&self, context: &ColorSliderTrackContext, bounds: Bounds<Pixels>, window: &mut Window) {
        paint_gradient_spectrum(self, context, bounds, window);
    }

    fn get_color_for_context(&self, context: &ColorSliderTrackContext, position: f32) -> Hsla {
        let pct = if context.reversed { 1.0 - position } else { position };
        let stops = gradient_stops(self, context);

        if stops.is_empty() {
            return hsla(0.0, 0.0, 0.0, 1.0);
        }
        if stops.len() == 1 {
            return stops[0].color;
        }

        let pct = pct.clamp(0.0, 1.0);
        if pct <= stops[0].position {
            return stops[0].color;
        }
        if pct >= stops[stops.len() - 1].position {
            return stops[stops.len() - 1].color;
        }

        for pair in stops.windows(2) {
            let start = &pair[0];
            let end = &pair[1];
            if pct < start.position || pct > end.position {
                continue;
            }

            let span = (end.position - start.position).max(f32::EPSILON);
            let t = (pct - start.position) / span;

            return match context.interpolation {
                ColorInterpolation::Rgb => color_spec::interpolate_rgb(start.color, end.color, t),
                ColorInterpolation::Hsl => color_spec::interpolate_hsl(start.color, end.color, t),
                ColorInterpolation::Lab => color_spec::interpolate_lab(start.color, end.color, t),
            };
        }

        stops[stops.len() - 1].color
    }
}

pub struct AlphaDelegate<S: ColorSpecification> {
    pub spec: S,
}

fn paint_alpha_spectrum<S: ColorSpecification>(
    delegate: &AlphaDelegate<S>,
    context: &ColorSliderTrackContext,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) {
    let axis = context.axis;
    let (start, end) = if context.reversed { (1.0, 0.0) } else { (0.0, 1.0) };
    let is_dark = context.theme_is_dark;
    let rem_size = window.rem_size();
    let radius = track_corner_radius(context, bounds, rem_size);

    let mut opaque_color = delegate.spec.to_hsla();
    opaque_color.a = 1.0;
    let mut transparent_color = opaque_color;
    transparent_color.a = 0.0;

    let (c1, c2) = if is_dark {
        (hsla(0., 0., 0.1, 1.), hsla(0., 0., 0.13, 1.))
    } else {
        (hsla(0., 0., 1.0, 1.), hsla(0., 0., 0.95, 1.))
    };

    window.paint_quad(PaintQuad {
        bounds,
        corner_radii: Corners::all(radius),
        background: c1.into(),
        border_widths: Edges::default(),
        border_color: transparent_black(),
        border_style: BorderStyle::default(),
    });

    paint_masked_checkerboard(window, bounds, radius, c2, true, px(color_spec::constants::CHECKERBOARD_SIZE));

    let angle = axis_gradient_angle(axis);
    window.paint_quad(PaintQuad {
        bounds,
        corner_radii: Corners::all(radius),
        background: linear_gradient(
            angle,
            linear_color_stop(transparent_color, start),
            linear_color_stop(opaque_color, end),
        ),
        border_widths: Edges::default(),
        border_color: transparent_black(),
        border_style: BorderStyle::default(),
    });
}

impl<S: ColorSpecification> ColorSliderDelegate for AlphaDelegate<S> {
    fn paint_domain_track(&self, context: &ColorSliderTrackContext, bounds: Bounds<Pixels>, window: &mut Window) {
        paint_alpha_spectrum(self, context, bounds, window);
    }

    fn get_color_for_context(&self, context: &ColorSliderTrackContext, position: f32) -> Hsla {
        let t = spectrum_position(context, position).clamp(0.0, 1.0);
        let mut opaque_color = self.spec.to_hsla();
        opaque_color.a = 1.0;
        let mut transparent_color = opaque_color;
        transparent_color.a = 0.0;
        color_spec::interpolate_rgb(transparent_color, opaque_color, t)
    }
}

pub struct ChannelDelegate<S: ColorSpecification> {
    spec: S,
    channel_name: SharedString,
}

impl<S: ColorSpecification> ChannelDelegate<S> {
    pub fn new(spec: S, channel_name: SharedString) -> Result<Self, String> {
        // Validate channel exists
        if !spec.channels().iter().any(|c| c.name == channel_name.as_ref()) {
            return Err(format!("Channel '{}' not found in {:?}", channel_name, spec.name()));
        }
        Ok(Self { spec, channel_name })
    }

    fn resolve_channel(&self) -> Option<ColorChannel> {
        self.spec.channels().iter().find(|c| c.name == self.channel_name.as_ref()).copied()
    }
}

fn paint_channel_spectrum<S: ColorSpecification>(
    delegate: &ChannelDelegate<S>,
    context: &ColorSliderTrackContext,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) {
    let Some(channel) = delegate.resolve_channel() else {
        return;
    };

    let axis = context.axis;
    let reversed = context.reversed;
    let interpolation = context.interpolation;
    let (range_min, range_max) = effective_channel_range(context, channel);

    if interpolation == ColorInterpolation::Rgb {
        let (start, end) = if reversed { (1.0, 0.0) } else { (0.0, 1.0) };
        let mut start_spec = delegate.spec;
        start_spec.set_value(delegate.channel_name.as_ref(), range_min);
        let mut end_spec = delegate.spec;
        end_spec.set_value(delegate.channel_name.as_ref(), range_max);
        let angle = axis_gradient_angle(axis);
        let radius = track_corner_radius(context, bounds, window.rem_size());
        window.paint_quad(PaintQuad {
            bounds,
            corner_radii: Corners::all(radius),
            background: linear_gradient(
                angle,
                linear_color_stop(start_spec.to_hsla(), start),
                linear_color_stop(end_spec.to_hsla(), end),
            ),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: BorderStyle::default(),
        });
        return;
    }

    let rem_size = window.rem_size();
    let total_size = axis_total_size(bounds, axis);
    let segment_count = 10;
    let segment_size = total_size / segment_count as f32;
    let corner_radius = track_corner_radius(context, bounds, rem_size);
    let spec = delegate.spec;
    let channel_name = delegate.channel_name.clone();

    for i in 0..segment_count {
        let t_start = i as f32 / segment_count as f32;
        let t_end = (i + 1) as f32 / segment_count as f32;
        let spec_t_start = if reversed { 1.0 - t_start } else { t_start };
        let spec_t_end = if reversed { 1.0 - t_end } else { t_end };
        let val_start = range_min + (range_max - range_min) * spec_t_start;
        let val_end = range_min + (range_max - range_min) * spec_t_end;
        let mut start_spec = spec;
        start_spec.set_value(channel_name.as_ref(), val_start);
        let start_color = start_spec.to_hsla();
        let mut end_spec = spec;
        end_spec.set_value(channel_name.as_ref(), val_end);
        let end_color = end_spec.to_hsla();
        let (start_offset, current_segment_size) =
            calculate_overlapping_segment(i, segment_count, segment_size, total_size);
        let band_bounds = axis_segment_bounds(bounds, axis, start_offset, current_segment_size);
        let mut corner_radii = Corners::default();
        apply_edge_corner_radii(&mut corner_radii, axis, corner_radius, i == 0, i == segment_count - 1);
        let angle = axis_gradient_angle(axis);

        window.paint_quad(PaintQuad {
            bounds: band_bounds,
            corner_radii,
            background: linear_gradient(angle, linear_color_stop(start_color, 0.0), linear_color_stop(end_color, 1.0)),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: BorderStyle::default(),
        });
    }
}

impl<S: ColorSpecification> ColorSliderDelegate for ChannelDelegate<S> {
    fn paint_domain_track(&self, context: &ColorSliderTrackContext, bounds: Bounds<Pixels>, window: &mut Window) {
        paint_channel_spectrum(self, context, bounds, window);
    }

    fn get_color_for_context(&self, context: &ColorSliderTrackContext, position: f32) -> Hsla {
        let Some(channel) = self.resolve_channel() else {
            return self.spec.to_hsla();
        };
        channel_color_at_position(self.spec, self.channel_name.as_ref(), channel, context, position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_slider::color_spec::Hsl;

    fn approx_eq(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-6, "expected {a} ~= {b}, delta={}", (a - b).abs());
    }

    #[::core::prelude::v1::test]
    fn overlapping_segment_adds_overlap_except_last_segment() {
        let item = px(10.0);
        let total = px(30.0);

        let (start0, size0) = calculate_overlapping_segment(0, 3, item, total);
        let (start1, size1) = calculate_overlapping_segment(1, 3, item, total);
        let (start2, size2) = calculate_overlapping_segment(2, 3, item, total);

        let start0: f32 = start0.into();
        let size0: f32 = size0.into();
        let start1: f32 = start1.into();
        let size1: f32 = size1.into();
        let start2: f32 = start2.into();
        let size2: f32 = size2.into();

        approx_eq(start0, 0.0);
        approx_eq(size0, 12.0);
        approx_eq(start1, 10.0);
        approx_eq(size1, 12.0);
        approx_eq(start2, 20.0);
        approx_eq(size2, 10.0);
    }

    #[::core::prelude::v1::test]
    fn overlapping_segment_uses_total_size_for_single_segment() {
        let (start, size) = calculate_overlapping_segment(0, 1, px(40.0), px(33.0));
        let start: f32 = start.into();
        let size: f32 = size.into();
        approx_eq(start, 0.0);
        approx_eq(size, 33.0);
    }

    #[::core::prelude::v1::test]
    fn channel_delegate_new_validates_channel_name() {
        let spec = Hsl { h: 0.0, s: 0.5, l: 0.5, a: 1.0 };

        let ok = ChannelDelegate::new(spec, Hsl::HUE.into());
        assert!(ok.is_ok());

        let err = ChannelDelegate::new(spec, "unknown".into());
        assert!(err.is_err());
        assert!(err.err().is_some_and(|message| message.contains("Channel 'unknown' not found")));
    }

    #[::core::prelude::v1::test]
    fn channel_preview_matches_rgb_track_interpolation() {
        use crate::color_slider::color_spec::Oklch;
        use crate::color_slider::track_context::ColorSliderTrackContext;
        use crate::color_slider::types::{Axis, ColorInterpolation};

        let spec = Oklch { l: 0.64, c: 0.14, h: 257.0, a: 1.0 };
        let delegate = ChannelDelegate::new(spec, Oklch::LIGHTNESS.into()).expect("valid channel");
        let channel = delegate.resolve_channel().expect("lightness channel");
        let context = ColorSliderTrackContext {
            range: 0.0..1.0,
            reversed: false,
            axis: Axis::Horizontal,
            interpolation: ColorInterpolation::Rgb,
            corner_radii: Default::default(),
            theme_is_dark: false,
        };

        let direct = {
            let mut sample = spec;
            sample.set_value(Oklch::LIGHTNESS, 0.5);
            sample.to_hsla()
        };
        let preview = channel_color_at_position(spec, Oklch::LIGHTNESS, channel, &context, 0.5);

        assert_ne!(preview, direct, "thumb preview should follow painted rgb gradient, not direct conversion");
    }

    fn test_track_context(range: std::ops::Range<f32>) -> crate::color_slider::track_context::ColorSliderTrackContext {
        use crate::color_slider::track_context::ColorSliderTrackContext;
        use crate::color_slider::types::{Axis, ColorInterpolation};

        ColorSliderTrackContext {
            range,
            reversed: false,
            axis: Axis::Horizontal,
            interpolation: ColorInterpolation::Rgb,
            corner_radii: Default::default(),
            theme_is_dark: false,
        }
    }

    #[::core::prelude::v1::test]
    fn gradient_delegate_uses_explicit_stop_positions_for_sampling() {
        let delegate = GradientDelegate::new(
            vec![
                GradientStop { position: 0.2, color: gpui_luma::color::gpui_bridge::from_hsla(black()) },
                GradientStop { position: 0.8, color: gpui_luma::color::gpui_bridge::from_hsla(white()) },
            ],
            GamutMapping::CssLocalMinde,
        )
        .unwrap();
        let context = test_track_context(0.0..1.0);

        assert_eq!(delegate.get_color_for_context(&context, 0.0), black());
        assert_eq!(delegate.get_color_for_context(&context, 0.2), black());
        assert_eq!(delegate.get_color_for_context(&context, 1.0), white());

        let middle = delegate.get_color_for_context(&context, 0.5).to_rgb();
        assert!((middle.r - 0.5).abs() < 0.05, "expected midpoint mix, got {:?}", middle);
    }

    #[::core::prelude::v1::test]
    fn gradient_delegate_from_colors_evenly_spaces_stops() {
        let delegate = GradientDelegate::from_colors(
            vec![black(), white(), black()].into_iter().map(gpui_luma::color::gpui_bridge::from_hsla).collect(),
        )
        .unwrap();

        assert_eq!(
            delegate.stops,
            vec![
                PreviewGradientStop { position: 0.0, color: black() },
                PreviewGradientStop { position: 0.5, color: white() },
                PreviewGradientStop { position: 1.0, color: black() },
            ]
        );
    }

    #[::core::prelude::v1::test]
    fn hsl_mixer_channel_previews_are_not_all_swatch() {
        use crate::color_slider::color_spec::Hsl;

        let spec = Hsl { h: 210.0, s: 0.72, l: 0.52, a: 0.85 };
        let swatch = spec.to_hsla();
        let context = test_track_context(0.0..1.0);
        let sat_channel = spec.channels()[1];
        let light_channel = spec.channels()[2];

        let sat_preview = channel_color_at_position(spec, Hsl::SATURATION, sat_channel, &context, 0.72);
        let light_preview = channel_color_at_position(spec, Hsl::LIGHTNESS, light_channel, &context, 0.52);

        assert_ne!(sat_preview, swatch);
        assert_ne!(light_preview, swatch);
        assert_ne!(sat_preview, light_preview);
    }

    #[::core::prelude::v1::test]
    fn rgb_mixer_channel_previews_differ_per_channel() {
        use crate::color_slider::color_spec::RgbaSpec;

        let spec = RgbaSpec { r: 128.0, g: 64.0, b: 192.0, a: 1.0 };
        let context = test_track_context(0.0..255.0);
        let channels = spec.channels();

        let red_preview = channel_color_at_position(spec, RgbaSpec::RED, channels[0], &context, 0.25);
        let green_preview = channel_color_at_position(spec, RgbaSpec::GREEN, channels[1], &context, 0.75);

        assert_ne!(red_preview, green_preview);
    }

    #[::core::prelude::v1::test]
    fn hsv_mixer_value_preview_follows_track_not_swatch() {
        use crate::color_slider::color_spec::Hsv;

        let spec = Hsv { h: 12.0, s: 0.78, v: 0.86, a: 1.0 };
        let swatch = spec.to_hsla();
        let context = test_track_context(0.0..1.0);
        let value_channel = spec.channels()[2];
        let preview = channel_color_at_position(spec, Hsv::VALUE, value_channel, &context, 0.86);

        assert_ne!(preview, swatch);
    }

    #[::core::prelude::v1::test]
    fn lab_mixer_channel_previews_follow_track_not_swatch() {
        use crate::color_slider::color_spec::Lab;

        let spec = Lab { l: 50.0, a: 30.0, b: -20.0, alpha: 1.0, auto_clamp: false, dynamic_range: false };
        let swatch = spec.to_hsla();
        let context = test_track_context(0.0..100.0);
        let lightness_channel = spec.channels()[0];
        let preview = channel_color_at_position(spec, Lab::LIGHTNESS, lightness_channel, &context, 0.5);

        assert_ne!(preview, swatch);
    }

    #[::core::prelude::v1::test]
    fn hue_delegate_preview_is_rainbow_not_composed_swatch() {
        use crate::color_slider::color_spec::Hsl;
        use crate::color_slider::types::ColorSliderDelegate;

        let spec = Hsl { h: 210.0, s: 0.72, l: 0.52, a: 1.0 };
        let context = test_track_context(0.0..360.0);
        let preview = HueDelegate.get_color_for_context(&context, 210.0 / 360.0);

        assert_ne!(preview, spec.to_hsla());
    }
}

#[cfg(test)]
mod gradient_source_tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn p3_stops_and_sample_are_independent_of_preview_mapping() {
        let source = ColorValue::display_p3(1.1, -0.1, 0.2, 0.345678);
        let stops = vec![GradientStop { position: 0.0, color: source }, GradientStop { position: 1.0, color: source }];
        let mapped = GradientDelegate::new(stops.clone(), GamutMapping::CssLocalMinde).unwrap();
        let clipped = GradientDelegate::new(stops.clone(), GamutMapping::Clip).unwrap();
        assert_eq!(mapped.source_stops(), stops);
        assert_eq!(mapped.source_at_position(0.0, ColorInterpolation::Lab).unwrap(), source);
        for space in [ColorInterpolation::Rgb, ColorInterpolation::Hsl, ColorInterpolation::Lab] {
            assert_eq!(mapped.source_at_position(0.5, space).unwrap(), clipped.source_at_position(0.5, space).unwrap());
        }
        assert!(
            !mapped
                .source_at_position(0.5, ColorInterpolation::Rgb)
                .unwrap()
                .is_in_gamut(gpui_luma::color::Gamut::Srgb)
                .unwrap()
        );
    }
}
