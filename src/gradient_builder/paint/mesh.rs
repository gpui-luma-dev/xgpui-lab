use gpui::{Bounds, Corners, Hsla, Pixels, Window, px};
use image::{Frame, ImageBuffer, Rgba};
use smallvec::smallvec;
use std::sync::Arc;
use tiny_skia::{Paint, PathBuilder, Pixmap, PremultipliedColorU8, Stroke, Transform};

use super::types::MeshPoint;
use super::super::color::interpolate_rgb;

const GEOMETRY_TANGENT_LIMIT: f32 = 0.7;
const COLOR_TANGENT_SCALE: f32 = 0.75;

pub trait MeshGradientAlgorithm {
    fn rasterize(
        &self,
        size: gpui::Size<Pixels>,
        points: &[MeshPoint],
        background: Hsla,
    ) -> Option<Arc<gpui::RenderImage>>;
}

pub struct CoonsPatchMeshAlgorithm;

impl MeshGradientAlgorithm for CoonsPatchMeshAlgorithm {
    fn rasterize(
        &self,
        size: gpui::Size<Pixels>,
        points: &[MeshPoint],
        background: Hsla,
    ) -> Option<Arc<gpui::RenderImage>> {
        rasterize_with_coons_patch(size, points, background, true)
    }
}

static DEFAULT_MESH_ALGORITHM: CoonsPatchMeshAlgorithm = CoonsPatchMeshAlgorithm;

/// Render the mesh with optional editing guides.
pub fn rasterize_mesh_gradient_preview(
    size: gpui::Size<Pixels>,
    points: &[MeshPoint],
    background: Hsla,
    show_guides: bool,
) -> Option<Arc<gpui::RenderImage>> {
    if show_guides {
        DEFAULT_MESH_ALGORITHM.rasterize(size, points, background)
    } else {
        rasterize_with_coons_patch(size, points, background, false)
    }
}

// This hook is intentionally kept even before a second algorithm is wired in.
#[allow(dead_code)]
pub fn rasterize_mesh_gradient_preview_with<A: MeshGradientAlgorithm + ?Sized>(
    algorithm: &A,
    size: gpui::Size<Pixels>,
    points: &[MeshPoint],
    background: Hsla,
) -> Option<Arc<gpui::RenderImage>> {
    algorithm.rasterize(size, points, background)
}

pub fn mesh_dimensions(points: &[MeshPoint]) -> Option<(usize, usize)> {
    let rows = points.iter().map(|point| point.row as usize).max()? + 1;
    let cols = points.iter().map(|point| point.col as usize).max()? + 1;
    if rows < 2 || cols < 2 || points.len() != rows * cols {
        return None;
    }
    Some((rows, cols))
}

fn rasterize_with_coons_patch(
    size: gpui::Size<Pixels>,
    points: &[MeshPoint],
    background: Hsla,
    show_guides: bool,
) -> Option<Arc<gpui::RenderImage>> {
    let (rows, cols) = mesh_dimensions(points)?;
    let cells = build_mesh_cells(points, rows, cols);

    let scale = raster_scale_for_size(size);
    let width = (size.width.as_f32() * scale).round() as u32;
    let height = (size.height.as_f32() * scale).round() as u32;
    if width == 0 || height == 0 {
        return None;
    }

    let mut pixmap = Pixmap::new(width, height)?;
    fill_pixmap(&mut pixmap, background);
    let subdivisions = mesh_cell_subdivisions(width, height);
    {
        let pixels = pixmap.pixels_mut();
        for cell in &cells {
            rasterize_mesh_cell(pixels, width, height, cell, subdivisions);
        }
    }

    if show_guides {
        paint_mesh_guides(&mut pixmap, points, rows, cols, width as f32, height as f32);
    }
    pixmap_to_render_image(pixmap)
}

