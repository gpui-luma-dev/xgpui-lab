//! Trusted synthetic and published profile fixtures; no file/image decoding.
use std::sync::{Arc, OnceLock};
use xgpui_color::{LinearRgb, RgbDefinition, RgbSpace, icc::IccRgbProfile};
use xgpui_media::pipeline::{AlphaMode, ImageColor, ImagePrimaries, PixelColor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Named,
    Parametric,
    Sampled,
    CompactSrgb,
    CompactP3,
    CompactAdobe,
    Srgb2014,
}
impl Mode {
    pub fn next(self) -> Self {
        match self {
            Self::Named => Self::Parametric,
            Self::Parametric => Self::Sampled,
            Self::Sampled => Self::CompactSrgb,
            Self::CompactSrgb => Self::CompactP3,
            Self::CompactP3 => Self::CompactAdobe,
            Self::CompactAdobe => Self::Srgb2014,
            Self::Srgb2014 => Self::Named,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Named => "named",
            Self::Parametric => "ICC parametric fixture",
            Self::Sampled => "ICC sampled fixture",
            Self::CompactSrgb => "ICC sRGB (Compact v4)",
            Self::CompactP3 => "ICC P3 (Compact v4)",
            Self::CompactAdobe => "ICC Adobe-compatible (Compact v4)",
            Self::Srgb2014 => "ICC sRGB2014 (v2 sampled)",
        }
    }
    pub fn color(self, named: ImageColor) -> anyhow::Result<PixelColor> {
        let index = match self {
            Self::Named => return Ok(named.into()),
            Self::Parametric => 0,
            Self::Sampled => 1,
            Self::CompactSrgb => 2,
            Self::CompactP3 => 3,
            Self::CompactAdobe => 4,
            Self::Srgb2014 => 5,
        };
        Ok(PixelColor::Custom { rgb: fixtures()?[index].clone(), alpha: AlphaMode::Straight })
    }
}
static FIXTURES: OnceLock<Result<[Arc<RgbDefinition>; 6], String>> = OnceLock::new();
pub fn fixtures() -> anyhow::Result<&'static [Arc<RgbDefinition>; 6]> {
    FIXTURES
        .get_or_init(|| {
            let parse = |bytes: &[u8]| {
                IccRgbProfile::parse(bytes).map(|p| Arc::new(p.into_rgb_definition())).map_err(|e| e.to_string())
            };
            Ok([
                parse(include_bytes!("../../../assets/colorimetry/icc/parametric-rgb.icc"))?,
                parse(include_bytes!("../../../assets/colorimetry/icc/sampled-rgb.icc"))?,
                parse(include_bytes!("../../../assets/colorimetry/icc/compact/sRGB-v4.icc"))?,
                parse(include_bytes!("../../../assets/colorimetry/icc/compact/DisplayP3-v4.icc"))?,
                parse(include_bytes!("../../../assets/colorimetry/icc/compact/AdobeCompat-v4.icc"))?,
                parse(include_bytes!("../../../assets/colorimetry/icc/sRGB2014.icc"))?,
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
    fn published_profiles_cycle_and_match_independent_xyz() {
        let mut mode = Mode::Named;
        let expected_modes = [
            Mode::Parametric,
            Mode::Sampled,
            Mode::CompactSrgb,
            Mode::CompactP3,
            Mode::CompactAdobe,
            Mode::Srgb2014,
            Mode::Named,
        ];
        for next in expected_modes {
            mode = mode.next();
            assert_eq!(mode, next);
        }
        // Exact serialized tags + 120-digit Bradford oracle from the sibling
        // generate_real_icc_references.py; these are not named-space constants.
        for (mode, green, gray, decoded) in [
            (
                Mode::CompactSrgb,
                [0.3575719712079898, 0.7151991803970688, 0.11920574451602804],
                [0.025863563264526754, 0.027211743888804932, 0.029635160593795767],
                0.027211743888804932,
            ),
            (
                Mode::CompactP3,
                [0.265666201411577, 0.6917408005147115, 0.04512168518649937],
                [0.025863563264526754, 0.027211743888804932, 0.029635160593795767],
                0.027211743888804932,
            ),
            (
                Mode::CompactAdobe,
                [0.18556077203321786, 0.6273601958560202, 0.07067922932565311],
                [0.021854144243416727, 0.022993327329977945, 0.025041061344471423],
                0.022993327329977945,
            ),
            (
                Mode::Srgb2014,
                [0.3576171193575301, 0.7151362416068924, 0.11920756219116285],
                [0.02586876064369192, 0.027213980056848303, 0.02964513297459798],
                0.02721477274291892,
            ),
        ] {
            let color = mode.color(ImageColor::SRGB).unwrap();
            for (rgba, expected) in [([0., 1., 0., 0.5], green), ([0.18, 0.18, 0.18, 0.5], gray)] {
                let sample = inspect(&color, rgba).unwrap();
                let energy = if rgba[0] == 0. { 1. } else { 3. * decoded };
                for c in 0..3 {
                    assert!((sample.xyz[c] - expected[c]).abs() <= 128. * f64::EPSILON * energy);
                }
            }
            let gray = inspect(&color, [0.18, 0.18, 0.18, 0.5]).unwrap();
            for value in gray.decoded {
                assert!((value - decoded).abs() <= 16. * f64::EPSILON * decoded);
            }
            assert_eq!(inspect(&color, [0.18, 0.18, 0.18, 0.]).unwrap().xyz, [0.; 3]);
        }
    }

    #[test]
    fn srgb2014_keeps_its_encoded_white_and_changes_gray_interpretation() {
        let color = Mode::Srgb2014.color(ImageColor::SRGB).unwrap();
        let sample = inspect(&color, [1., 1., 1., 1.]).unwrap();
        let expected = [0.9505411229429714, 0.9999708729491109, 1.0893029772703657];
        for (actual, expected) in sample.xyz.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= 192. * f64::EPSILON);
        }
        assert!((sample.xyz[2] - xgpui_color::WhitePoint::D65.xyz()[2]).abs() > 1e-4);
        let compact = Mode::CompactSrgb.color(ImageColor::SRGB).unwrap();
        assert_ne!(
            inspect(&color, [0.18, 0.18, 0.18, 1.]).unwrap().decoded,
            inspect(&compact, [0.18, 0.18, 0.18, 1.]).unwrap().decoded
        );
    }

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
