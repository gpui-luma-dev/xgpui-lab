use std::sync::Arc;
use gpui::{Hsla, Pixels, RenderImage, Size};
use super::MeshPoint;
use super::super::color_field::{ColorField, FieldPoint, Mixing};

pub struct FieldShape<'a> {
    pub mixing: Mixing,
    pub background_weight: f32,
    pub point_transforms: &'a [[f32; 2]],
    pub point_spreads: &'a [f32],
    pub softness_percent: f32,
}

fn field_distance(dx: f32, dy: f32, angle: f32, stretch: f32) -> f32 {
    if stretch == 1. { return dx * dx + dy * dy; }
    let (sin, cos) = angle.to_radians().sin_cos();
    let x = cos * dx + sin * dy;
    let y = -sin * dx + cos * dy;
    let stretch = stretch.clamp(1. / 9., 9.);
    x * x / stretch + y * y * stretch
}

fn field_weight(distance_squared: f32, spread_percent: f32, point_spread: f32, softness_percent: f32) -> f32 {
    let radius = (spread_percent.clamp(10.0, 150.0) / 100.0) * (point_spread.clamp(10.0, 200.0) / 100.0);
    // Keep the e^-1 contour fixed while varying the fade profile.
    let exponent = 100.0 / softness_percent.clamp(25.0, 200.0);
    (-(distance_squared * 5.0 / radius.powi(2)).powf(exponent)).exp()
}

// Lift palette hues onto one fixed axis. Choosing a shortest arc from the
// spatially varying intermediate color can flip at 180 degrees and create seams.
fn palette_hues(points: &[MeshPoint], background: Hsla) -> Vec<f32> {
    points
        .iter()
        .map(|point| {
            let midpoint = gpui_luma_color::color_slider::color_spec::interpolate_hsl(background, point.color, 0.5);
            let half_delta = (midpoint.h - background.h + 0.5).rem_euclid(1.0) - 0.5;
            background.h + half_delta * 2.0
        })
        .collect()
}

#[cfg(test)]
fn sample(points: &[MeshPoint], background: Hsla, u: f32, v: f32, spread_percent: f32, use_hsl: bool) -> gpui::Rgba {
    let hues = use_hsl.then(|| palette_hues(points, background));
    sample_with_hues(
        points,
        background,
        u,
        v,
        spread_percent,
        hues.as_deref(),
        &FieldShape { mixing: Mixing::Ordered, background_weight: 0.25, point_transforms: &[], point_spreads: &[], softness_percent: 100.0 },
    )
}

// Soft radial fields layered over a base color, independent of mesh topology.
fn sample_with_hues(
    points: &[MeshPoint],
    background: Hsla,
    u: f32,
    v: f32,
    spread_percent: f32,
    hues: Option<&[f32]>,
    shape: &FieldShape<'_>,
) -> gpui::Rgba {
    let mut result = background.to_rgb();
    let mut hsl = background;
    for (index, point) in points.iter().enumerate() {
        let [angle, stretch] = shape.point_transforms.get(index).copied().unwrap_or([0., 1.]);
        let distance_squared = field_distance(u - point.u, v - point.v, angle, stretch);
        let alpha = field_weight(
            distance_squared,
            spread_percent,
            shape.point_spreads.get(index).copied().unwrap_or(100.0),
            shape.softness_percent,
        ) * point.color.a;
        let color = point.color.to_rgb();
        let output_alpha = alpha + result.a * (1.0 - alpha);
        if let Some(hues) = hues {
            hsl.h += (hues[index] - hsl.h) * alpha;
            hsl.s += (point.color.s - hsl.s) * alpha;
            hsl.l += (point.color.l - hsl.l) * alpha;
            result = gpui::hsla(hsl.h.rem_euclid(1.0), hsl.s, hsl.l, output_alpha).to_rgb();
        } else {
            result.r += (color.r - result.r) * alpha;
            result.g += (color.g - result.g) * alpha;
            result.b += (color.b - result.b) * alpha;
        }
        result.a = output_alpha;
    }
    result
}