#[derive(Clone, Copy, Debug)]
struct MeshCell {
    p00: MeshPoint,
    p10: MeshPoint,
    p01: MeshPoint,
    p11: MeshPoint,
    top_geom: Cubic2,
    bottom_geom: Cubic2,
    left_geom: Cubic2,
    right_geom: Cubic2,
    top_color: Cubic4,
    bottom_color: Cubic4,
    left_color: Cubic4,
    right_color: Cubic4,
}

#[derive(Clone, Copy, Debug)]
struct Cubic2 {
    p0: (f32, f32),
    c1: (f32, f32),
    c2: (f32, f32),
    p1: (f32, f32),
}

#[derive(Clone, Copy, Debug)]
struct Cubic4 {
    p0: [f32; 4],
    c1: [f32; 4],
    c2: [f32; 4],
    p1: [f32; 4],
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

fn fill_pixmap(pixmap: &mut Pixmap, color: Hsla) {
    let rgba = hsla_to_rgba4(color);
    let Some(pixel) = rgba_to_pixel(rgba) else {
        return;
    };
    for existing in pixmap.pixels_mut().iter_mut() {
        *existing = pixel;
    }
}

fn pixmap_to_render_image(pixmap: Pixmap) -> Option<Arc<gpui::RenderImage>> {
    let width = pixmap.width();
    let height = pixmap.height();
    let raw_bytes = pixmap.data().to_vec();
    let image_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, raw_bytes)?;
    let frame = Frame::new(image_buffer);
    Some(Arc::new(gpui::RenderImage::new(smallvec![frame])))
}

fn mesh_cell_subdivisions(width: u32, height: u32) -> usize {
    let max_side = width.max(height) as f32;
    if max_side <= 220.0 {
        24
    } else if max_side <= 420.0 {
        32
    } else {
        40
    }
}

fn rasterize_mesh_cell(
    pixels: &mut [PremultipliedColorU8],
    width: u32,
    height: u32,
    cell: &MeshCell,
    subdivisions: usize,
) {
    let step = 1.0 / subdivisions as f32;
    for row in 0..subdivisions {
        let v0 = row as f32 * step;
        let v1 = (row + 1) as f32 * step;
        for col in 0..subdivisions {
            let u0 = col as f32 * step;
            let u1 = (col + 1) as f32 * step;

            let p00 = scale_to_pixels(coons_position(cell, u0, v0), width, height);
            let p10 = scale_to_pixels(coons_position(cell, u1, v0), width, height);
            let p01 = scale_to_pixels(coons_position(cell, u0, v1), width, height);
            let p11 = scale_to_pixels(coons_position(cell, u1, v1), width, height);

            let c00 = hsla_to_rgba4(sample_mesh_cell_color(cell, u0, v0));
            let c10 = hsla_to_rgba4(sample_mesh_cell_color(cell, u1, v0));
            let c01 = hsla_to_rgba4(sample_mesh_cell_color(cell, u0, v1));
            let c11 = hsla_to_rgba4(sample_mesh_cell_color(cell, u1, v1));

            rasterize_mesh_quad(pixels, width, height, p00, c00, p10, c10, p11, c11, p01, c01);
        }
    }
}

fn sample_mesh_cell_color(cell: &MeshCell, u: f32, v: f32) -> Hsla {
    coons_color(cell, u, v)
        .unwrap_or_else(|| bilinear_hsla_color(cell.p00.color, cell.p10.color, cell.p01.color, cell.p11.color, u, v))
}

fn scale_to_pixels(point: (f32, f32), width: u32, height: u32) -> (f32, f32) {
    (point.0 * width as f32, point.1 * height as f32)
}

fn hsla_to_rgba4(color: Hsla) -> [f32; 4] {
    let rgb = color.to_rgb();
    [rgb.r, rgb.g, rgb.b, color.a]
}

