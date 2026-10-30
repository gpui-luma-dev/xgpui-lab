//! Independent composition check, starting at the actual CPU-packed FP16 bytes.
//! This isolates GPU upload/conversion/composition from source/ICC ingestion.
use super::half;
use anyhow::{Result, ensure};
use gpui::{Bounds, LinearP3Image, ScaledPixels};

// Independent 120-digit Decimal primary/white xy solve, using
// xgpui/scripts/generate_adobe_rgb_references.py: inverse(P3) * Rec.2020.
// No production Rust or shader matrix is called by this reference.
const REC_TO_P3: [[f64; 3]; 3] = [
    [1.343578252584332, -0.2821796705261357, -0.06139858205819628],
    [-0.06529745278911953, 1.0757879158485746, -0.010490463059454957],
    [0.0028217872617009514, -0.01959849452449406, 1.0167767072627931],
];

fn reference(stored: [f64; 4], rec: bool, effects: bool) -> ([f64; 4], [f64; 4]) {
    let rgb: [f64; 3] = std::array::from_fn(|c| {
        if rec {
            (0..3).map(|i| REC_TO_P3[c][i] * stored[i]).sum()
        } else {
            stored[c]
        }
    });
    let energy: [f64; 3] = std::array::from_fn(|c| {
        if rec {
            (0..3).map(|i| (REC_TO_P3[c][i] * stored[i]).abs()).sum()
        } else {
            stored[c].abs()
        }
    });
    // These are the exact f32 opacity values passed to the two layer effects.
    let opacity = if effects {
        f64::from(0.65_f32) * f64::from(0.8_f32)
    } else {
        1.
    };
    let a = stored[3] * opacity;
    let expected = [rgb[0] * opacity + 1. - a, rgb[1] * opacity + 1. - a, rgb[2] * opacity + 1. - a, 1.];
    // Binary16 round-to-nearest has relative error <= 2^-11 plus 2^-25
    // absolute for subnormals. Direct: conversion/output writes (3 allowance).
    // Effects: capture, two blur passes, nested opacity/composition (12 allowance).
    // Sum absolute source terms handles cancellation; alpha error propagates
    // through the white backdrop in captures. f32 matrix arithmetic is separate.
    let writes = if effects { 12. } else { 3. };
    let budget = std::array::from_fn(|c| {
        if c == 3 {
            return 0.;
        } // Opaque white backdrop must leave alpha exactly one.
        let alpha_energy = if effects { stored[3] } else { 0. };
        writes * (2f64.powi(-11) * (energy[c] + expected[c].abs() + alpha_energy) + 2f64.powi(-25))
            + 8. * f64::from(f32::EPSILON) * energy[c]
    });
    (expected, budget)
}

pub fn verify(
    image: &LinearP3Image,
    bounds: Bounds<ScaledPixels>,
    stored: [f64; 4],
    rec: bool,
    effects: bool,
) -> Result<String> {
    let width = image.size.width.0 as usize;
    let height = image.size.height.0 as usize;
    ensure!(width > 0 && height > 0 && image.pixels.len() == width * height, "Invalid GPU snapshot");
    ensure!(
        bounds.origin.x.0 >= 0.
            && bounds.origin.y.0 >= 0.
            && bounds.right().0 <= width as f32
            && bounds.bottom().0 <= height as f32,
        "Scroll until the entire native sample is visible, then check again"
    );
    let x = (bounds.origin.x.0 + bounds.size.width.0 / 2.).floor() as usize;
    let y = (bounds.origin.y.0 + bounds.size.height.0 / 2.).floor() as usize;
    ensure!(x > 0 && y > 0 && x + 1 < width && y + 1 < height, "Sample center is outside the snapshot");
    let (expected, budget) = reference(stored, rec, effects);
    let mut worst = 0_f64;
    for row in y - 1..=y + 1 {
        for col in x - 1..=x + 1 {
            let actual = image.pixels[row * width + col].map(half);
            for c in 0..4 {
                let error = (actual[c] - expected[c]).abs();
                ensure!(
                    actual[c].is_finite() && error <= budget[c],
                    "GPU mismatch at ({col},{row}) channel {c}: {} vs {} (budget {})",
                    actual[c],
                    expected[c],
                    budget[c]
                );
                if budget[c] > 0. {
                    worst = worst.max(error / budget[c]);
                }
            }
        }
    }
    let center = image.pixels[y * width + x].map(half);
    let budgets = budget.map(|v| format!("{v:.3e}"));
    Ok(format!(
        "GPU PASS (snapshot) • 9 center pixels • linear P3 RGBA {center:.7?}\nIndependent expected {expected:.7?} • budgets {budgets:?} • worst {:.1}% of budget. Packed FP16 → GPU composition, before display color management; center-only check, not a blur-edge or monitor measurement.",
        worst * 100.
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{DevicePixels, point, size};
    fn fixture() -> (LinearP3Image, Bounds<ScaledPixels>) {
        (
            LinearP3Image {
                size: size(DevicePixels(5), DevicePixels(5)),
                pixels: vec![[0xb000, 0x3d00, 0, 0x3c00]; 25],
            },
            Bounds::new(point(ScaledPixels(0.), ScaledPixels(0.)), size(ScaledPixels(5.), ScaledPixels(5.))),
        )
    }
    #[test]
    fn catches_extended_channel_clamping_nonfinite_and_alpha_errors() {
        let (mut frame, b) = fixture();
        let stored = [-0.125, 1.25, 0., 1.];
        assert!(verify(&frame, b, stored, false, false).is_ok());
        for (channel, bad) in [(0, 0), (1, 0x3c00), (2, 0x7e00), (3, 0)] {
            frame.pixels[12][channel] = bad;
            assert!(verify(&frame, b, stored, false, false).is_err());
            frame = fixture().0;
        }
    }
    #[test]
    fn dark_budgets_scale_and_offscreen_checks_fail() {
        let (_, dark) = reference([1e-4, 1e-4, 1e-4, 1.], false, false);
        assert!(dark[0] < 5e-7);
        let (frame, mut b) = fixture();
        b.origin.y = ScaledPixels(-1.);
        assert!(verify(&frame, b, [-0.125, 1.25, 0., 1.], false, false).is_err());
    }
}
