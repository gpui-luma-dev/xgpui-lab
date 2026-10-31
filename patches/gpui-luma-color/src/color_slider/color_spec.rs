use gpui_luma::color::{ColorValue, GamutMapping, gpui_bridge};
use anyhow::Result;
use palette::convert::FromColorUnclamped;
use gpui::{Hsla, Rgba};

mod interpolation;
pub use interpolation::{interpolate_hsl, interpolate_lab, interpolate_rgb, interpolate_source};
pub use super::oklch_spec::Oklch;

#[cfg(test)]
mod tests;

pub mod constants {
    /// The size of the checkerboard squares for alpha slider tracks.
    pub const CHECKERBOARD_SIZE: f32 = 4.0;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorChannel {
    pub name: &'static str,
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub step: Option<f32>,
    pub unit: &'static str,
}

/// Resolves the slider step for a color channel, using a fine default for unit-range channels.
pub fn slider_step_for_channel(channel: &ColorChannel) -> f32 {
    channel.step.unwrap_or_else(|| {
        let span = channel.max - channel.min;
        if span <= 1.0 { 0.01 } else { 1.0 }
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HueAlpha {
    pub h: f32,
    pub a: f32,
}

impl HueAlpha {
    pub const HUE: &'static str = "hue";
    pub const ALPHA: &'static str = "alpha";

    const CHANNELS: [ColorChannel; 2] = [
        ColorChannel { name: Self::HUE, label: "Hue", min: 0.0, max: 360.0, step: Some(1.0), unit: "°" },
        ColorChannel { name: Self::ALPHA, label: "Alpha", min: 0.0, max: 1.0, step: None, unit: "" },
    ];
}

impl ColorSpecification for HueAlpha {
    fn to_color_value(&self) -> ColorValue {
        ColorValue::Srgb(palette::Srgba::from_color_unclamped(palette::Hsla::new(self.h, 1.0, 0.5, self.a)))
    }
    fn from_color_value(source: ColorValue) -> Result<Self> {
        let hsl = palette::Hsla::from_color_unclamped(source.to_srgba_unclamped()?);
        Ok(Self { h: hsl.hue.into_raw_degrees(), a: hsl.alpha })
    }

    fn name(&self) -> &'static str {
        "Hue+Alpha"
    }

    fn channels(&self) -> &[ColorChannel] {
        &Self::CHANNELS
    }

    fn get_value(&self, channel_name: &str) -> f32 {
        match channel_name {
            Self::HUE => self.h,
            Self::ALPHA => self.a,
            _ => 0.0,
        }
    }

    fn set_value(&mut self, channel_name: &str, value: f32) {
        match channel_name {
            Self::HUE => self.h = value.clamp(0.0, 360.0),
            Self::ALPHA => self.a = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn to_hsla(&self) -> Hsla {
        gpui_bridge::from_palette_hsla(palette::Hsla::new(self.h, 1.0, 0.5, self.a))
    }

    fn from_hsla(hsla: Hsla) -> Self {
        let color = gpui_bridge::to_palette_hsla(hsla);
        Self { h: color.hue.into_raw_degrees(), a: color.alpha }
    }

    fn uses_rainbow_hue_track(&self) -> bool {
        true
    }
}

pub trait ColorSpecification: 'static + Clone + Copy + Send + Sync {
    #[allow(dead_code)]
    fn name(&self) -> &'static str;
    fn channels(&self) -> &[ColorChannel];
    #[allow(dead_code)]
    fn get_value(&self, channel_name: &str) -> f32;
    fn set_value(&mut self, channel_name: &str, value: f32);
    /// Source export without gamut mapping. Lab uses D65 and exports extended linear sRGB.
    fn to_color_value(&self) -> ColorValue;
    /// Explicit space conversion without gamut mapping; keep the original source for identity.
    fn from_color_value(source: ColorValue) -> Result<Self>;
    /// Current backend preview only.
    fn to_hsla(&self) -> Hsla;
    fn from_hsla(hsla: Hsla) -> Self;

    #[allow(dead_code)]
    fn format_value(&self, channel_name: &str) -> String {
        let value = self.get_value(channel_name);
        format!("{:.3}", value)
    }

    // By default, spaces are continuous RGB-bound so this is just their static bounds
    fn channel_bounds(&self, channel_name: &str) -> (f32, f32) {
        let channel = self.channels().iter().find(|c| c.name == channel_name).unwrap();
        (channel.min, channel.max)
    }

    /// Optional sub-range within [`Self::channel_bounds`] where thumb interaction is allowed.
    /// When set, the domain track still paints across the full [`Self::channel_bounds`].
    fn channel_allowed_interval(&self, channel_name: &str) -> Option<(f32, f32)> {
        let _ = channel_name;
        None
    }

    /// Allowed interaction intervals within [`Self::channel_bounds`].
    fn channel_allowed_intervals(&self, channel_name: &str) -> Vec<std::ops::RangeInclusive<f32>> {
        match self.channel_allowed_interval(channel_name) {
            Some((min, max)) => vec![min..=max],
            None => Vec::new(),
        }
    }

    /// Intervals rendered as allowed/blocked regions on the track.
    fn channel_track_intervals(&self, channel_name: &str) -> Vec<std::ops::RangeInclusive<f32>> {
        self.channel_allowed_intervals(channel_name)
    }

    /// When true, the slider accepts the full range and defers clamping to the spec.
    fn uses_continuous_channel_interaction(&self, channel_name: &str) -> bool {
        let _ = channel_name;
        false
    }

    /// When true, the hue channel uses the fixed rainbow hue track instead of a spec slice.
    fn uses_rainbow_hue_track(&self) -> bool {
        true
    }

    #[allow(dead_code)]
    fn set_auto_clamp(&mut self, _auto_clamp: bool) {}

    #[allow(dead_code)]
    fn set_dynamic_range(&mut self, _dynamic_range: bool) {}

    fn clamp_spec_to_gamut(&mut self) {}

    fn clamp_channel_to_gamut(&self, channel_name: &str, proposed: f32) -> f32 {
        let bounds = self.channel_bounds(channel_name);
        proposed.clamp(bounds.0, bounds.1)
    }

    fn apply_channel_slider_value(&mut self, channel_name: &str, proposed: f32) {
        let clamped = self.clamp_channel_to_gamut(channel_name, proposed);
        self.set_value(channel_name, clamped);
        self.clamp_spec_to_gamut();
    }

    #[allow(dead_code)]
    fn summary(&self) -> String {
        let alpha = self.get_value("alpha");
        if self.name() == "HSL" {
            format!(
                "hsla({:.0}°, {:.0}%, {:.0}%, {:.2})",
                self.get_value("hue"),
                self.get_value("saturation") * 100.0,
                self.get_value("lightness") * 100.0,
                alpha
            )
        } else if self.name() == "HSV" {
            format!(
                "hsva({:.0}°, {:.0}%, {:.0}%, {:.2})",
                self.get_value("hue"),
                self.get_value("saturation") * 100.0,
                self.get_value("value") * 100.0,
                alpha
            )
        } else if self.name() == "RGBA" {
            format!(
                "rgba({:.0}, {:.0}, {:.0}, {:.2})",
                self.get_value("red"),
                self.get_value("green"),
                self.get_value("blue"),
                alpha
            )
        } else if self.name() == "Lab" {
            format!(
                "lab({:.0}, {:.0}, {:.0}, {:.2})",
                self.get_value("lightness"),
                self.get_value("a"),
                self.get_value("b"),
                alpha
            )
        } else if self.name() == "Hue+Alpha" {
            format!("hue+alpha({:.0}°, {:.2})", self.get_value("hue"), alpha)
        } else if self.name() == "OKLCH" {
            format!(
                "oklch({:.2} {:.3} {:.0} / {:.2})",
                self.get_value("lightness"),
                self.get_value("chroma"),
                self.get_value("hue"),
                alpha
            )
        } else {
            String::new()
        }
    }

    fn is_out_of_gamut(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl {
    pub h: f32, // 0..360
    pub s: f32, // 0..1
    pub l: f32, // 0..1
    pub a: f32, // 0..1
}

impl Hsl {
    pub const HUE: &'static str = "hue";
    pub const SATURATION: &'static str = "saturation";
    pub const LIGHTNESS: &'static str = "lightness";
    pub const ALPHA: &'static str = "alpha";

    const CHANNELS: [ColorChannel; 4] = [
        ColorChannel { name: Self::HUE, label: "Hue", min: 0.0, max: 360.0, step: Some(1.0), unit: "°" },
        ColorChannel { name: Self::SATURATION, label: "Saturation", min: 0.0, max: 1.0, step: None, unit: "%" },
        ColorChannel { name: Self::LIGHTNESS, label: "Lightness", min: 0.0, max: 1.0, step: None, unit: "%" },
        ColorChannel { name: Self::ALPHA, label: "Alpha", min: 0.0, max: 1.0, step: None, unit: "" },
    ];
}

impl ColorSpecification for Hsl {
    fn to_color_value(&self) -> ColorValue {
        ColorValue::Srgb(palette::Srgba::from_color_unclamped(palette::Hsla::new(self.h, self.s, self.l, self.a)))
    }
    fn from_color_value(source: ColorValue) -> Result<Self> {
        let hsl = palette::Hsla::from_color_unclamped(source.to_srgba_unclamped()?);
        Ok(Self { h: hsl.hue.into_raw_degrees(), s: hsl.saturation, l: hsl.lightness, a: hsl.alpha })
    }

    fn name(&self) -> &'static str {
        "HSL"
    }

    fn channels(&self) -> &[ColorChannel] {
        &Self::CHANNELS
    }

    fn get_value(&self, channel_name: &str) -> f32 {
        match channel_name {
            Self::HUE => self.h,
            Self::SATURATION => self.s,
            Self::LIGHTNESS => self.l,
            Self::ALPHA => self.a,
            _ => 0.0,
        }
    }

    fn set_value(&mut self, channel_name: &str, value: f32) {
        match channel_name {
            Self::HUE => self.h = value.clamp(0.0, 360.0),
            Self::SATURATION => self.s = value.clamp(0.0, 1.0),
            Self::LIGHTNESS => self.l = value.clamp(0.0, 1.0),
            Self::ALPHA => self.a = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn to_hsla(&self) -> Hsla {
        gpui_bridge::from_palette_hsla(palette::Hsla::new(self.h, self.s, self.l, self.a))
    }

    fn from_hsla(hsla: Hsla) -> Self {
        let color = gpui_bridge::to_palette_hsla(hsla);
        Self { h: color.hue.into_raw_degrees(), s: color.saturation, l: color.lightness, a: color.alpha }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub h: f32, // 0..360
    pub s: f32, // 0..1
    pub v: f32, // 0..1
    pub a: f32, // 0..1
}

impl Hsv {
    pub const HUE: &'static str = "hue";
    pub const SATURATION: &'static str = "saturation";
    pub const VALUE: &'static str = "value";
    pub const ALPHA: &'static str = "alpha";

    const CHANNELS: [ColorChannel; 4] = [
        ColorChannel { name: Self::HUE, label: "Hue", min: 0.0, max: 360.0, step: Some(1.0), unit: "°" },
        ColorChannel { name: Self::SATURATION, label: "Saturation", min: 0.0, max: 1.0, step: None, unit: "%" },
        ColorChannel { name: Self::VALUE, label: "Value", min: 0.0, max: 1.0, step: None, unit: "%" },
        ColorChannel { name: Self::ALPHA, label: "Alpha", min: 0.0, max: 1.0, step: None, unit: "" },
    ];

    pub fn from_hsla_ext(hsla: Hsla) -> Self {
        let rgba = gpui_bridge::preview_rgba(hsla);
        Self::from_rgba(rgba)
    }

    pub fn from_rgba(rgba: Rgba) -> Self {
        let hsv = palette::Hsva::from_color_unclamped(palette::Srgba::new(rgba.r, rgba.g, rgba.b, rgba.a));
        Self { h: hsv.hue.into_positive_degrees(), s: hsv.saturation, v: hsv.value, a: hsv.alpha }
    }

    pub fn to_hsla_ext(self) -> Hsla {
        gpui_bridge::to_hsla(self.to_color_value(), GamutMapping::Clip).unwrap_or_else(|_| gpui::transparent_black())
    }
}

impl ColorSpecification for Hsv {
    fn to_color_value(&self) -> ColorValue {
        ColorValue::Srgb(palette::Srgba::from_color_unclamped(palette::Hsva::new(self.h, self.s, self.v, self.a)))
    }
    fn from_color_value(source: ColorValue) -> Result<Self> {
        let hsv = palette::Hsva::from_color_unclamped(source.to_srgba_unclamped()?);
        Ok(Self { h: hsv.hue.into_raw_degrees(), s: hsv.saturation, v: hsv.value, a: hsv.alpha })
    }

    fn name(&self) -> &'static str {
        "HSV"
    }

    fn channels(&self) -> &[ColorChannel] {
        &Self::CHANNELS
    }

    fn get_value(&self, channel_name: &str) -> f32 {
        match channel_name {
            Self::HUE => self.h,
            Self::SATURATION => self.s,
            Self::VALUE => self.v,
            Self::ALPHA => self.a,
            _ => 0.0,
        }
    }

    fn set_value(&mut self, channel_name: &str, value: f32) {
        match channel_name {
            Self::HUE => self.h = value.clamp(0.0, 360.0),
            Self::SATURATION => self.s = value.clamp(0.0, 1.0),
            Self::VALUE => self.v = value.clamp(0.0, 1.0),
            Self::ALPHA => self.a = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn to_hsla(&self) -> Hsla {
        self.to_hsla_ext()
    }

    fn from_hsla(hsla: Hsla) -> Self {
        Self::from_hsla_ext(hsla)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RgbaSpec {
    pub r: f32, // 0..255
    pub g: f32, // 0..255
    pub b: f32, // 0..255
    pub a: f32, // 0..1
}

impl RgbaSpec {
    pub const RED: &'static str = "red";
    pub const GREEN: &'static str = "green";
    pub const BLUE: &'static str = "blue";
    pub const ALPHA: &'static str = "alpha";

    const CHANNELS: [ColorChannel; 4] = [
        ColorChannel { name: Self::RED, label: "Red", min: 0.0, max: 255.0, step: Some(1.0), unit: "" },
        ColorChannel { name: Self::GREEN, label: "Green", min: 0.0, max: 255.0, step: Some(1.0), unit: "" },
        ColorChannel { name: Self::BLUE, label: "Blue", min: 0.0, max: 255.0, step: Some(1.0), unit: "" },
        ColorChannel { name: Self::ALPHA, label: "Alpha", min: 0.0, max: 1.0, step: None, unit: "" },
    ];
}

impl ColorSpecification for RgbaSpec {
    fn to_color_value(&self) -> ColorValue {
        ColorValue::srgb(self.r / 255.0, self.g / 255.0, self.b / 255.0, self.a)
    }
    fn from_color_value(source: ColorValue) -> Result<Self> {
        let rgb = source.to_srgba_unclamped()?;
        Ok(Self { r: rgb.red * 255.0, g: rgb.green * 255.0, b: rgb.blue * 255.0, a: rgb.alpha })
    }

    fn name(&self) -> &'static str {
        "RGBA"
    }

    fn channels(&self) -> &[ColorChannel] {
        &Self::CHANNELS
    }

    fn get_value(&self, channel_name: &str) -> f32 {
        match channel_name {
            Self::RED => self.r,
            Self::GREEN => self.g,
            Self::BLUE => self.b,
            Self::ALPHA => self.a,
            _ => 0.0,
        }
    }

    fn set_value(&mut self, channel_name: &str, value: f32) {
        match channel_name {
            Self::RED => self.r = value.clamp(0.0, 255.0),
            Self::GREEN => self.g = value.clamp(0.0, 255.0),
            Self::BLUE => self.b = value.clamp(0.0, 255.0),
            Self::ALPHA => self.a = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn to_hsla(&self) -> Hsla {
        gpui_bridge::to_hsla(self.to_color_value(), GamutMapping::Clip).unwrap_or_else(|_| gpui::transparent_black())
    }

    fn from_hsla(hsla: Hsla) -> Self {
        let rgba = gpui_bridge::preview_rgba(hsla);
        Self { r: rgba.r * 255.0, g: rgba.g * 255.0, b: rgba.b * 255.0, a: rgba.a }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lab {
    pub l: f32,     // 0..100
    pub a: f32,     // -128..127
    pub b: f32,     // -128..127
    pub alpha: f32, // 0..1
    pub auto_clamp: bool,
    pub dynamic_range: bool,
}

impl Lab {
    pub const LIGHTNESS: &'static str = "lightness";
    pub const A: &'static str = "a";
    pub const B: &'static str = "b";
    pub const ALPHA: &'static str = "alpha";

    const CHANNELS: [ColorChannel; 4] = [
        ColorChannel {
            name: Self::LIGHTNESS,
            label: "Lightness (L*)",
            min: 0.0,
            max: 100.0,
            step: Some(1.0),
            unit: "",
        },
        ColorChannel { name: Self::A, label: "Green-Red (a*)", min: -128.0, max: 127.0, step: Some(1.0), unit: "" },
        ColorChannel { name: Self::B, label: "Blue-Yellow (b*)", min: -128.0, max: 127.0, step: Some(1.0), unit: "" },
        ColorChannel { name: Self::ALPHA, label: "Alpha", min: 0.0, max: 1.0, step: None, unit: "" },
    ];

    pub fn to_hsla_checked(&self) -> (Hsla, bool) {
        let (rgba, out_of_gamut) = lab_to_rgb_checked(self.l, self.a, self.b, self.alpha);
        (gpui_bridge::preview_hsla(rgba), out_of_gamut)
    }

    // Non-trait Math Helpers originally from LabMixer

    const CLAMP_SEARCH_STEPS: usize = 96;
    const CLAMP_REFINE_STEPS: usize = 14;

    fn spec_in_gamut(spec: Lab) -> bool {
        !spec.to_hsla_checked().1
    }

    fn channel_bounds_math(axis: &str) -> (f32, f32) {
        match axis {
            Self::LIGHTNESS => (0.0, 100.0),
            Self::A | Self::B => (-128.0, 127.0),
            _ => (0.0, 1.0),
        }
    }

    fn clamp_channel_to_gamut_math(base_spec: Lab, axis_name: &str, proposed: f32) -> f32 {
        if axis_name == Self::ALPHA {
            return proposed.clamp(0.0, 1.0);
        }
        let (min, max) = Self::channel_bounds_math(axis_name);
        let proposed = proposed.clamp(min, max);
        let mut proposed_spec = base_spec;
        proposed_spec.set_value(axis_name, proposed);
        if Self::spec_in_gamut(proposed_spec) {
            return proposed;
        }

        let mut anchor = base_spec.get_value(axis_name).clamp(min, max);
        let mut anchor_spec = base_spec;
        anchor_spec.set_value(axis_name, anchor);

        // Fallback for unexpected states where base_spec is already out of gamut.
        if !Self::spec_in_gamut(anchor_spec) {
            let step = (max - min) / Self::CLAMP_SEARCH_STEPS as f32;
            let mut best = None::<(f32, f32)>;
            for i in 0..=Self::CLAMP_SEARCH_STEPS {
                let sample_value = if i == Self::CLAMP_SEARCH_STEPS {
                    max
                } else {
                    min + step * i as f32
                };
                let mut sample_spec = base_spec;
                sample_spec.set_value(axis_name, sample_value);
                if Self::spec_in_gamut(sample_spec) {
                    let dist = (sample_value - proposed).abs();
                    match best {
                        Some((_, best_dist)) if best_dist <= dist => {}
                        _ => best = Some((sample_value, dist)),
                    }
                }
            }
            let Some((best_anchor, _)) = best else {
                return proposed;
            };
            anchor = best_anchor;
        }

        if (proposed - anchor).abs() <= f32::EPSILON {
            return anchor;
        }

        let mut in_gamut = anchor;
        let mut out_of_gamut = proposed;
        for _ in 0..Self::CLAMP_REFINE_STEPS {
            let mid = (in_gamut + out_of_gamut) * 0.5;
            let mut sample = base_spec;
            sample.set_value(axis_name, mid);
            if Self::spec_in_gamut(sample) {
                in_gamut = mid;
            } else {
                out_of_gamut = mid;
            }
        }
        in_gamut.clamp(min, max)
    }

    fn gamut_interval_around(base_spec: Lab, axis_name: &str) -> (f32, f32) {
        if axis_name == Self::ALPHA {
            return (0.0, 1.0);
        }
        let (min, max) = Self::channel_bounds_math(axis_name);
        let mut current = base_spec.get_value(axis_name).clamp(min, max);
        let mut current_spec = base_spec;
        current_spec.set_value(axis_name, current);

        if !Self::spec_in_gamut(current_spec) {
            current = Self::clamp_channel_to_gamut_math(base_spec, axis_name, current);
            current_spec.set_value(axis_name, current);
            if !Self::spec_in_gamut(current_spec) {
                return (min, max);
            }
        }

        let step = (max - min) / Self::CLAMP_SEARCH_STEPS as f32;

        let mut lower_in = current;
        let mut probe = current;
        let mut lower_found = false;
        while probe > min {
            let next = (probe - step).max(min);
            let mut sample = base_spec;
            sample.set_value(axis_name, next);
            if Self::spec_in_gamut(sample) {
                lower_in = next;
                probe = next;
            } else {
                let mut in_v = lower_in;
                let mut out_v = next;
                for _ in 0..Self::CLAMP_REFINE_STEPS {
                    let mid = (in_v + out_v) * 0.5;
                    let mut refined = base_spec;
                    refined.set_value(axis_name, mid);
                    if Self::spec_in_gamut(refined) {
                        in_v = mid;
                    } else {
                        out_v = mid;
                    }
                }
                lower_in = in_v;
                lower_found = true;
                break;
            }
        }
        if !lower_found {
            lower_in = min;
        }

        let mut upper_in = current;
        probe = current;
        let mut upper_found = false;
        while probe < max {
            let next = (probe + step).min(max);
            let mut sample = base_spec;
            sample.set_value(axis_name, next);
            if Self::spec_in_gamut(sample) {
                upper_in = next;
                probe = next;
            } else {
                let mut in_v = upper_in;
                let mut out_v = next;
                for _ in 0..Self::CLAMP_REFINE_STEPS {
                    let mid = (in_v + out_v) * 0.5;
                    let mut refined = base_spec;
                    refined.set_value(axis_name, mid);
                    if Self::spec_in_gamut(refined) {
                        in_v = mid;
                    } else {
                        out_v = mid;
                    }
                }
                upper_in = in_v;
                upper_found = true;
                break;
            }
        }
        if !upper_found {
            upper_in = max;
        }

        (lower_in, upper_in)
    }
}

impl ColorSpecification for Lab {
    fn to_color_value(&self) -> ColorValue {
        ColorValue::LinearSrgb(palette::LinSrgba::from_color_unclamped(
            palette::Laba::<palette::white_point::D65, f32>::new(self.l, self.a, self.b, self.alpha),
        ))
    }
    fn from_color_value(source: ColorValue) -> Result<Self> {
        let lab = palette::Laba::<palette::white_point::D65, f32>::from_color_unclamped(source.to_srgba_unclamped()?);
        Ok(Self { l: lab.l, a: lab.a, b: lab.b, alpha: lab.alpha, auto_clamp: false, dynamic_range: false })
    }

    fn name(&self) -> &'static str {
        "Lab"
    }

    fn channels(&self) -> &[ColorChannel] {
        &Self::CHANNELS
    }

    fn get_value(&self, channel_name: &str) -> f32 {
        match channel_name {
            Self::LIGHTNESS => self.l,
            Self::A => self.a,
            Self::B => self.b,
            Self::ALPHA => self.alpha,
            _ => 0.0,
        }
    }

    fn set_value(&mut self, channel_name: &str, value: f32) {
        match channel_name {
            Self::LIGHTNESS => self.l = value.clamp(0.0, 100.0),
            Self::A => self.a = value.clamp(-128.0, 127.0),
            Self::B => self.b = value.clamp(-128.0, 127.0),
            Self::ALPHA => self.alpha = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn to_hsla(&self) -> Hsla {
        self.to_hsla_checked().0
    }

    fn from_hsla(hsla: Hsla) -> Self {
        let rgba = gpui_bridge::preview_rgba(hsla);
        let (l, a, b) = rgb_to_lab(rgba);
        Self { l, a, b, alpha: rgba.a, auto_clamp: false, dynamic_range: false }
    }

    fn set_auto_clamp(&mut self, auto_clamp: bool) {
        self.auto_clamp = auto_clamp;
        if auto_clamp {
            self.dynamic_range = false;
        }
    }

    fn set_dynamic_range(&mut self, dynamic_range: bool) {
        self.dynamic_range = dynamic_range;
        if dynamic_range {
            self.auto_clamp = false;
        }
    }

    fn channel_bounds(&self, channel_name: &str) -> (f32, f32) {
        if self.dynamic_range && channel_name != Self::ALPHA {
            Self::gamut_interval_around(*self, channel_name)
        } else {
            Self::channel_bounds_math(channel_name)
        }
    }

    fn channel_allowed_interval(&self, channel_name: &str) -> Option<(f32, f32)> {
        if self.auto_clamp && channel_name != Self::ALPHA {
            Some(Self::gamut_interval_around(*self, channel_name))
        } else {
            None
        }
    }

    fn clamp_spec_to_gamut(&mut self) {
        if !self.auto_clamp && !self.dynamic_range {
            return;
        }

        for _ in 0..3 {
            if Self::spec_in_gamut(*self) {
                break;
            }
            self.l = Self::clamp_channel_to_gamut_math(*self, Self::LIGHTNESS, self.l);
            self.a = Self::clamp_channel_to_gamut_math(*self, Self::A, self.a);
            self.b = Self::clamp_channel_to_gamut_math(*self, Self::B, self.b);
        }
    }

    fn clamp_channel_to_gamut(&self, channel_name: &str, proposed: f32) -> f32 {
        if self.dynamic_range {
            let mut proposed_spec = *self;
            proposed_spec.set_value(channel_name, proposed);
            if Self::spec_in_gamut(proposed_spec) {
                return proposed;
            }
            return Self::clamp_channel_to_gamut_math(*self, channel_name, proposed);
        }
        if !self.auto_clamp {
            return proposed;
        }
        Self::clamp_channel_to_gamut_math(*self, channel_name, proposed)
    }

    fn is_out_of_gamut(&self) -> bool {
        !Self::spec_in_gamut(*self)
    }
}

fn rgb_to_lab(rgb: Rgba) -> (f32, f32, f32) {
    let lab =
        palette::Lab::<palette::white_point::D65, f32>::from_color_unclamped(palette::Srgb::new(rgb.r, rgb.g, rgb.b));
    (lab.l, lab.a, lab.b)
}
fn lab_to_rgb(l: f32, a: f32, b: f32, alpha: f32) -> Rgba {
    lab_to_rgb_checked(l, a, b, alpha).0
}
fn lab_to_rgb_checked(l: f32, a: f32, b: f32, alpha: f32) -> (Rgba, bool) {
    let spec = Lab { l, a, b, alpha, auto_clamp: false, dynamic_range: false };
    let source = spec.to_color_value();
    let in_gamut = source.is_in_gamut(gpui_luma::color::Gamut::Srgb).unwrap_or(false);
    (
        gpui_bridge::to_rgba(source, GamutMapping::CssLocalMinde)
            .unwrap_or_else(|_| gpui::transparent_black().to_rgb()),
        !in_gamut,
    )
}