/// Keep editor adaptation outside the reusable field primitive.
pub fn build_color_field(points: &[MeshPoint], background: Hsla, spread: f32, hsl: bool, shape: &FieldShape<'_>) -> ColorField {
    let rgba = |c: Hsla| { let rgb=c.to_rgb(); [rgb.r,rgb.g,rgb.b,c.a] };
    let hues=palette_hues(points,background);
    ColorField {
        points: points.iter().enumerate().map(|(i,p)| {
            let [angle,stretch]=shape.point_transforms.get(i).copied().unwrap_or([0.,1.]);
            FieldPoint {position:[p.u,p.v],color:rgba(p.color),hsl:[hues[i],p.color.s,p.color.l],
                radius:spread.clamp(10.,150.)/100.*shape.point_spreads.get(i).copied().unwrap_or(100.).clamp(10.,200.)/100.,angle,stretch}
        }).collect(),
        background:rgba(background),background_hsl:[background.h,background.s,background.l],softness:shape.softness_percent.clamp(25.,200.)/100.,
        hsl,mixing:shape.mixing,background_weight:shape.background_weight,
    }
}

pub fn rasterize_freeform_preview(
    size: Size<Pixels>,
    points: &[MeshPoint],
    background: Hsla,
    spread_percent: f32,
    use_hsl: bool,
    shape: &FieldShape<'_>,
) -> Option<Arc<RenderImage>> {
    let width = size.width.as_f32().round() as u32;
    let height = size.height.as_f32().round() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    let hues = use_hsl.then(|| palette_hues(points, background));
    let normalized = (shape.mixing == Mixing::Normalized).then(|| build_color_field(points, background, spread_percent, use_hsl, shape));
    for y in 0..height {
        for x in 0..width {
            let color = if let Some(field) = &normalized {
                let c=field.sample([(x as f32+0.5)/width as f32,(y as f32+0.5)/height as f32]);
                gpui::Rgba {r:c[0],g:c[1],b:c[2],a:c[3]}
            } else { sample_with_hues(
                points,
                background,
                (x as f32 + 0.5) / width as f32,
                (y as f32 + 0.5) / height as f32,
                spread_percent,
                hues.as_deref(),
                shape,
            ) };
            pixmap.pixels_mut()[(y * width + x) as usize] = tiny_skia::PremultipliedColorU8::from_rgba(
                (color.b * color.a * 255.0).round() as u8,
                (color.g * color.a * 255.0).round() as u8,
                (color.r * color.a * 255.0).round() as u8,
                (color.a * 255.0).round() as u8,
            )?;
        }
    }
    let buffer = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, pixmap.data().to_vec())?;
    Some(Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(buffer)])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spread_scales_radius_and_softness_preserves_reference_contour() {
        assert!((field_weight(0.05, 100.0, 50.0, 100.0) - field_weight(0.2, 100.0, 100.0, 100.0)).abs() < 0.00001);
        for softness in [25.0, 100.0, 200.0] {
            assert_eq!(field_weight(0.0, 100.0, 100.0, softness), 1.0);
            assert!((field_weight(0.2, 100.0, 100.0, softness) - (-1.0_f32).exp()).abs() < 0.00001);
        }
        assert!(field_weight(0.4, 100.0, 100.0, 200.0) > field_weight(0.4, 100.0, 100.0, 25.0));
    }

    #[test]
    fn overlapping_opposing_hues_have_no_spatial_seam() {
        let background = gpui::hsla(0.0, 1.0, 0.5, 1.0);
        let points = [
            MeshPoint { row: 0, col: 0, u: 0.5, v: 0.5, color: gpui::hsla(0.4, 1.0, 0.5, 1.0) },
            MeshPoint { row: 0, col: 1, u: 0.8, v: 0.5, color: gpui::hsla(0.8, 1.0, 0.5, 1.0) },
        ];
        // The first field crosses hue 0.3 here, opposite the second field's 0.8.
        let crossing = 0.5 - (-0.75_f32.ln() / 5.0).sqrt();
        let left = sample(&points, background, crossing - 0.0001, 0.5, 100.0, true);
        let right = sample(&points, background, crossing + 0.0001, 0.5, 100.0, true);
        assert!((left.r - right.r).abs() < 0.01);
        assert!((left.g - right.g).abs() < 0.01);
        assert!((left.b - right.b).abs() < 0.01);
    }

    #[test]
    fn hsl_blends_through_hue_wheel_and_preserves_composited_alpha() {
        let background = gpui::hsla(0.0, 1.0, 0.5, 0.4);
        let point = MeshPoint { row: 0, col: 0, u: 0.5, v: 0.5, color: gpui::hsla(1.0 / 3.0, 1.0, 0.5, 0.5) };
        let hsl = sample(&[point], background, 0.5, 0.5, 100.0, true);
        let rgb = sample(&[point], background, 0.5, 0.5, 100.0, false);
        assert!((hsl.r - 1.0).abs() < 0.001 && (hsl.g - 1.0).abs() < 0.001);
        assert!((rgb.r - 0.5).abs() < 0.001 && (rgb.g - 0.5).abs() < 0.001);
        assert!((hsl.a - 0.7).abs() < 0.001);
        assert_eq!(hsl.a, rgb.a);
    }

    #[test]
    fn empty_fields_preserve_background() {
        let background = gpui::hsla(0.6, 0.4, 0.5, 0.7);
        assert_eq!(sample(&[], background, 0.5, 0.5, 100.0, false), background.to_rgb());
    }

    #[test]
    fn smaller_spread_tightens_field_without_changing_center() {
        let background = gpui::hsla(0.0, 0.0, 0.0, 1.0);
        let point = MeshPoint { row: 0, col: 0, u: 0.5, v: 0.5, color: gpui::hsla(0.0, 1.0, 0.5, 1.0) };
        assert!(
            sample(&[point], background, 0.7, 0.5, 25.0, false).r
                < sample(&[point], background, 0.7, 0.5, 100.0, false).r
        );
        assert_eq!(sample(&[point], background, 0.5, 0.5, 25.0, false), point.color.to_rgb());
    }

    #[test]
    fn field_follows_its_position_and_fades_smoothly() {
        let background = gpui::hsla(0.0, 0.0, 0.0, 1.0);
        let mut point = MeshPoint { row: 0, col: 0, u: 0.2, v: 0.3, color: gpui::hsla(0.0, 1.0, 0.5, 1.0) };
        assert_eq!(sample(&[point], background, 0.2, 0.3, 100.0, false), point.color.to_rgb());
        assert!(
            sample(&[point], background, 0.4, 0.3, 100.0, false).r
                > sample(&[point], background, 0.8, 0.3, 100.0, false).r
        );
        point.u = 0.8;
        assert_eq!(sample(&[point], background, 0.8, 0.3, 100.0, false), point.color.to_rgb());
    }
}

