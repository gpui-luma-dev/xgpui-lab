use gpui::{
    Bounds, Corners, Edges, Hsla, PaintQuad, Pixels, Window, linear_color_stop, linear_gradient, px, transparent_black,
};
use image::{Frame, ImageBuffer, Rgba};
use smallvec::smallvec;
use std::sync::Arc;
use tiny_skia::{Pixmap, PremultipliedColorU8};

use super::types::{GradientType, color_at_position, sorted_stops};
use super::super::color::interpolate_rgb;

const EDGE_EPSILON: f32 = 0.001;

fn horizontal_edge_corner_radii(radii: Corners<Pixels>, touches_left: bool, touches_right: bool) -> Corners<Pixels> {
    Corners {
        top_left: if touches_left { radii.top_left } else { px(0.0) },
        bottom_left: if touches_left { radii.bottom_left } else { px(0.0) },
        top_right: if touches_right { radii.top_right } else { px(0.0) },
        bottom_right: if touches_right { radii.bottom_right } else { px(0.0) },
    }
}

fn vertical_edge_corner_radii(radii: Corners<Pixels>, touches_top: bool, touches_bottom: bool) -> Corners<Pixels> {
    Corners {
        top_left: if touches_top { radii.top_left } else { px(0.0) },
        top_right: if touches_top { radii.top_right } else { px(0.0) },
        bottom_left: if touches_bottom { radii.bottom_left } else { px(0.0) },
        bottom_right: if touches_bottom { radii.bottom_right } else { px(0.0) },
    }
}

pub fn paint_gradient_preview(
    gradient_type: GradientType,
    window: &mut Window,
    bounds: Bounds<Pixels>,
    stops: &[(f32, Hsla)],
    rotation_deg: f32,
    corner_radii: Corners<Pixels>,
    allow_native_two_stop: bool,
) {
    match gradient_type {
        GradientType::Linear => {
            paint_linear_gradient_preview(window, bounds, stops, rotation_deg, corner_radii, allow_native_two_stop);
        }
        GradientType::Radial => paint_rasterized_radial_gradient_preview(window, bounds, stops, corner_radii),
        GradientType::Angular => {
            paint_rasterized_angular_gradient_preview(window, bounds, stops, rotation_deg, corner_radii);
        }
    }
}

pub fn rasterize_gradient_preview(
    gradient_type: GradientType,
    size: gpui::Size<Pixels>,
    stops: &[(f32, Hsla)],
    rotation_deg: f32,
) -> Option<Arc<gpui::RenderImage>> {
    match gradient_type {
        GradientType::Linear => rasterize_linear_gradient_preview(size, stops, rotation_deg),
        GradientType::Radial => rasterize_radial_gradient_preview(size, stops),
        GradientType::Angular => rasterize_angular_gradient_preview(size, stops, rotation_deg),
    }
}

