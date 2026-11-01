//! App prototype for a future SDK color-field primitive.
//! Positions/radii use normalized receiver coordinates, Y down. Angle is degrees
//! clockwise from +X. Stretch is an area-preserving axis ratio; 1 is circular.
//! Input RGB is encoded in the selected output primaries. HSL hues are lifted
//! relative to the background once, avoiding spatially changing hue branches.
//! Ordered preserves the editor's straight-channel lerp and source-over alpha.
//! Normalized is an associated-color weighted mixture, including the background.
use anyhow::{Result, ensure};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mixing {
    #[default]
    Ordered,
    Normalized,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    Srgb,
    DisplayP3,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FieldPoint {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub hsl: [f32; 3],
    pub radius: f32,
    pub angle: f32,
    pub stretch: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ColorField {
    pub points: Vec<FieldPoint>,
    pub background: [f32; 4],
    pub background_hsl: [f32; 3],
    pub softness: f32,
    pub hsl: bool,
    pub mixing: Mixing,
    /// Normalized mode only: background kernel weight, independent of alpha.
    pub background_weight: f32,
}
impl ColorField {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.points.len() <= 16, "Color fields support at most 16 points");
        let color = |c: &[f32; 4]| c.iter().all(|v| v.is_finite() && (0. ..=1.).contains(v));
        ensure!(color(&self.background), "Invalid field background");
        ensure!(
            self.background_hsl.iter().all(|v| v.is_finite())
                && self.background_hsl[1..].iter().all(|v| (0. ..=1.).contains(v)),
            "Invalid background HSL"
        );
        ensure!(self.softness.is_finite() && (0.25..=2.).contains(&self.softness), "Softness must be 0.25..=2");
        ensure!(
            self.background_weight.is_finite() && (0. ..=2.).contains(&self.background_weight),
            "Background weight must be 0..=2"
        );
        for p in &self.points {
            ensure!(p.position.iter().all(|v| v.is_finite() && (0. ..=1.).contains(v)), "Invalid field position");
            ensure!(
                color(&p.color)
                    && p.hsl.iter().all(|v| v.is_finite())
                    && p.hsl[1..].iter().all(|v| (0. ..=1.).contains(v)),
                "Invalid field color"
            );
            ensure!(p.radius.is_finite() && p.radius > 0. && p.radius <= 3., "Invalid field radius");
            ensure!(
                p.angle.is_finite() && p.stretch.is_finite() && (1. / 9. ..=9.).contains(&p.stretch),
                "Invalid field transform"
            );
        }
        Ok(())
    }
    pub fn sample(&self, uv: [f32; 2]) -> [f32; 4] {
        let mut rgb = [self.background[0], self.background[1], self.background[2]];
        let mut hsl = self.background_hsl;
        let mut alpha = self.background[3];
        let mut total = self.background_weight;
        let mut associated_weight = self.background_weight * alpha;
        let mut sum = if self.hsl { hsl } else { rgb }.map(|v| v * associated_weight);
        for p in &self.points {
            let dx = uv[0] - p.position[0];
            let dy = uv[1] - p.position[1];
            let (sin, cos) = p.angle.to_radians().sin_cos();
            let x = cos * dx + sin * dy;
            let y = -sin * dx + cos * dy;
            let distance = if p.stretch == 1. {
                dx * dx + dy * dy
            } else {
                x * x / p.stretch + y * y * p.stretch
            };
            let weight = (-(distance * 5. / p.radius.powi(2)).powf(1. / self.softness)).exp();
            let a = weight * p.color[3];
            if self.mixing == Mixing::Ordered {
                if self.hsl {
                    for (target, value) in hsl.iter_mut().zip(p.hsl) {
                        *target += (value - *target) * a;
                    }
                    rgb = hsl_rgb(hsl);
                } else {
                    for (target, value) in rgb.iter_mut().zip(p.color) {
                        *target += (value - *target) * a;
                    }
                }
                alpha = a + alpha * (1. - a);
            } else {
                total += weight;
                associated_weight += a;
                let values = if self.hsl {
                    p.hsl
                } else {
                    [p.color[0], p.color[1], p.color[2]]
                };
                for (target, value) in sum.iter_mut().zip(values) {
                    *target += value * a;
                }
            }
        }
        if self.mixing == Mixing::Normalized {
            if total <= 1e-12 {
                return self.background;
            }
            if associated_weight <= 1e-12 {
                return [0.; 4];
            }
            rgb = sum.map(|v| v / associated_weight);
            if self.hsl {
                rgb = hsl_rgb(rgb);
            }
            alpha = associated_weight / total;
        }
        [rgb[0], rgb[1], rgb[2], alpha]
    }
}
fn hsl_rgb(hsl: [f32; 3]) -> [f32; 3] {
    let c = gpui::hsla(hsl[0].rem_euclid(1.), hsl[1], hsl[2], 1.).to_rgb();
    [c.r, c.g, c.b]
}
#[cfg(target_os = "macos")]
mod metal;
#[cfg(target_os = "macos")]
pub use metal::{Paint, PipelineCache};

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn fixture() -> ColorField {
        let make = |position, h: f32, s, l, a, radius, angle, stretch| {
            let c = gpui::hsla(h, s, l, a).to_rgb();
            FieldPoint { position, color: [c.r, c.g, c.b, a], hsl: [h, s, l], radius, angle, stretch }
        };
        ColorField {
            points: vec![
                make([0.25, 0.3], 0.1, 0.9, 0.55, 0.6, 0.7, 35., 0.4),
                make([0.7, 0.6], 0.4, 0.8, 0.4, 0.8, 1.2, 120., 3.),
            ],
            background: [0.15, 0.15, 0.15, 0.4],
            background_hsl: [0., 0., 0.15],
            softness: 1.,
            hsl: false,
            mixing: Mixing::Ordered,
            background_weight: 0.25,
        }
    }
    #[test]
    fn normalized_is_order_independent_and_ordered_retains_layering() {
        let mut field = fixture();
        let mut reverse = field.clone();
        reverse.points.reverse();
        assert_ne!(field.sample([0.5, 0.5]), reverse.sample([0.5, 0.5]));
        for hsl in [false, true] {
            field.hsl = hsl;
            field.mixing = Mixing::Normalized;
            reverse = field.clone();
            reverse.points.reverse();
            for uv in [[0., 0.], [0.5, 0.5], [1., 1.]] {
                for (a, b) in field.sample(uv).into_iter().zip(reverse.sample(uv)) {
                    assert!((a - b).abs() < 0.00001);
                }
            }
        }
    }
    #[test]
    fn transparent_normalized_fields_do_not_tint_visible_colors() {
        let mut field = fixture();
        field.mixing = Mixing::Normalized;
        field.background = [0., 0., 1., 0.];
        field.points.truncate(1);
        field.points[0].color = [1., 0., 0., 1.];
        let result = field.sample(field.points[0].position);
        assert_eq!(&result[..3], &[1., 0., 0.]);
        assert!((result[3] - 0.8).abs() < 0.00001);
        field.points[0].color[3] = 0.;
        assert_eq!(field.sample([0.3, 0.3]), [0.; 4]);
    }
    #[test]
    fn validation_handles_empty_coincident_and_invalid_fields() {
        let mut field = fixture();
        field.points.push(field.points[0].clone());
        assert!(field.validate().is_ok());
        field.points.clear();
        assert!(field.validate().is_ok());
        assert_eq!(field.sample([0.5, 0.5]), field.background);
        field.background_weight = f32::NAN;
        assert!(field.validate().is_err());
    }
}
