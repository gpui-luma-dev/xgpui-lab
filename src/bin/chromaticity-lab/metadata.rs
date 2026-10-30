//! Trusted synthetic profile fixtures; no file/image decoding.
use std::sync::{Arc, OnceLock};
use xgpui_color::{LinearRgb, RgbDefinition, RgbSpace, icc::IccRgbProfile};
use xgpui_media::pipeline::{AlphaMode, ImageColor, ImagePrimaries, PixelColor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Named,
    Parametric,
    Sampled,
}
impl Mode {
    pub fn next(self) -> Self {
        match self {
            Self::Named => Self::Parametric,
            Self::Parametric => Self::Sampled,
            Self::Sampled => Self::Named,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Named => "named",
            Self::Parametric => "ICC parametric fixture",
            Self::Sampled => "ICC sampled fixture",
        }
    }
    pub fn color(self, named: ImageColor) -> anyhow::Result<PixelColor> {
        let index = match self {
            Self::Named => return Ok(named.into()),
            Self::Parametric => 0,
            Self::Sampled => 1,
        };
        Ok(PixelColor::Custom { rgb: fixtures()?[index].clone(), alpha: AlphaMode::Straight })
    }
}
static FIXTURES: OnceLock<Result<[Arc<RgbDefinition>; 2], String>> = OnceLock::new();
pub fn fixtures() -> anyhow::Result<&'static [Arc<RgbDefinition>; 2]> {
    FIXTURES
        .get_or_init(|| {
            let parse = |bytes: &[u8]| {
                IccRgbProfile::parse(bytes).map(|p| Arc::new(p.into_rgb_definition())).map_err(|e| e.to_string())
            };
            Ok([
                parse(include_bytes!("../../../assets/colorimetry/icc/parametric-rgb.icc"))?,
                parse(include_bytes!("../../../assets/colorimetry/icc/sampled-rgb.icc"))?,
            ])
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!("ICC fixture: {e}"))
}

pub struct SourceSample {
    pub decoded: [f64; 3],
    pub xyz: [f64; 3],
}
pub fn inspect(color: &PixelColor, rgba: [f32; 4]) -> anyhow::Result<SourceSample> {
    let pixel = color.decode(rgba.map(f64::from), ImagePrimaries::Rec2020)?.rgba();
    let xyz = LinearRgb::new(RgbSpace::Rec2020, [pixel[0], pixel[1], pixel[2]])?.to_xyz()?.components();
    let decoded = match color {
        PixelColor::Named(named) => {
            let p = named.decode(rgba.map(f64::from), named.primaries)?.rgba();
            [p[0], p[1], p[2]]
        }
        PixelColor::ExtendedLinear { primaries, .. } => {
            let p = color.decode(rgba.map(f64::from), *primaries)?.rgba();
            [p[0], p[1], p[2]]
        }
        PixelColor::Custom { rgb, alpha } => {
            if rgba[3] == 0. {
                [0.; 3]
            } else {
                let mut encoded = [f64::from(rgba[0]), f64::from(rgba[1]), f64::from(rgba[2])];
                if *alpha == AlphaMode::Premultiplied {
                    encoded = encoded.map(|v| v / f64::from(rgba[3]));
                }
                let curves = rgb.curves();
                [curves[0].decode(encoded[0])?, curves[1].decode(encoded[1])?, curves[2].decode(encoded[2])?]
            }
        }
    };
    Ok(SourceSample { decoded, xyz })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn near_black_uses_exact_serialized_curves_and_canonical_alpha() {
        let x = f64::from(0.0001_f32);
        for (mode, expected) in [(Mode::Parametric, x * 5072. / 65536.), (Mode::Sampled, x * 256. * 3. / 65535.)] {
            let color = mode.color(ImageColor::ADOBE_RGB_1998).unwrap();
            let source = inspect(&color, [0., 0., 0.0001, 0.5]).unwrap();
            assert!((source.decoded[2] - expected).abs() <= 8. * f64::EPSILON * expected);
            let transparent = inspect(&color, [0.1, 0.2, 0.3, 0.]).unwrap();
            assert_eq!(transparent.decoded, [0.; 3]);
            assert_eq!(transparent.xyz, [0.; 3]);
            assert!(inspect(&color, [f32::NAN, 0., 0., 1.]).is_err());
        }
    }
    #[test]
    fn metadata_changes_identical_pixels_and_stores_independent_half_references() {
        use xgpui_media::pipeline::{pixels_rgba16f_data, GamutMapping};
        // Independent 120-digit serialized-tag/Bradford/primary solve, exact
        // f32 input 0.0001, clipping, alpha=.5, then binary16 packing.
        for (mode, xyz, storage, bits) in [
            (
                Mode::Parametric,
                [7.3558157969206e-6, 7.73925513067297e-6, 8.428341347755057e-6],
                ImagePrimaries::DisplayP3,
                [0x41, 0x41, 0x41, 0x3800],
            ),
            (
                Mode::Parametric,
                [7.3558157969206e-6, 7.73925513067297e-6, 8.428341347755057e-6],
                ImagePrimaries::Rec2020,
                [0x41, 0x41, 0x41, 0x3800],
            ),
            (
                Mode::Sampled,
                [4.458447299523196e-7, 2.0439032060669202e-7, 1.1722832331397878e-6],
                ImagePrimaries::DisplayP3,
                [4, 0, 9, 0x3800],
            ),
            (
                Mode::Sampled,
                [4.458447299523196e-7, 2.0439032060669202e-7, 1.1722832331397878e-6],
                ImagePrimaries::Rec2020,
                [3, 0, 9, 0x3800],
            ),
        ] {
            let color = mode.color(ImageColor::ADOBE_RGB_1998).unwrap();
            let rgba = [0.0001, 0.0001, 0.0001, 0.5];
            let sample = inspect(&color, rgba).unwrap();
            for (a, b) in sample.xyz.into_iter().zip(xyz) {
                assert!((a - b).abs() <= 64. * f64::EPSILON * b.abs());
            }
            let data = pixels_rgba16f_data(1, 1, vec![rgba], color, storage, GamutMapping::Clip).unwrap();
            for (i, bits) in bits.into_iter().enumerate() {
                let actual = u16::from_le_bytes([data.levels()[0][2 * i], data.levels()[0][2 * i + 1]]);
                assert!(actual.abs_diff(bits) <= 1);
                if bits == 0 {
                    assert_eq!(actual, 0);
                }
            }
        }
        let param = Mode::Parametric.color(ImageColor::SRGB).unwrap();
        let sampled = Mode::Sampled.color(ImageColor::SRGB).unwrap();
        assert_ne!(inspect(&param, [0.0001; 4]).unwrap().decoded, inspect(&sampled, [0.0001; 4]).unwrap().decoded);
    }
}