fn paint_horizontal_gradient_track(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    stops: &[(f32, Hsla)],
    reverse: bool,
    corner_radii: Corners<Pixels>,
) {
    let stops = sorted_stops(stops);
    if stops.is_empty() {
        return;
    }

    if stops.len() == 1 {
        paint_solid(window, bounds, stops[0].1, horizontal_edge_corner_radii(corner_radii, true, true));
        return;
    }

    let width = bounds.size.width.as_f32();
    if width <= 0.0 {
        return;
    }

    let (first_pos, first_color, tail_pos, tail_color, iter) = if reverse {
        let last = stops[stops.len() - 1];
        let first = stops[0];
        let reversed = stops
            .windows(2)
            .rev()
            .map(|pair| (1.0 - pair[1].0, pair[1].1, 1.0 - pair[0].0, pair[0].1))
            .collect::<Vec<_>>();
        (1.0 - last.0, last.1, 1.0 - first.0, first.1, reversed)
    } else {
        let first = stops[0];
        let last = stops[stops.len() - 1];
        let forward = stops.windows(2).map(|pair| (pair[0].0, pair[0].1, pair[1].0, pair[1].1)).collect::<Vec<_>>();
        (first.0, first.1, last.0, last.1, forward)
    };

    if first_pos > 0.0 {
        let segment_end = bounds.origin.x + bounds.size.width * first_pos;
        paint_solid(
            window,
            Bounds { origin: bounds.origin, size: gpui::size(segment_end - bounds.origin.x, bounds.size.height) },
            first_color,
            horizontal_edge_corner_radii(corner_radii, true, false),
        );
    }

    for (start_pos, start_color, end_pos, end_color) in iter {
        let segment_start = bounds.origin.x + bounds.size.width * start_pos;
        let segment_end = bounds.origin.x + bounds.size.width * end_pos;
        if segment_end <= segment_start {
            continue;
        }
        let segment_bounds = Bounds {
            origin: gpui::point(segment_start, bounds.origin.y),
            size: gpui::size(segment_end - segment_start, bounds.size.height),
        };
        let touches_left = start_pos <= EDGE_EPSILON;
        let touches_right = end_pos >= 1.0 - EDGE_EPSILON;

        window.paint_quad(PaintQuad {
            bounds: segment_bounds,
            corner_radii: horizontal_edge_corner_radii(corner_radii, touches_left, touches_right),
            background: linear_gradient(90.0, linear_color_stop(start_color, 0.0), linear_color_stop(end_color, 1.0)),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: gpui::BorderStyle::default(),
        });
    }

    if tail_pos < 1.0 {
        let segment_start = bounds.origin.x + bounds.size.width * tail_pos;
        paint_solid(
            window,
            Bounds {
                origin: gpui::point(segment_start, bounds.origin.y),
                size: gpui::size(bounds.origin.x + bounds.size.width - segment_start, bounds.size.height),
            },
            tail_color,
            horizontal_edge_corner_radii(corner_radii, false, true),
        );
    }
}

fn paint_linear_gradient_preview(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    stops: &[(f32, Hsla)],
    rotation_deg: f32,
    corner_radii: Corners<Pixels>,
    allow_native_two_stop: bool,
) {
    let stops = sorted_stops(stops);
    if stops.is_empty() {
        return;
    }

    if stops.len() == 1 {
        paint_solid(window, bounds, stops[0].1, corner_radii);
        return;
    }

    if allow_native_two_stop && stops.len() == 2 {
        let (start_pos, start_color) = stops[0];
        let (end_pos, end_color) = stops[1];
        window.paint_quad(PaintQuad {
            bounds,
            corner_radii,
            background: linear_gradient(
                rotation_deg,
                linear_color_stop(start_color, start_pos),
                linear_color_stop(end_color, end_pos),
            ),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: gpui::BorderStyle::default(),
        });
        return;
    }

    if rotation_deg.rem_euclid(180.0).abs() < f32::EPSILON {
        paint_vertical_gradient_preview(window, bounds, &stops, rotation_deg, corner_radii);
        return;
    }

    if (rotation_deg - 270.0).rem_euclid(360.0).abs() < f32::EPSILON {
        paint_horizontal_gradient_track(window, bounds, &stops, true, corner_radii);
        return;
    }

    if (rotation_deg - 90.0).rem_euclid(360.0).abs() < f32::EPSILON {
        paint_horizontal_gradient_track(window, bounds, &stops, false, corner_radii);
        return;
    }

    paint_rasterized_gradient_preview(window, bounds, &stops, rotation_deg, corner_radii);
}