#[allow(clippy::too_many_arguments)]
fn rasterize_mesh_quad(
    pixels: &mut [PremultipliedColorU8],
    width: u32,
    height: u32,
    p00: (f32, f32),
    c00: [f32; 4],
    p10: (f32, f32),
    c10: [f32; 4],
    p11: (f32, f32),
    c11: [f32; 4],
    p01: (f32, f32),
    c01: [f32; 4],
) {
    let split_a = split_score(p00, p10, p11, p01, true);
    let split_b = split_score(p00, p10, p11, p01, false);

    if split_a >= split_b {
        rasterize_triangle(pixels, width, height, p00, c00, p10, c10, p11, c11);
        rasterize_triangle(pixels, width, height, p00, c00, p11, c11, p01, c01);
    } else {
        rasterize_triangle(pixels, width, height, p00, c00, p10, c10, p01, c01);
        rasterize_triangle(pixels, width, height, p10, c10, p11, c11, p01, c01);
    }
}

#[allow(clippy::too_many_arguments)]
fn rasterize_triangle(
    pixels: &mut [PremultipliedColorU8],
    width: u32,
    height: u32,
    p0: (f32, f32),
    c0: [f32; 4],
    p1: (f32, f32),
    c1: [f32; 4],
    p2: (f32, f32),
    c2: [f32; 4],
) {
    let min_x = p0.0.min(p1.0).min(p2.0).floor().max(0.0) as i32;
    let max_x = p0.0.max(p1.0).max(p2.0).ceil().min(width as f32 - 1.0) as i32;
    let min_y = p0.1.min(p1.1).min(p2.1).floor().max(0.0) as i32;
    let max_y = p0.1.max(p1.1).max(p2.1).ceil().min(height as f32 - 1.0) as i32;
    if min_x > max_x || min_y > max_y {
        return;
    }

    let Some((_, _, _)) = barycentric_weights((p0.0, p0.1), p0, p1, p2) else {
        return;
    };

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let sample = (x as f32 + 0.5, y as f32 + 0.5);
            let Some((w0, w1, w2)) = barycentric_weights(sample, p0, p1, p2) else {
                continue;
            };
            if w0 < -0.001 || w1 < -0.001 || w2 < -0.001 {
                continue;
            }

            let rgba = add4(add4(scale4(c0, w0), scale4(c1, w1)), scale4(c2, w2));
            if let Some(pixel) = rgba_to_pixel(rgba) {
                pixels[(y as u32 * width + x as u32) as usize] = pixel;
            }
        }
    }
}

fn split_score(p00: (f32, f32), p10: (f32, f32), p11: (f32, f32), p01: (f32, f32), use_primary_diagonal: bool) -> f32 {
    let (a0, a1, b0, b1, c0, c1) = if use_primary_diagonal {
        (p00, p10, p11, p00, p11, p01)
    } else {
        (p00, p10, p01, p10, p11, p01)
    };

    let area_a = signed_triangle_area(a0, a1, b0);
    let area_b = signed_triangle_area(b1, c0, c1);
    if area_a.abs() <= f32::EPSILON || area_b.abs() <= f32::EPSILON {
        return f32::NEG_INFINITY;
    }

    let same_winding_bonus = if area_a.signum() == area_b.signum() { 1.0 } else { -1.0 };
    let min_area = area_a.abs().min(area_b.abs());
    let max_area = area_a.abs().max(area_b.abs());
    same_winding_bonus * (min_area / max_area.max(f32::EPSILON))
}

