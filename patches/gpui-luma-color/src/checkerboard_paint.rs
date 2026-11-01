use gpui::*;

pub(crate) const DEFAULT_CHECKERBOARD_SQUARE_SIZE: Pixels = px(8.0);

/// Paints alternating checkerboard squares clipped to a rounded rectangle.
pub(crate) fn paint_masked_checkerboard(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    radius: Pixels,
    color: Hsla,
    odd_parity: bool,
    square_size: Pixels,
) {
    let rows = (bounds.size.height / square_size).ceil() as i32;
    let cols = (bounds.size.width / square_size).ceil() as i32;

    for row in 0..rows {
        for col in 0..cols {
            let is_odd = (row + col) % 2 == 1;
            if is_odd != odd_parity {
                continue;
            }

            let origin = bounds.origin + point(square_size * (col as f32), square_size * (row as f32));
            let sq_w = square_size.min(bounds.size.width - square_size * col as f32);
            let sq_h = square_size.min(bounds.size.height - square_size * row as f32);
            let square_bounds = Bounds { origin, size: size(sq_w, sq_h) };

            if !is_square_outside_rounded_rect(square_bounds, bounds, radius) {
                window.paint_quad(PaintQuad {
                    bounds: square_bounds,
                    corner_radii: Corners::default(),
                    background: color.into(),
                    border_widths: Edges::default(),
                    border_color: transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }
        }
    }
}

/// Returns `true` when any corner of `square` lies outside the rounded rect.
pub(crate) fn is_square_outside_rounded_rect(square: Bounds<Pixels>, rect: Bounds<Pixels>, radius: Pixels) -> bool {
    let r_f32 = radius.as_f32();
    if r_f32 <= 0.0 {
        return false;
    }

    let left = rect.origin.x.as_f32();
    let top = rect.origin.y.as_f32();
    let right = (rect.origin.x + rect.size.width).as_f32();
    let bottom = (rect.origin.y + rect.size.height).as_f32();

    let sq_left = square.origin.x.as_f32();
    let sq_top = square.origin.y.as_f32();
    let sq_right = (square.origin.x + square.size.width).as_f32();
    let sq_bottom = (square.origin.y + square.size.height).as_f32();

    let corners = [(sq_left, sq_top), (sq_right, sq_top), (sq_left, sq_bottom), (sq_right, sq_bottom)];

    for &(x, y) in &corners {
        {
            let cx = left + r_f32;
            let cy = top + r_f32;
            if x < cx && y < cy {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy > r_f32 * r_f32 {
                    return true;
                }
            }
        }

        {
            let cx = right - r_f32;
            let cy = top + r_f32;
            if x > cx && y < cy {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy > r_f32 * r_f32 {
                    return true;
                }
            }
        }

        {
            let cx = left + r_f32;
            let cy = bottom - r_f32;
            if x < cx && y > cy {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy > r_f32 * r_f32 {
                    return true;
                }
            }
        }

        {
            let cx = right - r_f32;
            let cy = bottom - r_f32;
            if x > cx && y > cy {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy > r_f32 * r_f32 {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds { origin: point(px(x), px(y)), size: size(px(w), px(h)) }
    }

    #[::core::prelude::v1::test]
    fn square_fully_inside_rounded_rect_is_kept() {
        let rect = bounds(0.0, 0.0, 32.0, 16.0);
        let square = bounds(8.0, 0.0, 8.0, 8.0);
        assert!(!is_square_outside_rounded_rect(square, rect, px(8.0)));
    }

    #[::core::prelude::v1::test]
    fn square_corner_outside_rounded_rect_is_rejected() {
        let rect = bounds(0.0, 0.0, 32.0, 16.0);
        let square = bounds(0.0, 0.0, 8.0, 8.0);
        assert!(is_square_outside_rounded_rect(square, rect, px(8.0)));
    }
}
