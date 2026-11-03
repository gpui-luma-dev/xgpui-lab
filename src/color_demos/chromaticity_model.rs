//! Chromaticity calculations ported from xgpui's extensions-web color_tasks_model.
use xgpui_color::{EncodedRgb, GamutMapping, LinearRgb, RgbSpace, XyzD65};

pub const SPACES: [RgbSpace; 4] = [RgbSpace::Srgb, RgbSpace::DisplayP3, RgbSpace::AdobeRgb1998, RgbSpace::Rec2020];
pub const NAMES: [&str; 4] = ["sRGB", "Display P3", "Adobe RGB (1998)", "Rec.2020"];
pub const PATCHES: [(&str, [f32; 3]); 6] = [
    ("Green", [0., 1., 0.]),
    ("Cyan", [0., 1., 1.]),
    ("Mid gray", [0.18; 3]),
    ("Near black", [0.0001; 3]),
    ("White", [1.; 3]),
    ("Red", [1., 0., 0.]),
];
pub const LUMINANCE: [f64; 3] = [0.05, 0.18, 0.5];
pub const COLORS: [u32; 4] = [0x65d7ff, 0xff9a62, 0xc4a0ff, 0x91df86];

pub fn xy(xyz: [f64; 3]) -> Option<[f64; 2]> {
    let sum: f64 = xyz.into_iter().sum();
    (sum.is_finite() && sum > 0.).then(|| [xyz[0] / sum, xyz[1] / sum])
}
pub fn locus() -> Vec<[f64; 2]> {
    include_str!("../../assets/color-labs/colorimetry/CIE_cc_1931_2deg.csv")
        .lines()
        .map(|line| {
            let v: Vec<f64> = line.split(',').map(|v| v.parse().expect("bundled CIE datum")).collect();
            xy([v[1], v[2], v[3]]).expect("nonzero spectral coordinate")
        })
        .collect()
}
fn contains(points: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    let mut previous = points[points.len() - 1];
    for &current in points {
        if (current[1] > p[1]) != (previous[1] > p[1])
            && p[0] < (previous[0] - current[0]) * (p[1] - current[1]) / (previous[1] - current[1]) + current[0]
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}
pub struct Cell {
    #[cfg(all(test, target_os = "macos"))]
    pub xy: [f64; 2],
    #[cfg(test)]
    pub rgb: [f32; 3],
    pub outside: bool,
}
pub fn cells(
    space: RgbSpace,
    luminance: f64,
    _mapping: GamutMapping,
    boundary: &[[f64; 2]],
) -> anyhow::Result<Vec<Cell>> {
    let mut cells = Vec::new();
    for row in 0..54 {
        for col in 0..48 {
            let p = [(col as f64 + 0.5) * 0.8 / 48., (row as f64 + 0.5) * 0.9 / 54.];
            if !contains(boundary, p) {
                continue;
            }
            let xyz = XyzD65::new([p[0] * luminance / p[1], luminance, (1. - p[0] - p[1]) * luminance / p[1]])?;
            let rgb = xyz.to_linear(space)?;
            let outside = rgb.components().into_iter().any(|v| !(-1e-12..=1. + 1e-12).contains(&v));
            // CPU colors are an independent oracle for shader readback tests.
            // The live UI only needs the sampled out-of-gamut count.
            #[cfg(test)]
            let preview = rgb
                .map_gamut(space, _mapping)?
                .map_gamut(RgbSpace::Srgb, GamutMapping::Clip)?
                .encode()?
                .components()
                .map(|v| v as f32);
            cells.push(Cell {
                #[cfg(all(test, target_os = "macos"))]
                xy: [col as f64 * 0.8 / 48., row as f64 * 0.9 / 54.],
                #[cfg(test)]
                rgb: preview,
                outside,
            });
        }
    }
    Ok(cells)
}
pub fn source_xy(space: RgbSpace, rgb: [f32; 3]) -> Option<[f64; 2]> {
    let linear = if space == RgbSpace::Rec2020 {
        LinearRgb::new(space, rgb.map(f64::from)).ok()?
    } else {
        EncodedRgb::new(space, rgb.map(f64::from)).ok()?.decode().ok()?
    };
    xy(linear.to_xyz().ok()?.components())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamut_fill_covers_the_same_locus_but_changes_mapping_and_outside_count() {
        let boundary = locus();
        assert_eq!(boundary.len(), 471);
        let srgb = cells(RgbSpace::Srgb, 0.18, GamutMapping::Clip, &boundary).unwrap();
        let p3 = cells(RgbSpace::DisplayP3, 0.18, GamutMapping::Clip, &boundary).unwrap();
        let reduced = cells(RgbSpace::DisplayP3, 0.18, GamutMapping::ReduceChroma, &boundary).unwrap();
        assert_eq!(srgb.len(), p3.len());
        assert!(srgb.iter().filter(|c| c.outside).count() > p3.iter().filter(|c| c.outside).count());
        assert!(p3.iter().zip(&reduced).any(|(a, b)| a.rgb != b.rgb));
        assert!(
            srgb.iter()
                .chain(&p3)
                .chain(&reduced)
                .all(|c| c.rgb.iter().all(|v| v.is_finite() && (0. ..=1.).contains(v)))
        );
        assert_ne!(source_xy(RgbSpace::Srgb, [0., 1., 0.]), source_xy(RgbSpace::DisplayP3, [0., 1., 0.]));
        assert!(source_xy(RgbSpace::Srgb, [0.; 3]).is_none());
    }
}