fn signed_triangle_area(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn mesh_point(points: &[MeshPoint], row: usize, col: usize) -> MeshPoint {
    points
        .iter()
        .copied()
        .find(|point| point.row as usize == row && point.col as usize == col)
        .unwrap_or(MeshPoint {
            row: row as u8,
            col: col as u8,
            u: col as f32 * 0.5,
            v: row as f32 * 0.5,
            color: gpui::hsla(0.0, 0.0, 0.0, 1.0),
        })
}

fn build_mesh_cells(points: &[MeshPoint], rows: usize, cols: usize) -> Vec<MeshCell> {
    let mut cells = Vec::with_capacity((rows - 1) * (cols - 1));
    for row in 0..(rows - 1) {
        for col in 0..(cols - 1) {
            let p00 = mesh_point(points, row, col);
            let p10 = mesh_point(points, row, col + 1);
            let p01 = mesh_point(points, row + 1, col);
            let p11 = mesh_point(points, row + 1, col + 1);

            let tx00 = mesh_tangent_position(points, rows, cols, row, col, true);
            let tx10 = mesh_tangent_position(points, rows, cols, row, col + 1, true);
            let tx01 = mesh_tangent_position(points, rows, cols, row + 1, col, true);
            let tx11 = mesh_tangent_position(points, rows, cols, row + 1, col + 1, true);
            let ty00 = mesh_tangent_position(points, rows, cols, row, col, false);
            let ty10 = mesh_tangent_position(points, rows, cols, row, col + 1, false);
            let ty01 = mesh_tangent_position(points, rows, cols, row + 1, col, false);
            let ty11 = mesh_tangent_position(points, rows, cols, row + 1, col + 1, false);

            let cx00 = mesh_tangent_color(points, rows, cols, row, col, true);
            let cx10 = mesh_tangent_color(points, rows, cols, row, col + 1, true);
            let cx01 = mesh_tangent_color(points, rows, cols, row + 1, col, true);
            let cx11 = mesh_tangent_color(points, rows, cols, row + 1, col + 1, true);
            let cy00 = mesh_tangent_color(points, rows, cols, row, col, false);
            let cy10 = mesh_tangent_color(points, rows, cols, row, col + 1, false);
            let cy01 = mesh_tangent_color(points, rows, cols, row + 1, col, false);
            let cy11 = mesh_tangent_color(points, rows, cols, row + 1, col + 1, false);

            cells.push(MeshCell {
                p00,
                p10,
                p01,
                p11,
                top_geom: cubic2_from_tangents(point2(p00), tx00, point2(p10), tx10),
                bottom_geom: cubic2_from_tangents(point2(p01), tx01, point2(p11), tx11),
                left_geom: cubic2_from_tangents(point2(p00), ty00, point2(p01), ty01),
                right_geom: cubic2_from_tangents(point2(p10), ty10, point2(p11), ty11),
                top_color: cubic4_from_tangents(color4(p00), cx00, color4(p10), cx10),
                bottom_color: cubic4_from_tangents(color4(p01), cx01, color4(p11), cx11),
                left_color: cubic4_from_tangents(color4(p00), cy00, color4(p01), cy01),
                right_color: cubic4_from_tangents(color4(p10), cy10, color4(p11), cy11),
            });
        }
    }
    cells
}

fn mesh_tangent_position(
    points: &[MeshPoint],
    rows: usize,
    cols: usize,
    row: usize,
    col: usize,
    horizontal: bool,
) -> (f32, f32) {
    let raw = if horizontal {
        if col == 0 {
            sub2(point2(mesh_point(points, row, col + 1)), point2(mesh_point(points, row, col)))
        } else if col + 1 == cols {
            sub2(point2(mesh_point(points, row, col)), point2(mesh_point(points, row, col - 1)))
        } else {
            scale2(sub2(point2(mesh_point(points, row, col + 1)), point2(mesh_point(points, row, col - 1))), 0.5)
        }
    } else if row == 0 {
        sub2(point2(mesh_point(points, row + 1, col)), point2(mesh_point(points, row, col)))
    } else if row + 1 == rows {
        sub2(point2(mesh_point(points, row, col)), point2(mesh_point(points, row - 1, col)))
    } else {
        scale2(sub2(point2(mesh_point(points, row + 1, col)), point2(mesh_point(points, row - 1, col))), 0.5)
    };

    clamp_vec2_length(raw, tangent_position_limit(points, rows, cols, row, col, horizontal) * GEOMETRY_TANGENT_LIMIT)
}

fn mesh_tangent_color(
    points: &[MeshPoint],
    rows: usize,
    cols: usize,
    row: usize,
    col: usize,
    horizontal: bool,
) -> [f32; 4] {
    let raw = if horizontal {
        if col == 0 {
            sub4(color4(mesh_point(points, row, col + 1)), color4(mesh_point(points, row, col)))
        } else if col + 1 == cols {
            sub4(color4(mesh_point(points, row, col)), color4(mesh_point(points, row, col - 1)))
        } else {
            monotone_color_tangent(
                sub4(color4(mesh_point(points, row, col)), color4(mesh_point(points, row, col - 1))),
                sub4(color4(mesh_point(points, row, col + 1)), color4(mesh_point(points, row, col))),
            )
        }
    } else if row == 0 {
        sub4(color4(mesh_point(points, row + 1, col)), color4(mesh_point(points, row, col)))
    } else if row + 1 == rows {
        sub4(color4(mesh_point(points, row, col)), color4(mesh_point(points, row - 1, col)))
    } else {
        monotone_color_tangent(
            sub4(color4(mesh_point(points, row, col)), color4(mesh_point(points, row - 1, col))),
            sub4(color4(mesh_point(points, row + 1, col)), color4(mesh_point(points, row, col))),
        )
    };

    scale4(raw, COLOR_TANGENT_SCALE)
}

fn point2(point: MeshPoint) -> (f32, f32) {
    (point.u, point.v)
}

fn color4(point: MeshPoint) -> [f32; 4] {
    let rgb = point.color.to_rgb();
    [rgb.r, rgb.g, rgb.b, point.color.a]
}

fn cubic2_from_tangents(p0: (f32, f32), t0: (f32, f32), p1: (f32, f32), t1: (f32, f32)) -> Cubic2 {
    Cubic2 { p0, c1: add2(p0, scale2(t0, 1.0 / 3.0)), c2: sub2(p1, scale2(t1, 1.0 / 3.0)), p1 }
}

fn cubic4_from_tangents(p0: [f32; 4], t0: [f32; 4], p1: [f32; 4], t1: [f32; 4]) -> Cubic4 {
    Cubic4 { p0, c1: add4(p0, scale4(t0, 1.0 / 3.0)), c2: sub4(p1, scale4(t1, 1.0 / 3.0)), p1 }
}

fn cubic2_eval(curve: Cubic2, t: f32) -> (f32, f32) {
    let one = 1.0 - t;
    add2(
        add2(scale2(curve.p0, one * one * one), scale2(curve.c1, 3.0 * one * one * t)),
        add2(scale2(curve.c2, 3.0 * one * t * t), scale2(curve.p1, t * t * t)),
    )
}

fn cubic4_eval(curve: Cubic4, t: f32) -> [f32; 4] {
    let one = 1.0 - t;
    add4(
        add4(scale4(curve.p0, one * one * one), scale4(curve.c1, 3.0 * one * one * t)),
        add4(scale4(curve.c2, 3.0 * one * t * t), scale4(curve.p1, t * t * t)),
    )
}

fn coons_position(cell: &MeshCell, u: f32, v: f32) -> (f32, f32) {
    let lc = add2(scale2(cubic2_eval(cell.top_geom, u), 1.0 - v), scale2(cubic2_eval(cell.bottom_geom, u), v));
    let dc = add2(scale2(cubic2_eval(cell.left_geom, v), 1.0 - u), scale2(cubic2_eval(cell.right_geom, v), u));
    sub2(
        add2(lc, dc),
        bilinear2(point2(cell.p00), point2(cell.p10), point2(cell.p01), point2(cell.p11), u, v),
    )
}

fn coons_color(cell: &MeshCell, u: f32, v: f32) -> Option<Hsla> {
    let lc = add4(scale4(cubic4_eval(cell.top_color, u), 1.0 - v), scale4(cubic4_eval(cell.bottom_color, u), v));
    let dc = add4(scale4(cubic4_eval(cell.left_color, v), 1.0 - u), scale4(cubic4_eval(cell.right_color, v), u));
    let rgba = sub4(
        add4(lc, dc),
        bilinear4(color4(cell.p00), color4(cell.p10), color4(cell.p01), color4(cell.p11), u, v),
    );
    rgba_to_hsla(rgba)
}

fn bilinear2(p00: (f32, f32), p10: (f32, f32), p01: (f32, f32), p11: (f32, f32), u: f32, v: f32) -> (f32, f32) {
    add2(
        add2(scale2(p00, (1.0 - u) * (1.0 - v)), scale2(p10, u * (1.0 - v))),
        add2(scale2(p01, (1.0 - u) * v), scale2(p11, u * v)),
    )
}

fn bilinear4(c00: [f32; 4], c10: [f32; 4], c01: [f32; 4], c11: [f32; 4], u: f32, v: f32) -> [f32; 4] {
    add4(
        add4(scale4(c00, (1.0 - u) * (1.0 - v)), scale4(c10, u * (1.0 - v))),
        add4(scale4(c01, (1.0 - u) * v), scale4(c11, u * v)),
    )
}

fn bilinear_hsla_color(c00: Hsla, c10: Hsla, c01: Hsla, c11: Hsla, u: f32, v: f32) -> Hsla {
    let top = interpolate_rgb(c00, c10, u);
    let bottom = interpolate_rgb(c01, c11, u);
    interpolate_rgb(top, bottom, v)
}

fn add2(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0 + b.0, a.1 + b.1)
}

