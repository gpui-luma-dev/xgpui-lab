//! Software source-space interpretation shared by CPU field and GPU uniforms.
use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicUsize, Ordering},
};
use gpui::{Hsla, Rgba};
use gpui_luma_color::{
    color_field::{ColorFieldModel2D, HueSaturationLightnessModel},
    color_slider::color_spec::Hsv,
};
use xgpui_color::{EncodedRgb, GamutMapping, LinearRgb, RgbSpace};

pub(super) const SPACES: [RgbSpace; 3] = [RgbSpace::Srgb, RgbSpace::DisplayP3, RgbSpace::Rec2020];
pub(super) const NAMES: [&str; 3] = ["sRGB", "Display P3", "Rec.2020"];

pub(super) fn preview(rgb: [f64; 3], space: RgbSpace) -> Result<Rgba, xgpui_color::ColorError> {
    if space == RgbSpace::Srgb {
        return Ok(Rgba { r: rgb[0] as f32, g: rgb[1] as f32, b: rgb[2] as f32, a: 1. });
    }
    preview_linear(EncodedRgb::new(space, rgb)?.decode()?)
}

fn preview_linear(color: LinearRgb) -> Result<Rgba, xgpui_color::ColorError> {
    let components = color.map_gamut(RgbSpace::Srgb, GamutMapping::Clip)?.encode()?.components();
    Ok(rgba(components))
}

/// Interpret encoded components in the selected source, then convert coordinates.
pub(super) struct Coordinates {
    pub srgb: [f64; 3],
    pub preview: Rgba,
    pub p3: [f64; 3],
    pub oklch: [f64; 3],
    pub rec2020: [f64; 3],
}
impl Coordinates {
    pub fn new(rgb: [f64; 3], space: RgbSpace) -> Result<Self, xgpui_color::ColorError> {
        let source = EncodedRgb::new(space, rgb)?.decode()?;
        Ok(Self {
            srgb: source.convert(RgbSpace::Srgb)?.encode()?.components(),
            preview: preview_linear(source)?,
            p3: source.convert(RgbSpace::DisplayP3)?.encode()?.components(),
            oklch: source.to_oklab()?.to_oklch()?.components(),
            rec2020: source.convert(RgbSpace::Rec2020)?.encode()?.components(),
        })
    }
}

pub(super) fn rgba(rgb: [f64; 3]) -> Rgba {
    Rgba { r: rgb[0] as f32, g: rgb[1] as f32, b: rgb[2] as f32, a: 1. }
}
pub(super) fn hex(rgb: [f64; 3]) -> String {
    let [r, g, b] = rgb.map(|value| (value.clamp(0., 1.) * 255.).round() as u8);
    format!("#{r:02X}{g:02X}{b:02X}")
}

#[derive(Clone)]
pub(super) struct FieldSettings {
    source_index: Arc<AtomicUsize>,
    width_bits: Arc<AtomicU32>,
}
impl FieldSettings {
    pub fn new(source_index: usize) -> Self {
        Self { source_index: Arc::new(AtomicUsize::new(source_index)), width_bits: Arc::new(AtomicU32::new(0)) }
    }
    pub fn source_index(&self) -> usize {
        self.source_index.load(Ordering::Relaxed)
    }
    pub fn source_space(&self) -> RgbSpace {
        SPACES[self.source_index()]
    }
    pub fn set_source(&self, index: usize) {
        self.source_index.store(index, Ordering::Relaxed);
    }
    pub fn resize(&self, width: f32) -> bool {
        self.width_bits.swap(width.to_bits(), Ordering::Relaxed) != width.to_bits()
    }
    pub fn model(&self) -> Arc<SourceFieldModel> {
        Arc::new(SourceFieldModel { settings: self.clone() })
    }
    fn strip_start(&self) -> f32 {
        1. - ENDPOINT_STRIP_WIDTH / f32::from_bits(self.width_bits.load(Ordering::Relaxed)).max(ENDPOINT_STRIP_WIDTH)
    }
    fn cache_key(&self) -> u64 {
        (self.source_index() as u64) << 32 | self.width_bits.load(Ordering::Relaxed) as u64
    }
}

pub(super) const ENDPOINT_STRIP_WIDTH: f32 = 6.;

pub(super) struct SourceFieldModel {
    settings: FieldSettings,
}
impl SourceFieldModel {
    fn display(&self, color: Hsla) -> Hsla {
        let raw: Rgba = color.into();
        preview([raw.r as f64, raw.g as f64, raw.b as f64], self.settings.source_space())
            .map(Into::into)
            .unwrap_or(color)
    }
}
impl ColorFieldModel2D for SourceFieldModel {
    fn apply_uv(&self, hsv: &mut Hsv, uv: (f32, f32)) {
        HueSaturationLightnessModel.apply_uv(hsv, uv);
    }
    fn uv_from_hsv(&self, hsv: &Hsv) -> (f32, f32) {
        HueSaturationLightnessModel.uv_from_hsv(hsv)
    }
    fn color_at_uv(&self, hsv: &Hsv, uv: (f32, f32)) -> Hsla {
        if uv.0 >= self.settings.strip_start() {
            return if uv.1 < 0.5 { gpui::white() } else { gpui::black() };
        }
        self.display(HueSaturationLightnessModel.color_at_uv(hsv, uv))
    }
    fn thumb_color(&self, hsv: &Hsv) -> Hsla {
        self.display(hsv.to_hsla_ext())
    }
    fn cache_key_part(&self) -> u64 {
        self.settings.cache_key()
    }
}