fn rasterize_linear_gradient_preview(
    size: gpui::Size<Pixels>,
    stops: &[(f32, Hsla)],
    rotation_deg: f32,
) -> Option<Arc<gpui::RenderImage>> {
    let stops = sorted_stops(stops);
    if stops.is_empty() {
        return None;
    }

    let scale = raster_scale_for_size(size);
    let width = (size.width.as_f32() * scale).round() as u32;
    let height = (size.height.as_f32() * scale).round() as u32;
    if width == 0 || height == 0 {
        return None;
    }

    let radians = rotation_deg.to_radians();
    let direction = (radians.sin(), -radians.cos());
    let corners = [(0.0_f32, 0.0_f32), (1.0_f32, 0.0_f32), (0.0_f32, 1.0_f32), (1.0_f32, 1.0_f32)];
    let mut min_projection = f32::INFINITY;
    let mut max_projection = f32::NEG_INFINITY;
    for (x, y) in corners {
        let projection = x * direction.0 + y * direction.1;
        min_projection = min_projection.min(projection);
        max_projection = max_projection.max(projection);
    }
    let projection_span = (max_projection - min_projection).max(f32::EPSILON);

    let mut pixmap = Pixmap::new(width, height)?;
    let pixels = pixmap.pixels_mut();

    for y in 0..height {
        for x in 0..width {
            let sample_x = (x as f32 + 0.5) / width as f32;
            let sample_y = (y as f32 + 0.5) / height as f32;
            let projection = sample_x * direction.0 + sample_y * direction.1;
            let sample = ((projection - min_projection) / projection_span).clamp(0.0, 1.0);
            let color = color_at_position(&stops, sample);
            let rgb = color.to_rgb();
            let alpha = color.a.clamp(0.0, 1.0);
            let r_u8 = (rgb.r.clamp(0.0, 1.0) * alpha * 255.0).round() as u8;
            let g_u8 = (rgb.g.clamp(0.0, 1.0) * alpha * 255.0).round() as u8;
            let b_u8 = (rgb.b.clamp(0.0, 1.0) * alpha * 255.0).round() as u8;
            let a_u8 = (alpha * 255.0).round() as u8;

            if let Some(pixel) = PremultipliedColorU8::from_rgba(b_u8, g_u8, r_u8, a_u8) {
                pixels[(y * width + x) as usize] = pixel;
            }
        }
    }

    let raw_bytes = pixmap.data().to_vec();
    let image_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, raw_bytes)?;
    let frame = Frame::new(image_buffer);
    Some(Arc::new(gpui::RenderImage::new(smallvec![frame])))
}

fn paint_vertical_gradient_preview(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    stops: &[(f32, Hsla)],
    rotation_deg: f32,
    corner_radii: Corners<Pixels>,
) {
    if stops.len() == 1 {
        paint_solid(window, bounds, stops[0].1, vertical_edge_corner_radii(corner_radii, true, true));
        return;
    }

    let height = bounds.size.height.as_f32();
    if height <= 0.0 {
        return;
    }

    let reverse = (rotation_deg - 180.0).abs() < f32::EPSILON;
    let (first_pos, first_color) = if reverse {
        let last = stops[stops.len() - 1];
        (1.0 - last.0, last.1)
    } else {
        stops[0]
    };
    if first_pos > 0.0 {
        let segment_end = bounds.origin.y + bounds.size.height * first_pos;
        paint_solid(
            window,
            Bounds { origin: bounds.origin, size: gpui::size(bounds.size.width, segment_end - bounds.origin.y) },
            first_color,
            vertical_edge_corner_radii(corner_radii, true, false),
        );
    }

    for pair in stops.windows(2) {
        let (start_pos, start_color) = pair[0];
        let (end_pos, end_color) = pair[1];
        let (start_pos, end_pos, start_color, end_color) = if reverse {
            (1.0 - end_pos, 1.0 - start_pos, end_color, start_color)
        } else {
            (start_pos, end_pos, start_color, end_color)
        };
        let segment_start = bounds.origin.y + bounds.size.height * start_pos;
        let segment_end = bounds.origin.y + bounds.size.height * end_pos;
        let segment_bounds = Bounds {
            origin: gpui::point(bounds.origin.x, segment_start),
            size: gpui::size(bounds.size.width, segment_end - segment_start),
        };
        let touches_top = start_pos <= EDGE_EPSILON;
        let touches_bottom = end_pos >= 1.0 - EDGE_EPSILON;

        window.paint_quad(PaintQuad {
            bounds: segment_bounds,
            corner_radii: vertical_edge_corner_radii(corner_radii, touches_top, touches_bottom),
            background: linear_gradient(180.0, linear_color_stop(start_color, 0.0), linear_color_stop(end_color, 1.0)),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: gpui::BorderStyle::default(),
        });
    }

    let (last_pos, last_color) = if reverse {
        let first = stops[0];
        (1.0 - first.0, first.1)
    } else {
        stops[stops.len() - 1]
    };
    if last_pos < 1.0 {
        let segment_start = bounds.origin.y + bounds.size.height * last_pos;
        paint_solid(
            window,
            Bounds {
                origin: gpui::point(bounds.origin.x, segment_start),
                size: gpui::size(bounds.size.width, bounds.origin.y + bounds.size.height - segment_start),
            },
            last_color,
            vertical_edge_corner_radii(corner_radii, false, true),
        );
    }
}