fn sub2(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0 - b.0, a.1 - b.1)
}

fn scale2(a: (f32, f32), scalar: f32) -> (f32, f32) {
    (a.0 * scalar, a.1 * scalar)
}

fn clamp_vec2_length(a: (f32, f32), max_length: f32) -> (f32, f32) {
    let length = (a.0 * a.0 + a.1 * a.1).sqrt();
    if length <= f32::EPSILON || length <= max_length {
        a
    } else {
        scale2(a, max_length / length)
    }
}

fn add4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn sub4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn scale4(a: [f32; 4], scalar: f32) -> [f32; 4] {
    [a[0] * scalar, a[1] * scalar, a[2] * scalar, a[3] * scalar]
}

fn rgba_to_hsla(rgba: [f32; 4]) -> Option<Hsla> {
    if rgba.iter().any(|component| !component.is_finite()) {
        return None;
    }

    Some(Hsla::from(gpui::Rgba {
        r: rgba[0].clamp(0.0, 1.0),
        g: rgba[1].clamp(0.0, 1.0),
        b: rgba[2].clamp(0.0, 1.0),
        a: rgba[3].clamp(0.0, 1.0),
    }))
}

fn rgba_to_pixel(rgba: [f32; 4]) -> Option<PremultipliedColorU8> {
    if rgba.iter().any(|component| !component.is_finite()) {
        return None;
    }
    let alpha = rgba[3].clamp(0.0, 1.0);
    let r = (rgba[0].clamp(0.0, 1.0) * alpha * 255.0).round() as u8;
    let g = (rgba[1].clamp(0.0, 1.0) * alpha * 255.0).round() as u8;
    let b = (rgba[2].clamp(0.0, 1.0) * alpha * 255.0).round() as u8;
    let a = (alpha * 255.0).round() as u8;
    PremultipliedColorU8::from_rgba(b, g, r, a)
}

