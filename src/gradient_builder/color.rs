use gpui::{Hsla, Rgba, hsla};
use gpui_luma_color::color_slider::color_spec::Hsv as SdkHsv;
use palette::{FromColor, Hsl, Hsv as PaletteHsv, Mix, Srgb, Srgba};

use super::paint::GradientType;

pub fn format_hex_color(color: Hsla) -> String {
    let srgb: Srgb = Srgb::from_color(hsla_to_srgba(color));
    format!(
        "#{:02x}{:02x}{:02x}",
        (srgb.red * 255.0).round() as u8,
        (srgb.green * 255.0).round() as u8,
        (srgb.blue * 255.0).round() as u8,
    )
}

#[allow(dead_code)]
pub fn parse_hex_color(raw: &str) -> Option<Hsla> {
    let hex = raw.trim().trim_start_matches('#');
    let rgba = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Srgba::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Srgba::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
        }
        _ => return None,
    };
    Some(srgba_to_hsla(rgba))
}

pub fn interpolate_rgb(start: Hsla, end: Hsla, t: f32) -> Hsla {
    let mixed = hsla_to_srgba(start).mix(hsla_to_srgba(end), t.clamp(0.0, 1.0));
    srgba_to_hsla(mixed)
}

pub fn hsla_to_sdk_hsv(color: Hsla) -> SdkHsv {
    let palette_hsv = PaletteHsv::from_color(hsla_to_srgba(color));
    SdkHsv {
        h: palette_hsv.hue.into_positive_degrees(),
        s: palette_hsv.saturation,
        v: palette_hsv.value,
        a: color.a,
    }
}

pub fn sdk_hsv_to_hsla(hsv: SdkHsv) -> Hsla {
    let palette_hsv = PaletteHsv::new(hsv.h, hsv.s, hsv.v);
    let srgb: Srgb = Srgb::from_color(palette_hsv);
    Rgba { r: srgb.red, g: srgb.green, b: srgb.blue, a: hsv.a }.into()
}

pub fn format_percent(position: f32) -> String {
    format!("{}%", (position * 100.0).round() as i32)
}

pub fn format_css_linear_gradient(stops: &[(f32, Hsla)], rotation_deg: f32) -> String {
    if stops.is_empty() {
        return "linear-gradient(90deg, transparent 0%, transparent 100%)".to_string();
    }

    let stop_list = stops
        .iter()
        .map(|(position, color)| format!("{} {}", format_hex_color(*color), format_percent(*position)))
        .collect::<Vec<_>>()
        .join(", ");

    format!("linear-gradient({}deg, {stop_list})", rotation_deg.round() as i32)
}

pub fn format_css_radial_gradient(stops: &[(f32, Hsla)]) -> String {
    if stops.is_empty() {
        return "radial-gradient(circle, transparent 0%, transparent 100%)".to_string();
    }

    let stop_list = stops
        .iter()
        .map(|(position, color)| format!("{} {}", format_hex_color(*color), format_percent(*position)))
        .collect::<Vec<_>>()
        .join(", ");

    format!("radial-gradient(circle, {stop_list})")
}

pub fn format_css_angular_gradient(stops: &[(f32, Hsla)], rotation_deg: f32) -> String {
    if stops.is_empty() {
        return format!("conic-gradient(from {}deg, transparent 0%, transparent 100%)", rotation_deg.round() as i32);
    }

    let stop_list = stops
        .iter()
        .map(|(position, color)| format!("{} {}", format_hex_color(*color), format_percent(*position)))
        .collect::<Vec<_>>()
        .join(", ");

    format!("conic-gradient(from {}deg, {stop_list})", rotation_deg.round() as i32)
}

pub fn format_css_gradient(gradient_type: GradientType, stops: &[(f32, Hsla)], rotation_deg: f32) -> String {
    match gradient_type {
        GradientType::Linear => format_css_linear_gradient(stops, rotation_deg),
        GradientType::Radial => format_css_radial_gradient(stops),
        GradientType::Angular => format_css_angular_gradient(stops, rotation_deg),
    }
}

fn hsla_to_srgba(color: Hsla) -> Srgba {
    let rgb = color.to_rgb();
    Srgba::new(rgb.r, rgb.g, rgb.b, color.a)
}

fn srgba_to_hsla(rgba: Srgba) -> Hsla {
    let hsl: Hsl = Hsl::from_color(rgba);
    hsla(hsl.hue.into_positive_degrees() / 360.0, hsl.saturation, hsl.lightness, rgba.alpha)
}