fn raster_scale_for_size(size: gpui::Size<Pixels>) -> f32 {
    let max_side = size.width.max(size.height).as_f32();
    if max_side <= 220.0 {
        2.0
    } else if max_side <= 420.0 {
        1.5
    } else {
        1.25
    }
}

fn color_at_wrapped_position(stops: &[(f32, Hsla)], position: f32) -> Hsla {
    if stops.is_empty() {
        return gpui::hsla(0.0, 0.0, 0.0, 1.0);
    }
    if stops.len() == 1 {
        return stops[0].1;
    }

    let position = position.rem_euclid(1.0);
    let stops = sorted_stops(stops);

    for window in stops.windows(2) {
        let (left_pos, left_color) = window[0];
        let (right_pos, right_color) = window[1];
        if position >= left_pos && position <= right_pos {
            let span = (right_pos - left_pos).max(f32::EPSILON);
            let t = (position - left_pos) / span;
            return interpolate_rgb(left_color, right_color, t);
        }
    }

    let (last_pos, last_color) = stops[stops.len() - 1];
    let (first_pos, first_color) = stops[0];
    let wrapped_span = (1.0 - last_pos + first_pos).max(f32::EPSILON);
    let wrapped_position = if position >= last_pos {
        position - last_pos
    } else {
        1.0 - last_pos + position
    };
    interpolate_rgb(last_color, first_color, (wrapped_position / wrapped_span).clamp(0.0, 1.0))
}

fn rasterize_radial_gradient_preview(
    size: gpui::Size<Pixels>,
    stops: &[(f32, Hsla)],
) -> Option<Arc<gpui::RenderImage>> {
    let stops = sorted_stops(stops);
    rasterize_with_sampler(size, &stops, |sample_x, sample_y, sorted| {
        let dx = sample_x - 0.5;
        let dy = sample_y - 0.5;
        let max_radius = (0.5_f32 * 0.5 + 0.5 * 0.5).sqrt();
        let distance = (dx * dx + dy * dy).sqrt();
        color_at_position(sorted, (distance / max_radius).clamp(0.0, 1.0))
    })
}

fn rasterize_angular_gradient_preview(
    size: gpui::Size<Pixels>,
    stops: &[(f32, Hsla)],
    rotation_deg: f32,
) -> Option<Arc<gpui::RenderImage>> {
    let stops = sorted_stops(stops);
    let rotation_turns = rotation_deg.rem_euclid(360.0) / 360.0;
    rasterize_with_sampler(size, &stops, move |sample_x, sample_y, sorted| {
        let dx = sample_x - 0.5;
        let dy = sample_y - 0.5;
        let angle = dx.atan2(-dy).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        color_at_wrapped_position(sorted, (angle - rotation_turns).rem_euclid(1.0))
    })
}

