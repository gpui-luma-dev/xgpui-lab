//! Software source-space interpretation shared by CPU field and GPU uniforms.
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
};
use gpui::{Hsla, Rgba};
use gpui_luma_color::{
    color_field::{ColorFieldModel2D},
    color_slider::color_spec::Hsv,
};
use xgpui_color::{EncodedRgb, GamutMapping, LinearRgb, RgbSpace};

pub(super) const SPACES: [RgbSpace; 4] = [RgbSpace::Srgb, RgbSpace::DisplayP3, RgbSpace::Rec2020, RgbSpace::AdobeRgb1998];
pub(super) const NAMES: [&str; 4] = ["sRGB", "Display P3", "Rec.2020", "Adobe RGB (1998)"];

pub(super) fn srgb_reference(rgb: [f64; 3], space: RgbSpace) -> Result<Rgba, xgpui_color::ColorError> {
    srgb_reference_linear(EncodedRgb::new(space, rgb)?.decode()?)
}

pub(super) fn convert_rgb(
    rgb: [f64; 3],
    source: RgbSpace,
    destination: RgbSpace,
) -> Result<[f64; 3], xgpui_color::ColorError> {
    if source == destination {
        return Ok(rgb);
    }
    Ok(EncodedRgb::new(source, rgb)?.decode()?.convert(destination)?.encode()?.components())
}

fn srgb_reference_linear(color: LinearRgb) -> Result<Rgba, xgpui_color::ColorError> {
    let components = color.map_gamut(RgbSpace::Srgb, GamutMapping::Clip)?.encode()?.components();
    Ok(rgba(components))
}

/// Interpret encoded components in the selected source, then convert coordinates.
pub(super) struct Coordinates {
    pub srgb: [f64; 3],
    pub srgb_reference: Rgba,
    pub p3: [f64; 3],
    pub oklch: [f64; 3],
    pub rec2020: [f64; 3],
    pub outside_srgb: bool,
}
impl Coordinates {
    pub fn new(rgb: [f64; 3], space: RgbSpace) -> Result<Self, xgpui_color::ColorError> {
        let source = EncodedRgb::new(space, rgb)?.decode()?;
        let srgb = source.convert(RgbSpace::Srgb)?;
        let p3 = source.convert(RgbSpace::DisplayP3)?;
        let rec2020 = source.convert(RgbSpace::Rec2020)?;
        Ok(Self {
            srgb: srgb.encode()?.components(),
            srgb_reference: srgb_reference_linear(source)?,
            p3: p3.encode()?.components(),
            oklch: source.to_oklab()?.to_oklch()?.components(),
            rec2020: rec2020.encode()?.components(),
            outside_srgb: outside_gamut(srgb),
        })
    }
}

fn outside_gamut(color: LinearRgb) -> bool {
    const BOUNDARY_TOLERANCE: f64 = 1e-6;
    color
        .components()
        .iter()
        .any(|channel| *channel < -BOUNDARY_TOLERANCE || *channel > 1. + BOUNDARY_TOLERANCE)
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
    saturation_bits: Arc<AtomicU32>,
    contour: Arc<AtomicBool>,
}
impl FieldSettings {
    pub fn new(source_index: usize) -> Self {
        Self {
            source_index: Arc::new(AtomicUsize::new(source_index)),
            width_bits: Arc::new(AtomicU32::new(0)),
            saturation_bits: Arc::new(AtomicU32::new(1f32.to_bits())),
            contour: Arc::new(AtomicBool::new(true)),
        }
    }
    pub fn contour(&self) -> bool {
        self.contour.load(Ordering::Relaxed)
    }
    pub fn set_contour(&self, enabled: bool) {
        self.contour.store(enabled, Ordering::Relaxed);
    }
    pub fn saturation(&self) -> f32 {
        f32::from_bits(self.saturation_bits.load(Ordering::Relaxed))
    }
    pub fn set_saturation(&self, saturation: f32) {
        self.saturation_bits.store(saturation.clamp(0., 1.).to_bits(), Ordering::Relaxed);
    }
    pub fn source_index(&self) -> usize {
        self.source_index.load(Ordering::Relaxed)
    }
    pub fn source_space(&self) -> RgbSpace {
        SPACES[self.source_index()]
    }
    pub fn window_mode(&self) -> gpui::WindowColorMode {
        if self.source_space() == RgbSpace::Srgb {
            gpui::WindowColorMode::Default
        } else {
            gpui::WindowColorMode::WideGamutSdr
        }
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
        ((self.source_index() as u64) << 32 | self.width_bits.load(Ordering::Relaxed) as u64)
            ^ (self.saturation_bits.load(Ordering::Relaxed) as u64).rotate_left(17)
    }
}

