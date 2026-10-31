use std::ops::RangeInclusive;

use gpui::Hsla;
use gpui_luma::color::{ColorValue, GamutMapping, gpui_bridge};
use palette::{Clamp, FromColor, IsWithinBounds, Oklch as PaletteOklch, Srgb};
use palette::convert::FromColorUnclamped;

use super::color_spec::{ColorChannel, ColorSpecification};

pub const CHROMA_MAX: f32 = 0.4;
const SCAN_STEPS: u32 = 360;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
    pub a: f32,
}

impl Oklch {
    pub const LIGHTNESS: &'static str = "lightness";
    pub const CHROMA: &'static str = "chroma";
    pub const HUE: &'static str = "hue";
    pub const ALPHA: &'static str = "alpha";

    const CHANNELS: [ColorChannel; 4] = [
        ColorChannel { name: Self::LIGHTNESS, label: "Lightness", min: 0.0, max: 1.0, step: None, unit: "" },
        ColorChannel { name: Self::CHROMA, label: "Chroma", min: 0.0, max: CHROMA_MAX, step: Some(0.01), unit: "" },
        ColorChannel { name: Self::HUE, label: "Hue", min: 0.0, max: 360.0, step: Some(1.0), unit: "°" },
        ColorChannel { name: Self::ALPHA, label: "Alpha", min: 0.0, max: 1.0, step: None, unit: "" },
    ];

    pub fn is_in_srgb_gamut(l: f32, c: f32, h: f32) -> bool {
        let oklch = PaletteOklch::new(l.clamp(0.0, 1.0), c.max(0.0), h.rem_euclid(360.0));
        Srgb::<f32>::from_color_unclamped(oklch).is_within_bounds()
    }

    pub fn max_in_gamut_chroma(l: f32, h: f32) -> f32 {
        if !Self::is_in_srgb_gamut(l, 0.0, h) {
            return 0.0;
        }

        let mut low = 0.0;
        let mut high = CHROMA_MAX;
        for _ in 0..24 {
            let mid = (low + high) * 0.5;
            if Self::is_in_srgb_gamut(l, mid, h) {
                low = mid;
            } else {
                high = mid;
            }
        }
        low
    }

    pub fn in_gamut_lightness_intervals(c: f32, h: f32) -> Vec<RangeInclusive<f32>> {
        scan_in_gamut_intervals(0.0, 1.0, SCAN_STEPS, |l| Self::is_in_srgb_gamut(l, c, h))
    }

    pub fn in_gamut_hue_intervals(l: f32, c: f32) -> Vec<RangeInclusive<f32>> {
        scan_in_gamut_intervals(0.0, 360.0, SCAN_STEPS, |hue| Self::is_in_srgb_gamut(l, c, hue))
    }

    fn clamp_to_srgb_gamut(mut self) -> Self {
        if Self::is_in_srgb_gamut(self.l, self.c, self.h) {
            return self;
        }

        let oklch = PaletteOklch::new(self.l.clamp(0.0, 1.0), self.c.max(0.0), self.h.rem_euclid(360.0));
        let clamped = Oklch::from_palette(PaletteOklch::from_color(Srgb::<f32>::from_color_unclamped(oklch).clamp()));
        self.l = clamped.l;
        self.c = clamped.c;
        self.h = clamped.h;
        self
    }

    fn from_palette(oklch: PaletteOklch) -> Self {
        Self {
            l: oklch.l.clamp(0.0, 1.0),
            c: oklch.chroma.max(0.0),
            h: oklch.hue.into_positive_degrees().rem_euclid(360.0),
            a: 1.0,
        }
    }

    fn clamp_channel_to_gamut_internal(self, channel_name: &str, proposed: f32) -> f32 {
        let (min, max) = self.channel_bounds(channel_name);
        let proposed = proposed.clamp(min, max);
        let mut candidate = self;
        candidate.set_value(channel_name, proposed);
        if Self::is_in_srgb_gamut(candidate.l, candidate.c, candidate.h) {
            return proposed;
        }

        let anchor = self.get_value(channel_name).clamp(min, max);
        if (proposed - anchor).abs() <= f32::EPSILON {
            return self.clamp_to_srgb_gamut().get_value(channel_name);
        }

        let mut in_gamut = anchor;
        let mut out_of_gamut = proposed;
        for _ in 0..24 {
            let mid = (in_gamut + out_of_gamut) * 0.5;
            candidate.set_value(channel_name, mid);
            if Self::is_in_srgb_gamut(candidate.l, candidate.c, candidate.h) {
                in_gamut = mid;
            } else {
                out_of_gamut = mid;
            }
        }
        in_gamut
    }
}