fn rasterize_with_sampler<F>(
    size: gpui::Size<Pixels>,
    stops: &[(f32, Hsla)],
    mut sample_color: F,
) -> Option<Arc<gpui::RenderImage>>
where
    F: FnMut(f32, f32, &[(f32, Hsla)]) -> Hsla,
{
    if stops.is_empty() {
        return None;
    }

    let scale = raster_scale_for_size(size);
    let width = (size.width.as_f32() * scale).round() as u32;
    let height = (size.height.as_f32() * scale).round() as u32;
    if width == 0 || height == 0 {
        return None;
    }

    let mut pixmap = Pixmap::new(width, height)?;
    let pixels = pixmap.pixels_mut();

    for y in 0..height {
        for x in 0..width {
            let sample_x = (x as f32 + 0.5) / width as f32;
            let sample_y = (y as f32 + 0.5) / height as f32;
            let color = sample_color(sample_x, sample_y, stops);
            let rgb = color.to_rgb();
            let alpha = color.a.clamp(0.0, 1.0);
            let r_u8 = (rgb.r.clamp(0.0, 1.0) * alpha * 255.0).round() as u8;
            let g_u8 = (rgb.g.clamp(0.0, 1.0) * alpha * 255.0).round() as u8;
            let b_u8 = (rgb.b.clamp(0.0, 1.0) * alpha * 255.0).round() as u8;
            let a_u8 = (alpha * 255.0).round() as u8;

            if let Some(pixel) = PremultipliedColorU8::from_rgba(b_u8, g_u8, r_u8, a_u8) {
                pixels[(y * width + x) as usize] = pixel;
            }
        }
    }

    pixmap_to_render_image(pixmap)
}

fn pixmap_to_render_image(pixmap: Pixmap) -> Option<Arc<gpui::RenderImage>> {
    let width = pixmap.width();
    let height = pixmap.height();
    let raw_bytes = pixmap.data().to_vec();
    let image_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, raw_bytes)?;
    let frame = Frame::new(image_buffer);
    Some(Arc::new(gpui::RenderImage::new(smallvec![frame])))
}

fn paint_rasterized_radial_gradient_preview(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    stops: &[(f32, Hsla)],
    corner_radii: Corners<Pixels>,
) {
    paint_rasterized_sampled_preview(window, bounds, stops, corner_radii, |sample_x, sample_y, sorted| {
        let dx = sample_x - 0.5;
        let dy = sample_y - 0.5;
        let max_radius = (0.5_f32 * 0.5 + 0.5 * 0.5).sqrt();
        let distance = (dx * dx + dy * dy).sqrt();
        color_at_position(sorted, (distance / max_radius).clamp(0.0, 1.0))
    });
}

fn paint_rasterized_angular_gradient_preview(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    stops: &[(f32, Hsla)],
    rotation_deg: f32,
    corner_radii: Corners<Pixels>,
) {
    let rotation_turns = rotation_deg.rem_euclid(360.0) / 360.0;
    paint_rasterized_sampled_preview(window, bounds, stops, corner_radii, move |sample_x, sample_y, sorted| {
        let dx = sample_x - 0.5;
        let dy = sample_y - 0.5;
        let angle = dx.atan2(-dy).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        color_at_wrapped_position(sorted, (angle - rotation_turns).rem_euclid(1.0))
    });
}

fn paint_rasterized_sampled_preview<F>(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    stops: &[(f32, Hsla)],
    corner_radii: Corners<Pixels>,
    mut sample_color: F,
) where
    F: FnMut(f32, f32, &[(f32, Hsla)]) -> Hsla,
{
    let width = bounds.size.width.as_f32();
    let height = bounds.size.height.as_f32();
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    let columns = ((width / 8.0).ceil() as usize).clamp(24, 160);
    let rows = ((height / 8.0).ceil() as usize).clamp(16, 96);

    for row in 0..rows {
        let y0 = row as f32 / rows as f32;
        let y1 = (row + 1) as f32 / rows as f32;
        for column in 0..columns {
            let x0 = column as f32 / columns as f32;
            let x1 = (column + 1) as f32 / columns as f32;
            let sample_x = (x0 + x1) * 0.5;
            let sample_y = (y0 + y1) * 0.5;
            let color = sample_color(sample_x, sample_y, stops);

            let left = bounds.origin.x + bounds.size.width * x0;
            let right = bounds.origin.x + bounds.size.width * x1;
            let top = bounds.origin.y + bounds.size.height * y0;
            let bottom = bounds.origin.y + bounds.size.height * y1;
            let segment_bounds =
                Bounds { origin: gpui::point(left, top), size: gpui::size(right - left, bottom - top) };
            let radii = Corners {
                top_left: if row == 0 && column == 0 {
                    corner_radii.top_left
                } else {
                    px(0.0)
                },
                top_right: if row == 0 && column + 1 == columns {
                    corner_radii.top_right
                } else {
                    px(0.0)
                },
                bottom_left: if row + 1 == rows && column == 0 {
                    corner_radii.bottom_left
                } else {
                    px(0.0)
                },
                bottom_right: if row + 1 == rows && column + 1 == columns {
                    corner_radii.bottom_right
                } else {
                    px(0.0)
                },
            };
            paint_solid(window, segment_bounds, color, radii);
        }
    }
}