#[cfg(test)]
mod transform_tests {
    use super::*;
    #[test]
    fn stretch_rotates_ellipse_and_preserves_area_and_neutral_shape() {
        assert_eq!(field_distance(0.3, 0.4, 37., 1.), 0.25);
        let along_x = field_distance(0.4, 0., 0., 4.);
        let along_y = field_distance(0., 0.4, 0., 4.);
        assert!(along_x < along_y);
        assert!(field_distance(0.4, 0., 0., 0.25) > field_distance(0.4, 0., 0., 1.));
        assert!((field_distance(0.1, 0., 0., 0.25) - 0.04).abs() < 0.00001);
        assert!((along_x - field_distance(0., 0.4, 90., 4.)).abs() < 0.00001);
        let radius = 0.2_f32;
        assert!((field_distance(radius * 2., 0., 0., 4.) - radius.powi(2)).abs() < 0.00001);
        assert!((field_distance(0., radius / 2., 0., 4.) - radius.powi(2)).abs() < 0.00001);
    }
}

#[cfg(test)]
mod prototype_parity_tests {
    use super::*;
    #[test]
    fn ordered_field_reference_matches_original_editor_algorithm() {
        let background=gpui::hsla(0.15,0.6,0.3,0.35);
        let points=vec![
            MeshPoint {row:0,col:0,u:0.3,v:0.2,color:gpui::hsla(0.8,0.8,0.5,0.6)},
            MeshPoint {row:0,col:1,u:0.7,v:0.8,color:gpui::hsla(0.4,0.7,0.3,0.9)},
        ];
        let shape=FieldShape {mixing:Mixing::Ordered,background_weight:0.25,point_transforms:&[[70.,0.5],[120.,3.]],point_spreads:&[50.,140.],softness_percent:75.};
        for hsl in [false,true] {
            let field=build_color_field(&points,background,110.,hsl,&shape);
            let hues=hsl.then(||palette_hues(&points,background));
            for y in 0..10 {for x in 0..10 {
                let uv=[(x as f32+0.5)/10.,(y as f32+0.5)/10.];
                let original=sample_with_hues(&points,background,uv[0],uv[1],110.,hues.as_deref(),&shape);
                for (a,b) in field.sample(uv).into_iter().zip([original.r,original.g,original.b,original.a]) {assert!((a-b).abs()<0.00001,"{a} != {b}");}
            }}
        }
    }
}