fn tangent_position_limit(
    points: &[MeshPoint],
    rows: usize,
    cols: usize,
    row: usize,
    col: usize,
    horizontal: bool,
) -> f32 {
    if horizontal {
        if col == 0 {
            distance2(point2(mesh_point(points, row, col)), point2(mesh_point(points, row, col + 1)))
        } else if col + 1 == cols {
            distance2(point2(mesh_point(points, row, col)), point2(mesh_point(points, row, col - 1)))
        } else {
            distance2(point2(mesh_point(points, row, col)), point2(mesh_point(points, row, col - 1)))
                .min(distance2(point2(mesh_point(points, row, col)), point2(mesh_point(points, row, col + 1))))
        }
    } else if row == 0 {
        distance2(point2(mesh_point(points, row, col)), point2(mesh_point(points, row + 1, col)))
    } else if row + 1 == rows {
        distance2(point2(mesh_point(points, row, col)), point2(mesh_point(points, row - 1, col)))
    } else {
        distance2(point2(mesh_point(points, row, col)), point2(mesh_point(points, row - 1, col)))
            .min(distance2(point2(mesh_point(points, row, col)), point2(mesh_point(points, row + 1, col))))
    }
}

fn distance2(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

fn monotone_color_tangent(prev_delta: [f32; 4], next_delta: [f32; 4]) -> [f32; 4] {
    [
        monotone_scalar_tangent(prev_delta[0], next_delta[0]),
        monotone_scalar_tangent(prev_delta[1], next_delta[1]),
        monotone_scalar_tangent(prev_delta[2], next_delta[2]),
        monotone_scalar_tangent(prev_delta[3], next_delta[3]),
    ]
}

fn monotone_scalar_tangent(prev_delta: f32, next_delta: f32) -> f32 {
    if prev_delta.abs() <= f32::EPSILON
        || next_delta.abs() <= f32::EPSILON
        || prev_delta.signum() != next_delta.signum()
    {
        0.0
    } else {
        let average = (prev_delta + next_delta) * 0.5;
        let limit = prev_delta.abs().min(next_delta.abs());
        average.clamp(-limit, limit)
    }
}

fn barycentric_weights(sample: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> Option<(f32, f32, f32)> {
    let denominator = (b.1 - c.1) * (a.0 - c.0) + (c.0 - b.0) * (a.1 - c.1);
    if denominator.abs() <= f32::EPSILON {
        return None;
    }

    let w0 = ((b.1 - c.1) * (sample.0 - c.0) + (c.0 - b.0) * (sample.1 - c.1)) / denominator;
    let w1 = ((c.1 - a.1) * (sample.0 - c.0) + (a.0 - c.0) * (sample.1 - c.1)) / denominator;
    let w2 = 1.0 - w0 - w1;
    Some((w0, w1, w2))
}

fn paint_mesh_guides(pixmap: &mut Pixmap, points: &[MeshPoint], rows: usize, cols: usize, width: f32, height: f32) {
    let stroke = Stroke { width: (width.min(height) * 0.004).max(1.5), ..Stroke::default() };
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 96);
    paint.anti_alias = true;

    for row in 0..rows {
        for col in 0..(cols - 1) {
            let a = mesh_point(points, row, col);
            let b = mesh_point(points, row, col + 1);
            if let Some(path) = line_path(a.u * width, a.v * height, b.u * width, b.v * height) {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    for col in 0..cols {
        for row in 0..(rows - 1) {
            let a = mesh_point(points, row, col);
            let b = mesh_point(points, row + 1, col);
            if let Some(path) = line_path(a.u * width, a.v * height, b.u * width, b.v * height) {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }
}

fn line_path(x0: f32, y0: f32, x1: f32, y1: f32) -> Option<tiny_skia::Path> {
    let mut path = PathBuilder::new();
    path.move_to(x0, y0);
    path.line_to(x1, y1);
    path.finish()
}

#[allow(dead_code)]
fn _paint_mesh_debug_overlay(_window: &mut Window, _bounds: Bounds<Pixels>, _corner_radii: Corners<Pixels>) {
    let _ = px(0.0);
}