pub(super) const ENDPOINT_STRIP_WIDTH: f32 = 6.;

pub(super) struct SourceFieldModel {
    settings: FieldSettings,
}
impl SourceFieldModel {
    fn display(&self, color: Hsla) -> Hsla {
        let raw: Rgba = color.into();
        srgb_reference([raw.r as f64, raw.g as f64, raw.b as f64], self.settings.source_space())
            .map(Into::into)
            .unwrap_or(color)
    }
}
impl ColorFieldModel2D for SourceFieldModel {
    fn apply_uv(&self, hsv: &mut Hsv, uv: (f32, f32)) {
        hsv.h = uv.0.clamp(0., 1.) * 360.;
        hsv.s = self.settings.saturation();
        hsv.v = 1. - uv.1.clamp(0., 1.);
        if uv.0 >= self.settings.strip_start() {
            hsv.s = 0.;
            hsv.v = if uv.1 < 0.5 { 1. } else { 0. };
        }
    }
    fn uv_from_hsv(&self, hsv: &Hsv) -> (f32, f32) {
        (hsv.h / 360., 1. - hsv.v)
    }
    fn color_at_uv(&self, hsv: &Hsv, uv: (f32, f32)) -> Hsla {
        if uv.0 >= self.settings.strip_start() {
            return if uv.1 < 0.5 { gpui::white() } else { gpui::black() };
        }
        let mut sample = *hsv;
        self.apply_uv(&mut sample, uv);
        self.display(sample.to_hsla_ext())
    }
    fn thumb_color(&self, hsv: &Hsv) -> Hsla {
        self.display(hsv.to_hsla_ext())
    }
    fn cache_key_part(&self) -> u64 {
        self.settings.cache_key()
    }
}

#[cfg(test)]
mod tests {
    use super::{Coordinates, SPACES, outside_gamut};
    use xgpui_color::{LinearRgb, RgbSpace};

    #[test]
    fn destination_gamut_checks_original_values_before_mapping() {
        for space in SPACES {
            for gray in [0., 0.5, 1.] {
                let color = Coordinates::new([gray; 3], space).unwrap();
                assert!(!color.outside_srgb);
            }
        }
        for rgb in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0.86, 0.32, 0.58]] {
            let color = Coordinates::new(rgb, RgbSpace::Srgb).unwrap();
            assert!(!color.outside_srgb);
        }
        let p3 = Coordinates::new([0., 1., 0.], RgbSpace::DisplayP3).unwrap();
        assert!(p3.outside_srgb);
        let rec2020 = Coordinates::new([0., 1., 0.], RgbSpace::Rec2020).unwrap();
        assert!(rec2020.outside_srgb);
        assert!(rec2020.srgb.iter().any(|value| *value < 0. || *value > 1.));
        assert!([rec2020.srgb_reference.r, rec2020.srgb_reference.g, rec2020.srgb_reference.b]
            .into_iter()
            .all(|value| (0. ..=1.).contains(&value)));
        assert!(!outside_gamut(LinearRgb::new(RgbSpace::Srgb, [-1e-8, 0.5, 1. + 1e-8]).unwrap()));
        assert!(outside_gamut(LinearRgb::new(RgbSpace::Srgb, [-1e-4, 0.5, 1.]).unwrap()));
    }
}