impl ColorSpecification for Oklch {
    fn name(&self) -> &'static str {
        "OKLCH"
    }

    fn channels(&self) -> &[ColorChannel] {
        &Self::CHANNELS
    }

    fn get_value(&self, channel_name: &str) -> f32 {
        match channel_name {
            Self::LIGHTNESS => self.l,
            Self::CHROMA => self.c,
            Self::HUE => self.h,
            Self::ALPHA => self.a,
            _ => 0.0,
        }
    }

    fn set_value(&mut self, channel_name: &str, value: f32) {
        match channel_name {
            Self::LIGHTNESS => self.l = value.clamp(0.0, 1.0),
            Self::CHROMA => self.c = value.clamp(0.0, CHROMA_MAX),
            Self::HUE => self.h = value.rem_euclid(360.0),
            Self::ALPHA => self.a = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn to_color_value(&self) -> ColorValue {
        ColorValue::oklch(self.l, self.c, self.h, self.a)
    }
    fn from_color_value(source: ColorValue) -> anyhow::Result<Self> {
        let c = source.to_oklcha_unclamped()?;
        Ok(Self { l: c.l, c: c.chroma, h: c.hue.into_raw_degrees(), a: c.alpha })
    }
    fn to_hsla(&self) -> Hsla {
        gpui_bridge::to_hsla(self.to_color_value(), GamutMapping::CssLocalMinde)
            .unwrap_or_else(|_| gpui::transparent_black())
    }
    fn from_hsla(hsla: Hsla) -> Self {
        let rgba = gpui_bridge::preview_rgba(hsla);
        let mut spec = Self::from_palette(PaletteOklch::from_color_unclamped(Srgb::new(rgba.r, rgba.g, rgba.b)));
        spec.a = rgba.a;
        spec
    }

    fn uses_rainbow_hue_track(&self) -> bool {
        false
    }

    fn channel_track_intervals(&self, channel_name: &str) -> Vec<RangeInclusive<f32>> {
        let mut intervals = match channel_name {
            Self::LIGHTNESS => Self::in_gamut_lightness_intervals(self.c, self.h),
            Self::CHROMA => {
                let max_chroma = Self::max_in_gamut_chroma(self.l, self.h);
                if max_chroma <= f32::EPSILON {
                    vec![0.0..=0.0]
                } else {
                    vec![0.0..=max_chroma]
                }
            }
            Self::HUE => Self::in_gamut_hue_intervals(self.l, self.c),
            _ => Vec::new(),
        };
        intervals = Self::ensure_intervals_include_current(self, channel_name, intervals);
        merge_intervals(intervals)
    }

    fn channel_allowed_intervals(&self, _channel_name: &str) -> Vec<RangeInclusive<f32>> {
        Vec::new()
    }

    fn uses_continuous_channel_interaction(&self, channel_name: &str) -> bool {
        matches!(channel_name, Self::LIGHTNESS | Self::CHROMA | Self::HUE)
    }

    fn apply_channel_slider_value(&mut self, channel_name: &str, proposed: f32) {
        self.set_value(channel_name, proposed);
    }

    fn clamp_spec_to_gamut(&mut self) {
        *self = self.clamp_to_srgb_gamut();
    }

    fn clamp_channel_to_gamut(&self, channel_name: &str, proposed: f32) -> f32 {
        self.clamp_channel_to_gamut_internal(channel_name, proposed)
    }

    fn summary(&self) -> String {
        format!("oklch({:.2} {:.3} {:.0} / {:.2})", self.l, self.c, self.h, self.a)
    }

    fn is_out_of_gamut(&self) -> bool {
        !Self::is_in_srgb_gamut(self.l, self.c, self.h)
    }
}

fn scan_in_gamut_intervals(
    min: f32,
    max: f32,
    steps: u32,
    mut sample: impl FnMut(f32) -> bool,
) -> Vec<RangeInclusive<f32>> {
    if steps == 0 {
        return Vec::new();
    }

    let step = (max - min) / steps as f32;
    let mut intervals = Vec::new();
    let mut start: Option<f32> = None;

    for index in 0..=steps {
        let value = if index == steps { max } else { min + step * index as f32 };
        let in_gamut = sample(value);

        match (in_gamut, start) {
            (true, None) => start = Some(value),
            (false, Some(start_value)) => {
                let end = if index == 0 {
                    min
                } else {
                    (value - step).max(start_value)
                };
                intervals.push(start_value..=end.min(max));
                start = None;
            }
            _ => {}
        }
    }

    if let Some(start_value) = start {
        intervals.push(start_value..=max);
    }

    intervals
}

fn local_in_gamut_interval(
    min: f32,
    max: f32,
    value: f32,
    mut in_gamut: impl FnMut(f32) -> bool,
) -> Option<RangeInclusive<f32>> {
    if !in_gamut(value) {
        return None;
    }

    let mut start = value;
    let (mut lo, mut hi) = (min, value);
    for _ in 0..32 {
        if hi - lo <= 1e-5 {
            break;
        }
        let mid = (lo + hi) * 0.5;
        if in_gamut(mid) {
            start = mid;
            hi = mid;
        } else {
            lo = mid;
        }
    }

    let mut end = value;
    let (mut lo, mut hi) = (value, max);
    for _ in 0..32 {
        if hi - lo <= 1e-5 {
            break;
        }
        let mid = (lo + hi) * 0.5;
        if in_gamut(mid) {
            end = mid;
            lo = mid;
        } else {
            hi = mid;
        }
    }

    Some(start..=end)
}

fn merge_intervals(mut intervals: Vec<RangeInclusive<f32>>) -> Vec<RangeInclusive<f32>> {
    if intervals.is_empty() {
        return intervals;
    }

    intervals.sort_by(|left, right| left.start().total_cmp(right.start()));
    let mut merged = Vec::with_capacity(intervals.len());
    let mut current = intervals[0].clone();

    for interval in intervals.into_iter().skip(1) {
        if *interval.start() <= *current.end() + 1e-4 {
            let end = current.end().max(*interval.end());
            current = *current.start()..=end;
        } else {
            merged.push(current);
            current = interval;
        }
    }

    merged.push(current);
    merged
}

impl Oklch {
    fn ensure_intervals_include_current(
        spec: &Self,
        channel_name: &str,
        mut intervals: Vec<RangeInclusive<f32>>,
    ) -> Vec<RangeInclusive<f32>> {
        if spec.is_out_of_gamut() {
            return intervals;
        }

        let value = spec.get_value(channel_name);
        if intervals.iter().any(|interval| interval.contains(&value)) {
            return intervals;
        }

        let (min, max) = spec.channel_bounds(channel_name);
        if let Some(local) = local_in_gamut_interval(min, max, value, |candidate| {
            let mut sample = *spec;
            sample.set_value(channel_name, candidate);
            Self::is_in_srgb_gamut(sample.l, sample.c, sample.h)
        }) {
            intervals.push(local);
        }

        intervals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn achromatic_oklch_is_in_gamut() {
        assert!(Oklch::is_in_srgb_gamut(0.5, 0.0, 120.0));
    }

    #[test]
    fn extreme_chroma_is_out_of_gamut() {
        assert!(!Oklch::is_in_srgb_gamut(0.5, 0.4, 0.0));
    }

    #[test]
    fn max_chroma_is_monotonic_and_bounded() {
        let max = Oklch::max_in_gamut_chroma(0.63, 265.0);
        assert!(max > 0.0);
        assert!(max <= CHROMA_MAX);
        assert!(Oklch::is_in_srgb_gamut(0.63, max, 265.0));
        assert!(!Oklch::is_in_srgb_gamut(0.63, (max + 0.001).min(CHROMA_MAX), 265.0));
    }

    #[test]
    fn high_chroma_hue_intervals_can_be_discontiguous() {
        let intervals = Oklch::in_gamut_hue_intervals(0.63, 0.22);
        assert!(!intervals.is_empty());
        assert!(intervals.iter().all(|interval| *interval.start() <= *interval.end()));
    }

    #[test]
    fn from_hsla_stays_in_gamut() {
        let spec = Oklch::from_hsla(gpui::hsla(0.55, 1.0, 0.5, 1.0));
        assert!(!spec.is_out_of_gamut());
    }

    #[test]
    fn startup_mixer_lightness_track_interval_is_narrow() {
        let spec = Oklch::from_hsla(gpui::hsla(0.55, 1.0, 0.5, 1.0));
        let intervals = spec.channel_track_intervals(Oklch::LIGHTNESS);
        assert!(!intervals.is_empty(), "startup lightness should render a gamut slice");
        let span = intervals[0].end() - intervals[0].start();
        assert!(span < 0.01, "expected a narrow lightness slice, got span {span} in {intervals:?}");
        assert!(intervals[0].contains(&spec.l));
    }

    #[test]
    fn startup_mixer_uses_continuous_lightness_interaction() {
        let spec = Oklch::from_hsla(gpui::hsla(0.55, 1.0, 0.5, 1.0));
        assert!(spec.uses_continuous_channel_interaction(Oklch::LIGHTNESS));
        assert!(spec.channel_allowed_intervals(Oklch::LIGHTNESS).is_empty());
    }

    #[test]
    fn lightness_slider_preserves_chroma_outside_srgb() {
        let mut spec = Oklch::from_hsla(gpui::hsla(0.55, 1.0, 0.5, 1.0));
        let original_l = spec.l;
        let original_c = spec.c;
        spec.apply_channel_slider_value(Oklch::LIGHTNESS, 0.85);
        assert!((spec.l - original_l).abs() > 1e-4);
        assert_eq!(spec.c, original_c);
        assert!(spec.is_out_of_gamut());
        let source = spec.to_color_value();
        let _ = spec.to_hsla();
        assert_eq!(spec.to_color_value(), source);
    }
}