fn paint_rasterized_gradient_preview(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    stops: &[(f32, Hsla)],
    rotation_deg: f32,
    corner_radii: Corners<Pixels>,
) {
    let width = bounds.size.width.as_f32();
    let height = bounds.size.height.as_f32();
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    let radians = rotation_deg.to_radians();
    let direction = (radians.sin(), -radians.cos());
    let corners = [(0.0_f32, 0.0_f32), (1.0_f32, 0.0_f32), (0.0_f32, 1.0_f32), (1.0_f32, 1.0_f32)];
    let mut min_projection = f32::INFINITY;
    let mut max_projection = f32::NEG_INFINITY;
    for (x, y) in corners {
        let projection = x * direction.0 + y * direction.1;
        min_projection = min_projection.min(projection);
        max_projection = max_projection.max(projection);
    }
    let projection_span = (max_projection - min_projection).max(f32::EPSILON);

    let columns = ((width / 8.0).ceil() as usize).clamp(24, 160);
    let rows = ((height / 8.0).ceil() as usize).clamp(16, 96);

    for row in 0..rows {
        let y0 = row as f32 / rows as f32;
        let y1 = (row + 1) as f32 / rows as f32;
        for column in 0..columns {
            let x0 = column as f32 / columns as f32;
            let x1 = (column + 1) as f32 / columns as f32;
            let sample_x = (x0 + x1) * 0.5;
            let sample_y = (y0 + y1) * 0.5;
            let projection = sample_x * direction.0 + sample_y * direction.1;
            let sample = ((projection - min_projection) / projection_span).clamp(0.0, 1.0);
            let color = color_at_position(stops, sample);

            let left = bounds.origin.x + bounds.size.width * x0;
            let right = bounds.origin.x + bounds.size.width * x1;
            let top = bounds.origin.y + bounds.size.height * y0;
            let bottom = bounds.origin.y + bounds.size.height * y1;
            let segment_bounds =
                Bounds { origin: gpui::point(left, top), size: gpui::size(right - left, bottom - top) };
            let radii = Corners {
                top_left: if row == 0 && column == 0 {
                    corner_radii.top_left
                } else {
                    px(0.0)
                },
                top_right: if row == 0 && column + 1 == columns {
                    corner_radii.top_right
                } else {
                    px(0.0)
                },
                bottom_left: if row + 1 == rows && column == 0 {
                    corner_radii.bottom_left
                } else {
                    px(0.0)
                },
                bottom_right: if row + 1 == rows && column + 1 == columns {
                    corner_radii.bottom_right
                } else {
                    px(0.0)
                },
            };
            paint_solid(window, segment_bounds, color, radii);
        }
    }
}

fn paint_solid(window: &mut Window, bounds: Bounds<Pixels>, color: Hsla, corner_radii: Corners<Pixels>) {
    window.paint_quad(PaintQuad {
        bounds,
        corner_radii,
        background: color.into(),
        border_widths: Edges::default(),
        border_color: transparent_black(),
        border_style: gpui::BorderStyle::default(),
    });
}
