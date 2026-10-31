use gpui::{Hsla, Rgba};
use gpui_luma::color::gpui_bridge;

pub fn interpolate_rgb(start: Hsla, end: Hsla, t: f32) -> Hsla {
    let start_rgba = gpui_bridge::preview_rgba(start);
    let end_rgba = gpui_bridge::preview_rgba(end);

    let r = start_rgba.r + (end_rgba.r - start_rgba.r) * t;
    let g = start_rgba.g + (end_rgba.g - start_rgba.g) * t;
    let b = start_rgba.b + (end_rgba.b - start_rgba.b) * t;
    let a = start_rgba.a + (end_rgba.a - start_rgba.a) * t;

    gpui_bridge::preview_hsla(Rgba { r, g, b, a })
}

pub fn interpolate_hsl(start: Hsla, end: Hsla, t: f32) -> Hsla {
    // Hue interpolation needs to handle the wrap-around
    let mut h1 = start.h;
    let mut h2 = end.h;

    let dh = h2 - h1;
    if dh > 0.5 {
        h1 += 1.0;
    } else if dh < -0.5 {
        h2 += 1.0;
    }

    let h = (h1 + (h2 - h1) * t) % 1.0;
    let s = start.s + (end.s - start.s) * t;
    let l = start.l + (end.l - start.l) * t;
    let a = start.a + (end.a - start.a) * t;

    gpui::hsla(h, s, l, a)
}

/// Interpolate prepared sRGB previews through Palette D65 Lab.
pub fn interpolate_lab(start: Hsla, end: Hsla, t: f32) -> Hsla {
    let start_rgb = gpui_bridge::preview_rgba(start);
    let end_rgb = gpui_bridge::preview_rgba(end);

    let (l1, a1, b1) = super::rgb_to_lab(start_rgb);
    let (l2, a2, b2) = super::rgb_to_lab(end_rgb);

    let l = l1 + (l2 - l1) * t;
    let a = a1 + (a2 - a1) * t;
    let b = b1 + (b2 - b1) * t;
    let alpha = start_rgb.a + (end_rgb.a - start_rgb.a) * t;

    gpui_bridge::preview_hsla(super::lab_to_rgb(l, a, b, alpha))
}

/// Interpolate source colors in an explicitly selected space, before preview mapping.
/// Alpha is straight and interpolated separately; this is a UI gradient operation.
pub fn interpolate_source(
    start: gpui_luma::color::ColorValue,
    end: gpui_luma::color::ColorValue,
    t: f32,
    space: super::super::types::ColorInterpolation,
) -> anyhow::Result<gpui_luma::color::ColorValue> {
    use gpui_luma::color::ColorValue;
    use super::{ColorSpecification, Hsl, Lab};
    start.validate()?;
    end.validate()?;
    anyhow::ensure!(t.is_finite() && (0.0..=1.0).contains(&t), "gradient position must be in 0..=1");
    if t == 0.0 {
        return Ok(start);
    }
    if t == 1.0 {
        return Ok(end);
    }
    let mix = |a: f32, b: f32| a + (b - a) * t;
    let result = match space {
        super::super::types::ColorInterpolation::Rgb => {
            let a = start.to_srgba_unclamped()?;
            let b = end.to_srgba_unclamped()?;
            ColorValue::srgb(mix(a.red, b.red), mix(a.green, b.green), mix(a.blue, b.blue), mix(a.alpha, b.alpha))
        }
        super::super::types::ColorInterpolation::Hsl => {
            let a = Hsl::from_color_value(start)?;
            let b = Hsl::from_color_value(end)?;
            let mut delta = b.h.rem_euclid(360.0) - a.h.rem_euclid(360.0);
            // Preserve the existing HSL half-turn tie: both directions pass through 90°.
            if delta > 180.0 {
                delta -= 360.0;
            } else if delta < -180.0 {
                delta += 360.0;
            }
            Hsl { h: (a.h + delta * t).rem_euclid(360.0), s: mix(a.s, b.s), l: mix(a.l, b.l), a: mix(a.a, b.a) }
                .to_color_value()
        }
        super::super::types::ColorInterpolation::Lab => {
            let a = Lab::from_color_value(start)?;
            let b = Lab::from_color_value(end)?;
            Lab {
                l: mix(a.l, b.l),
                a: mix(a.a, b.a),
                b: mix(a.b, b.b),
                alpha: mix(a.alpha, b.alpha),
                auto_clamp: false,
                dynamic_range: false,
            }
            .to_color_value()
        }
    };
    result.validate()?;
    Ok(result)
}
