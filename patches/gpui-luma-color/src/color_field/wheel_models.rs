#![allow(dead_code)]

use super::model::{ColorFieldModel2D, ColorFieldModelKind};
use crate::color_slider::color_spec::Hsv;
use gpui::Hsla;
use gpui_luma::color::gpui_bridge;
use palette::convert::FromColorUnclamped;
use std::f32::consts::TAU;

const WHITE_MIX_HUE_WHEEL_CACHE_KEY: u64 = 0x1001;
const HSL_WHEEL_CACHE_KEY: u64 = 0x1002;
const GAMMA_HSV_WHEEL_CACHE_KEY: u64 = 0x1003;
const OKLCH_WHEEL_CACHE_KEY: u64 = 0x1004;

macro_rules! impl_wheel_model {
    ($model:ty, $cache_key:expr, |$hsv:ident, $uv:ident| $color_expr:expr) => {
        impl ColorFieldModel2D for $model {
            fn apply_uv(&self, hsv: &mut Hsv, uv: (f32, f32)) {
                apply_wheel_uv(hsv, uv);
            }

            fn uv_from_hsv(&self, hsv: &Hsv) -> (f32, f32) {
                wheel_uv_from_hs(hsv.h, hsv.s)
            }

            fn color_at_uv(&self, $hsv: &Hsv, $uv: (f32, f32)) -> Hsla {
                $color_expr
            }

            fn cache_key_part(&self) -> u64 {
                $cache_key
            }

            fn kind(&self) -> ColorFieldModelKind {
                ColorFieldModelKind::HueSaturationWheel
            }

            fn thumb_color(&self, hsv: &Hsv) -> Hsla {
                self.color_at_uv(hsv, self.uv_from_hsv(hsv))
            }
        }
    };
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WhiteMixHueWheelModel;

impl_wheel_model!(WhiteMixHueWheelModel, WHITE_MIX_HUE_WHEEL_CACHE_KEY, |_hsv, uv| {
    let (hue, saturation) = wheel_hs_from_uv(uv);
    let hue_rgb = gpui_bridge::preview_rgba(gpui_bridge::from_palette_hsla(palette::Hsla::new(hue, 1.0, 0.5, 1.0)));
    let sat = saturation.clamp(0.0, 1.0);

    rgba_to_hsla(
        1.0 * (1.0 - sat) + hue_rgb.r * sat,
        1.0 * (1.0 - sat) + hue_rgb.g * sat,
        1.0 * (1.0 - sat) + hue_rgb.b * sat,
        1.0,
    )
});

#[derive(Clone, Copy, Debug, Default)]
pub struct HslWheelModel;

impl_wheel_model!(HslWheelModel, HSL_WHEEL_CACHE_KEY, |hsv, uv| {
    let (hue, saturation) = wheel_hs_from_uv(uv);
    gpui_bridge::from_palette_hsla(palette::Hsla::new(hue, saturation, hsv.v.clamp(0.0, 1.0), hsv.a.clamp(0.0, 1.0)))
});

#[derive(Clone, Copy, Debug, Default)]
pub struct GammaCorrectedHsvWheelModel;

impl_wheel_model!(GammaCorrectedHsvWheelModel, GAMMA_HSV_WHEEL_CACHE_KEY, |_hsv, uv| {
    let (hue, saturation) = wheel_hs_from_uv(uv);
    let hue_rgb = gpui_bridge::preview_rgba(Hsv { h: hue, s: 1.0, v: 1.0, a: 1.0 }.to_hsla_ext());
    let sat = saturation.clamp(0.0, 1.0);

    let white_linear = 1.0;
    let r_linear = white_linear * (1.0 - sat) + srgb_to_linear(hue_rgb.r) * sat;
    let g_linear = white_linear * (1.0 - sat) + srgb_to_linear(hue_rgb.g) * sat;
    let b_linear = white_linear * (1.0 - sat) + srgb_to_linear(hue_rgb.b) * sat;

    rgba_to_hsla(linear_to_srgb(r_linear), linear_to_srgb(g_linear), linear_to_srgb(b_linear), 1.0)
});

#[derive(Clone, Copy, Debug, Default)]
pub struct OklchWheelModel;

impl_wheel_model!(OklchWheelModel, OKLCH_WHEEL_CACHE_KEY, |hsv, uv| {
    let (hue, saturation) = wheel_hs_from_uv(uv);
    let sat = saturation.clamp(0.0, 1.0);
    let rim_lightness = (0.35 + hsv.v.clamp(0.0, 1.0) * 0.5).clamp(0.0, 1.0);
    let l = 1.0 - sat * (1.0 - rim_lightness);
    let c = sat * 0.33;
    let (r, g, b) = oklch_to_srgb_gamut_mapped(l, c, hue);
    rgba_to_hsla(r, g, b, hsv.a.clamp(0.0, 1.0))
});

fn apply_wheel_uv(hsv: &mut Hsv, uv: (f32, f32)) {
    let (hue, saturation) = wheel_hs_from_uv(uv);
    hsv.h = hue;
    hsv.s = saturation;
}

fn wheel_hs_from_uv(uv: (f32, f32)) -> (f32, f32) {
    let dx = uv.0 - 0.5;
    let dy = 0.5 - uv.1;
    let hue = dy.atan2(dx).rem_euclid(TAU).to_degrees();
    let saturation = ((dx * dx + dy * dy).sqrt() / 0.5).clamp(0.0, 1.0);
    (hue, saturation)
}

fn wheel_uv_from_hs(hue_degrees: f32, saturation: f32) -> (f32, f32) {
    let angle = hue_degrees.rem_euclid(360.0).to_radians();
    let radius = saturation.clamp(0.0, 1.0) * 0.5;
    ((0.5 + radius * angle.cos()).clamp(0.0, 1.0), (0.5 - radius * angle.sin()).clamp(0.0, 1.0))
}

fn rgba_to_hsla(r: f32, g: f32, b: f32, a: f32) -> Hsla {
    gpui_bridge::preview_hsla(gpui::Rgba {
        r: r.clamp(0.0, 1.0),
        g: g.clamp(0.0, 1.0),
        b: b.clamp(0.0, 1.0),
        a: a.clamp(0.0, 1.0),
    })
}

fn srgb_to_linear(v: f32) -> f32 {
    palette::Srgb::new(v.clamp(0.0, 1.0), 0.0, 0.0).into_linear().red
}

fn linear_to_srgb(v: f32) -> f32 {
    palette::Srgb::from_linear(palette::LinSrgb::new(v.clamp(0.0, 1.0), 0.0, 0.0)).red
}

// Keep the existing wheel boundary-chroma policy; this only prepares its sRGB preview.
fn oklch_to_srgb_gamut_mapped(l: f32, c: f32, h_degrees: f32) -> (f32, f32, f32) {
    if let Some(rgb) = oklch_to_srgb_if_in_gamut(l, c, h_degrees) {
        return rgb;
    }

    let mut lo = 0.0;
    let mut hi = c.max(0.0);
    let mut best = (1.0, 1.0, 1.0);

    for _ in 0..12 {
        let mid = (lo + hi) * 0.5;
        if let Some(rgb) = oklch_to_srgb_if_in_gamut(l, mid, h_degrees) {
            lo = mid;
            best = rgb;
        } else {
            hi = mid;
        }
    }

    best
}

fn oklch_to_srgb_if_in_gamut(l: f32, c: f32, h_degrees: f32) -> Option<(f32, f32, f32)> {
    let rgb = palette::LinSrgb::from_color_unclamped(palette::Oklch::new(l, c, h_degrees));
    let (r_linear, g_linear, b_linear) = (rgb.red, rgb.green, rgb.blue);

    if !(0.0..=1.0).contains(&r_linear) || !(0.0..=1.0).contains(&g_linear) || !(0.0..=1.0).contains(&b_linear) {
        return None;
    }

    Some((
        linear_to_srgb(r_linear).clamp(0.0, 1.0),
        linear_to_srgb(g_linear).clamp(0.0, 1.0),
        linear_to_srgb(b_linear).clamp(0.0, 1.0),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-5, "expected {a} ~= {b}, delta={}", (a - b).abs());
    }

    #[test]
    fn wheel_uv_round_trip_is_stable() {
        let uv = wheel_uv_from_hs(210.0, 0.7);
        let (h, s) = wheel_hs_from_uv(uv);
        approx_eq(h, 210.0);
        approx_eq(s, 0.7);
    }

    #[test]
    fn model_cache_keys_are_unique() {
        assert_ne!(WHITE_MIX_HUE_WHEEL_CACHE_KEY, HSL_WHEEL_CACHE_KEY);
        assert_ne!(WHITE_MIX_HUE_WHEEL_CACHE_KEY, GAMMA_HSV_WHEEL_CACHE_KEY);
        assert_ne!(WHITE_MIX_HUE_WHEEL_CACHE_KEY, OKLCH_WHEEL_CACHE_KEY);
        assert_ne!(HSL_WHEEL_CACHE_KEY, GAMMA_HSV_WHEEL_CACHE_KEY);
        assert_ne!(HSL_WHEEL_CACHE_KEY, OKLCH_WHEEL_CACHE_KEY);
        assert_ne!(GAMMA_HSV_WHEEL_CACHE_KEY, OKLCH_WHEEL_CACHE_KEY);
    }

    #[test]
    fn gamma_corrected_center_is_white() {
        let model = GammaCorrectedHsvWheelModel;
        let color = model.color_at_uv(&Hsv { h: 0.0, s: 0.0, v: 0.5, a: 1.0 }, (0.5, 0.5));
        let rgb = color.to_rgb();
        approx_eq(rgb.r, 1.0);
        approx_eq(rgb.g, 1.0);
        approx_eq(rgb.b, 1.0);
    }

    #[test]
    fn palette_conversion_matches_existing_wheel_samples() {
        for (l, c, h, expected) in [
            (0.7, 0.1, 45.0, [0.82511896, 0.54151577, 0.41329536]),
            (0.5, 0.08, 240.0, [0.20278569, 0.4117124, 0.5499151]),
            (0.8, 0.05, 120.0, [0.72615576, 0.7646376, 0.6243464]),
        ] {
            let (r, g, b) = oklch_to_srgb_if_in_gamut(l, c, h).unwrap();
            for (actual, expected) in [r, g, b].into_iter().zip(expected) {
                approx_eq(actual, expected);
            }
        }
    }

    #[test]
    fn oklch_gamut_mapping_stays_bounded() {
        let (r, g, b) = oklch_to_srgb_gamut_mapped(0.7, 1.2, 45.0);
        assert!((0.0..=1.0).contains(&r));
        assert!((0.0..=1.0).contains(&g));
        assert!((0.0..=1.0).contains(&b));
    }
}
