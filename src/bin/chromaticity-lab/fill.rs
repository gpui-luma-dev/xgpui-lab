//! Fixed-relative-Y chromaticity fills. Raster generation stays off the UI thread.
use super::{coordinates, primaries};
use anyhow::{Result, ensure};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};
use xgpui_color::{GamutMapping, RgbSpace, XyzD65};
use xgpui_media::{
    ImageJob, MediaLoader,
    pipeline::{AlphaMode, ImageColor, TransferFunction},
};

pub const SIDE: u32 = 256;
pub const LUMINANCE: [f64; 4] = [0.05, 0.18, 0.5, 1.];
#[derive(Clone, Copy)]
pub struct Settings {
    pub uv: bool,
    pub luminance: f64,
    pub output: RgbSpace,
    pub mapping: GamutMapping,
    pub hatch: bool,
}
pub struct Prepared {
    pub image: ImageJob,
    pub mapped: usize,
    pub visible: usize,
}
pub struct Job {
    receiver: Receiver<Result<Prepared>>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl Job {
    pub fn start(settings: Settings) -> Self {
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker = cancel.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let (pixels, mapped, visible) = raster(settings, SIDE, || worker.load(Ordering::Acquire))?;
                ensure!(!worker.load(Ordering::Acquire), "fill cancelled");
                let color = ImageColor {
                    primaries: primaries(settings.output),
                    transfer: TransferFunction::Linear,
                    alpha: AlphaMode::Straight,
                };
                let loader = MediaLoader::default();
                // Bounded P3 does not fit wholly inside Rec.2020. Retain that source
                // contract; Adobe RGB/Rec.2020 use the explicit Rec.2020 texture route.
                let image = if settings.output == RgbSpace::DisplayP3 || settings.output == RgbSpace::Srgb {
                    loader.load_rgba16f(SIDE, SIDE, pixels, color)?
                } else {
                    loader.load_rec2020_rgba16f(SIDE, SIDE, pixels, color, GamutMapping::Clip)?
                };
                Ok(Prepared { image, mapped, visible })
            })();
            let _ = sender.send(result);
        });
        Self { receiver, cancel }
    }
    pub fn poll(&self) -> Result<Option<Prepared>> {
        match self.receiver.try_recv() {
            Ok(result) => result.map(Some),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => anyhow::bail!("fill worker disconnected"),
        }
    }
}
/// xyY and u′v′Y inversion. Y is relative to reference white, not display nits.
pub fn xyz(point: [f64; 2], uv: bool, y: f64) -> Option<[f64; 3]> {
    let [a, b] = point;
    if !a.is_finite() || !b.is_finite() || !y.is_finite() || b <= 0. || y <= 0. {
        return None;
    }
    let value = if uv {
        [9. * a * y / (4. * b), y, (12. - 3. * a - 20. * b) * y / (4. * b)]
    } else {
        [a * y / b, y, (1. - a - b) * y / b]
    };
    value.iter().all(|v| v.is_finite()).then_some(value)
}
fn contains(p: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let mut previous = polygon[polygon.len() - 1];
    for &current in polygon {
        if (current[1] > p[1]) != (previous[1] > p[1])
            && p[0] < (previous[0] - current[0]) * (p[1] - current[1]) / (previous[1] - current[1]) + current[0]
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}
fn outside(rgb: [f64; 3]) -> bool {
    // Boundary tolerance is in linear RGB units: 64 binary64 epsilons at unit
    // scale, covering the small 3x3 inverse/multiply error at exact primaries.
    rgb.iter().any(|v| *v < -64. * f64::EPSILON || *v > 1. + 64. * f64::EPSILON)
}
fn raster(s: Settings, side: u32, cancelled: impl Fn() -> bool) -> Result<(Vec<[f32; 4]>, usize, usize)> {
    ensure!(
        side > 0 && side <= SIDE && s.luminance > 0. && s.luminance <= 1.,
        "invalid fill dimensions or luminance"
    );
    let polygon: Vec<_> = include_str!("../../../assets/colorimetry/CIE_cc_1931_2deg.csv")
        .lines()
        .map(|line| {
            let v: Vec<f64> = line.split(',').map(|s| s.parse().expect("validated CIE datum")).collect();
            coordinates([v[1], v[2], v[3]], s.uv).expect("CIE spectral coordinate")
        })
        .collect();
    let (xmax, ymax) = if s.uv { (0.65, 0.65) } else { (0.8, 0.9) };
    let mut pixels = Vec::with_capacity((side * side) as usize);
    let mut mapped = 0;
    let mut visible = 0;
    for row in 0..side {
        ensure!(!cancelled(), "fill cancelled");
        for column in 0..side {
            let point = [
                (f64::from(column) + 0.5) / f64::from(side) * xmax,
                (1. - (f64::from(row) + 0.5) / f64::from(side)) * ymax,
            ];
            if !contains(point, &polygon) {
                pixels.push([0.; 4]);
                continue;
            }
            let Some(xyz) = xyz(point, s.uv, s.luminance) else {
                pixels.push([0.; 4]);
                continue;
            };
            let linear = XyzD65::new(xyz)?.to_linear(s.output)?;
            let needs_mapping = outside(linear.components());
            visible += 1;
            mapped += usize::from(needs_mapping);
            let rgb = linear.map_gamut(s.output, s.mapping)?.components();
            let rgb = if s.hatch && needs_mapping && (row + column) % 12 < 2 {
                [0.035; 3]
            } else {
                rgb
            };
            pixels.push([rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.]);
        }
    }
    Ok((pixels, mapped, visible))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn d65_xy_and_uv_recover_independent_xyz() {
        let expected = [0.9504559270516716, 1., 1.0890577507598784];
        for (point, uv) in [([0.3127, 0.3290], false), ([0.1978300066428368, 0.468319994938791], true)] {
            for y in [0.05, 0.18, 0.5, 1.] {
                let actual = xyz(point, uv, y).unwrap();
                for c in 0..3 {
                    assert!((actual[c] - expected[c] * y).abs() <= 32. * f64::EPSILON * y);
                }
            }
        }
        assert!(xyz([0., 0.], false, 0.18).is_none());
        assert!(xyz([f64::NAN, 0.2], true, 0.18).is_none());
    }
    #[test]
    fn fixed_luminance_changes_rgb_cube_membership() {
        // Independent sRGB red primary and D65 Y coefficient.
        let red = XyzD65::new(xyz([0.64, 0.33], false, 0.21263900587151027).unwrap())
            .unwrap()
            .to_linear(RgbSpace::Srgb)
            .unwrap()
            .components();
        for (actual, expected) in red.into_iter().zip([1., 0., 0.]) {
            assert!((actual - expected).abs() <= 64. * f64::EPSILON);
        }
        assert!(!outside(red));
        let brighter = XyzD65::new(xyz([0.64, 0.33], false, 1.).unwrap())
            .unwrap()
            .to_linear(RgbSpace::Srgb)
            .unwrap()
            .components();
        assert!(outside(brighter));
    }
    #[test]
    fn locus_mask_mapping_alpha_and_cancellation() {
        for output in super::super::SPACES {
            for uv in [false, true] {
                for mapping in [GamutMapping::Clip, GamutMapping::ReduceChroma] {
                    let s = Settings { uv, luminance: 0.18, output, mapping, hatch: true };
                    let (pixels, mapped, visible) = raster(s, 24, || false).unwrap();
                    assert!(mapped > 0 && mapped < visible && visible < pixels.len());
                    assert!(pixels.iter().all(|p| p.iter().all(|v| v.is_finite() && (0. ..=1.).contains(v))));
                    assert!(pixels.iter().filter(|p| p[3] == 0.).all(|p| *p == [0.; 4]));
                    assert!(raster(s, 24, || true).is_err());
                }
            }
        }
    }
}
