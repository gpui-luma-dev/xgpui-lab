//! CPU reference samples for an explicitly sRGB browser preview.
use xgpui_gamut::color::*;
pub const SAMPLE_COUNT: usize = 32;
pub const PRESETS: [&str; 3] = ["P3 blue → yellow", "P3 red → green", "P3 orange → violet"];
pub struct SampleRow {
    pub label: &'static str,
    pub rgba: Vec<[f32; 4]>,
    pub outside: Vec<bool>,
}
pub fn rows(preset: usize, policy: GamutMapping, translucent: bool) -> Result<Vec<SampleRow>, ColorError> {
    rows_in_space(preset, policy, translucent, RgbSpace::Srgb)
}

/// Same reference samples, encoded for the selected presentation space.
pub fn rows_in_space(
    preset: usize,
    policy: GamutMapping,
    translucent: bool,
    destination: RgbSpace,
) -> Result<Vec<SampleRow>, ColorError> {
    let (a, b) = match preset % 3 {
        0 => ([0., 0., 1.], [1., 1., 0.]),
        1 => ([1., 0., 0.], [0., 1., 0.]),
        _ => ([1., 0.18, 0.], [0.45, 0., 1.]),
    };
    let a = LinearRgba::new(LinearRgb::new(RgbSpace::DisplayP3, a)?, 1.)?;
    let b = LinearRgba::new(LinearRgb::new(RgbSpace::DisplayP3, b)?, if translucent { 0.25 } else { 1. })?;
    let encoded_a = a.rgb().convert(RgbSpace::Srgb)?.encode()?.components();
    let encoded_b = b.rgb().convert(RgbSpace::Srgb)?.encode()?.components();
    let mut rows = Vec::new();
    for (index, label) in ["Encoded sRGB", "Linear RGB", "OkLab", "OkLCh · shorter hue"].into_iter().enumerate() {
        let model = match index {
            1 => ColorInterpolation::LinearRgb,
            2 => ColorInterpolation::Oklab,
            _ => ColorInterpolation::Oklch(HueInterpolation::Shorter),
        };
        let gradient = LinearGradient::new([0., 0.], [1., 0.], [a, b], RgbSpace::Srgb, model)?;
        let mut row = SampleRow { label, rgba: Vec::new(), outside: Vec::new() };
        for sample in 0..SAMPLE_COUNT {
            let t = sample as f64 / (SAMPLE_COUNT - 1) as f64;
            let color = if index == 0 {
                let alpha = a.alpha() * (1. - t) + b.alpha() * t;
                let encoded = std::array::from_fn(|i| {
                    (encoded_a[i] * a.alpha() * (1. - t) + encoded_b[i] * b.alpha() * t) / alpha
                });
                LinearRgba::new(EncodedRgb::new(RgbSpace::Srgb, encoded)?.decode()?, alpha)?
            } else {
                gradient.sample_at(t)?
            };
            // Ignore only numerical noise at the RGB cube boundary.
            row.outside.push(color.rgb().components().iter().any(|v| *v < -1e-7 || *v > 1. + 1e-7));
            let mapped = color.map_gamut(destination, policy)?;
            let [r, g, b] = mapped.rgb().encode()?.components();
            row.rgba.push([r as f32, g as f32, b as f32, mapped.alpha() as f32]);
        }
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_presets_produce_bounded_samples_with_shared_endpoints() {
        for preset in 0..PRESETS.len() {
            for policy in [GamutMapping::Clip, GamutMapping::ReduceChroma] {
                for translucent in [false, true] {
                    let rows = rows(preset, policy, translucent).unwrap();
                    assert_eq!(rows.len(), 4);
                    for row in &rows {
                        assert_eq!(row.rgba.len(), SAMPLE_COUNT);
                        assert_eq!(row.outside.len(), SAMPLE_COUNT);
                        for rgba in &row.rgba {
                            assert!(rgba.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
                        }
                        for i in [0, SAMPLE_COUNT - 1] {
                            for c in 0..4 {
                                assert!((row.rgba[i][c] - rows[0].rgba[i][c]).abs() < 1e-5);
                            }
                        }
                        assert_eq!(row.rgba[0][3], 1.);
                        assert_eq!(row.rgba[SAMPLE_COUNT - 1][3], if translucent { 0.25 } else { 1. });
                    }
                }
            }
        }
    }
    #[test]
    fn p3_output_preserves_p3_red_and_srgb_diagnostics() {
        let srgb = rows(1, GamutMapping::Clip, false).unwrap();
        let p3 = rows_in_space(1, GamutMapping::Clip, false, RgbSpace::DisplayP3).unwrap();
        for (a, b) in srgb.iter().zip(&p3) {
            assert_eq!(a.outside, b.outside);
            assert!(b.rgba.iter().flatten().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
            assert!((b.rgba[0][0] - 1.).abs() < 1e-5);
            assert!(b.rgba[0][1].abs() < 1e-5 && b.rgba[0][2].abs() < 1e-5);
        }
    }
    #[test]
    fn mapping_changes_preview_without_changing_outside_diagnostics() {
        let clip = rows(1, GamutMapping::Clip, false).unwrap();
        let reduce = rows(1, GamutMapping::ReduceChroma, false).unwrap();
        let mut changes = 0;
        for (a, b) in clip.iter().zip(&reduce) {
            assert_eq!(a.outside, b.outside);
            assert!(a.outside.iter().any(|v| *v));
            changes += a
                .rgba
                .iter()
                .zip(&b.rgba)
                .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| (a - b).abs() > 0.001))
                .count();
        }
        assert!(changes > 0);
    }
}
