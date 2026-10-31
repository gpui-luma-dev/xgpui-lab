//! Main-pass GPU chromaticity chart. The CPU model remains the color oracle.
use std::fmt::Write;
use xgpui_color::{LinearRgb, WGSL_COLOR_MATH};
use xgpui_shader::instanced::{InstanceField, ShaderInstance, ShaderProgram};
use super::chromaticity_model as model;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Parameters {
    pub simulated: f32,
    pub luminance: f32,
    pub reduce_chroma: f32,
    pub show_fill: f32,
    pub source_x: f32,
    pub source_y: f32,
    pub source_visible: f32,
    pub overlays: f32,
}
impl ShaderInstance for Parameters {
    const FIELDS: &'static [InstanceField] = &[
        InstanceField { name: "simulated", offset: 0, components: 1 },
        InstanceField { name: "luminance", offset: 4, components: 1 },
        InstanceField { name: "reduce_chroma", offset: 8, components: 1 },
        InstanceField { name: "show_fill", offset: 12, components: 1 },
        InstanceField { name: "source_x", offset: 16, components: 1 },
        InstanceField { name: "source_y", offset: 20, components: 1 },
        InstanceField { name: "source_visible", offset: 24, components: 1 },
        InstanceField { name: "overlays", offset: 28, components: 1 },
    ];
}

// At a 4K plot width the simplification error is below 0.05 logical pixels.
// Preserve measured CSV coordinates; this only removes redundant line segments.
const LOCUS_TOLERANCE: f64 = 1e-5;
fn distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let edge = [b[0] - a[0], b[1] - a[1]];
    let t = (((p[0] - a[0]) * edge[0] + (p[1] - a[1]) * edge[1]) / (edge[0] * edge[0] + edge[1] * edge[1]).max(1e-30))
        .clamp(0., 1.);
    (p[0] - a[0] - edge[0] * t).hypot(p[1] - a[1] - edge[1] * t)
}
fn simplify(points: &[[f64; 2]], output: &mut Vec<[f64; 2]>) {
    if points.len() <= 2 {
        output.push(points[0]);
        return;
    }
    let (index, farthest) = points[1..points.len() - 1]
        .iter()
        .enumerate()
        .map(|(i, &p)| (i + 1, distance(p, points[0], points[points.len() - 1])))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .expect("interior points");
    if farthest > LOCUS_TOLERANCE {
        simplify(&points[..=index], output);
        simplify(&points[index..], output);
    } else {
        output.push(points[0]);
    }
}
fn outline() -> Vec<[f64; 2]> {
    let points = model::locus();
    let mut result = Vec::new();
    simplify(&points, &mut result);
    result.push(points[points.len() - 1]);
    result
}
fn source() -> anyhow::Result<String> {
    let locus = outline();
    let mut source = String::from(WGSL_COLOR_MATH);
    writeln!(
        source,
        "\nconst LOCUS_COUNT: u32 = {}u;\nconst LOCUS: array<vec2<f32>, {}> = array(",
        locus.len(),
        locus.len()
    )?;
    for p in locus {
        writeln!(source, "vec2({:.10}, {:.10}),", p[0], p[1])?;
    }
    writeln!(source, ");\nconst PRIMARIES: array<vec2<f32>, 12> = array(")?;
    for space in model::SPACES {
        for channel in 0..3 {
            let xyz = LinearRgb::new(space, std::array::from_fn(|i| if i == channel { 1. } else { 0. }))?
                .to_xyz()?
                .components();
            let p = model::xy(xyz).ok_or_else(|| anyhow::anyhow!("Invalid reference primary"))?;
            writeln!(source, "vec2({:.10}, {:.10}),", p[0], p[1])?;
        }
    }
    writeln!(source, ");\nconst OUTLINE_COLORS: array<vec3<f32>, 4> = array(")?;
    for color in model::COLORS {
        writeln!(
            source,
            "vec3({:.8}, {:.8}, {:.8}),",
            ((color >> 16) & 255) as f32 / 255.,
            ((color >> 8) & 255) as f32 / 255.,
            (color & 255) as f32 / 255.
        )?;
    }
    writeln!(source, ");\n{}", include_str!("chromaticity.wgsl"))?;
    Ok(source)
}
pub(super) fn program() -> anyhow::Result<ShaderProgram<Parameters>> {
    ShaderProgram::new(&source()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_validates_and_preserves_the_spectral_locus_with_subpixel_error() {
        program().expect("portable shader and typed ABI");
        let simplified = outline();
        assert!(simplified.len() < 180);
        for p in model::locus() {
            let nearest = simplified.windows(2).map(|edge| distance(p, edge[0], edge[1])).fold(f64::INFINITY, f64::min);
            assert!(nearest <= LOCUS_TOLERANCE * 1.001, "{p:?}: {nearest}");
        }
    }
}

#[cfg(all(test, feature = "test-support", target_os = "macos"))]
mod gpu_tests {
    use super::{model, outline, distance, program, Parameters};
    use gpui::{
        Bounds, ContentMask, DevicePixels, PlatformHeadlessRenderer, Primitive, ScaledPixels, Scene, ShaderDraw, point,
        size,
    };
    use xgpui_color::{GamutMapping, RgbSpace, XyzD65};

    fn capture(
        renderer: &mut dyn PlatformHeadlessRenderer,
        shader: &xgpui_shader::instanced::ShaderProgram<Parameters>,
        parameters: Parameters,
        backing_scale: f32,
    ) -> anyhow::Result<image::RgbaImage> {
        let extent = size(DevicePixels((600. * backing_scale) as i32), DevicePixels((420. * backing_scale) as i32));
        let bounds = Bounds::new(
            point(ScaledPixels(0.), ScaledPixels(0.)),
            size(ScaledPixels(600. * backing_scale), ScaledPixels(420. * backing_scale)),
        );
        let mut data = [[0.; 4]; gpui::SHADER_DATA_SLOTS];
        for (i, v) in bytemuck::cast_slice::<_, f32>(&[parameters]).iter().enumerate() {
            data[i / 4][i % 4] = *v;
        }
        let mut scene = Scene::default();
        // A distinct enclosure color proves empty shader pixels expose the host.
        scene.insert_primitive(gpui::Quad {
            bounds,
            content_mask: ContentMask { bounds },
            background: gpui::rgb(0x293b4d).into(),
            ..Default::default()
        });
        scene.insert_primitive(Primitive::Shader(ShaderDraw {
            order: 0,
            bounds,
            content_mask: ContentMask { bounds },
            opacity: 1.,
            scale_factor: backing_scale,
            program: shader.scene_program(),
            data,
            path_mask: None,
            glyph_mask: None,
        }));
        scene.finish();
        renderer.render_scene_to_image(&scene, extent)
    }

    #[test]
    #[ignore = "requires a native Metal device"]
    fn native_gpu_matches_cpu_color_math_at_1x_and_2x_and_captures_smooth_edges() -> anyhow::Result<()> {
        let shader = program()?;
        let mut renderer = gpui_apple::metal_renderer::MetalHeadlessRenderer::new();
        let boundary = model::locus();
        let simplified = outline();
        let logical_scale = (420. - 48.) / 0.9;
        let left = (600. - logical_scale * 0.8) * 0.5;
        let mut samples = 0;
        for backing_scale in [1., 2.] {
            for (index, space) in model::SPACES.into_iter().enumerate() {
                for (reduce, mapping) in [(0., GamutMapping::Clip), (1., GamutMapping::ReduceChroma)] {
                    let parameters = Parameters {
                        simulated: index as f32,
                        luminance: 0.18,
                        reduce_chroma: reduce,
                        show_fill: 1.,
                        source_x: 0.3,
                        source_y: 0.6,
                        source_visible: 1.,
                        overlays: 0.,
                    };
                    let image = capture(&mut renderer, &shader, parameters, backing_scale)?;
                    let cells = model::cells(space, 0.18, mapping, &boundary)?;
                    for cell in cells.iter().step_by(19) {
                        let center = [cell.xy[0] + 0.4 / 48., cell.xy[1] + 0.45 / 54.];
                        let x = ((left + center[0] * logical_scale) * f64::from(backing_scale)) as u32;
                        let y = ((396. - center[1] * logical_scale) * f64::from(backing_scale)) as u32;
                        let xy = [
                            ((f64::from(x) + 0.5) / f64::from(backing_scale) - left) / logical_scale,
                            (396. - (f64::from(y) + 0.5) / f64::from(backing_scale)) / logical_scale,
                        ];
                        if simplified.windows(2).map(|e| distance(xy, e[0], e[1])).fold(f64::INFINITY, f64::min)
                            * logical_scale
                            < 3.
                        {
                            continue;
                        }
                        let xyz = XyzD65::new([xy[0] * 0.18 / xy[1], 0.18, (1. - xy[0] - xy[1]) * 0.18 / xy[1]])?;
                        let rgb = xyz
                            .to_linear(space)?
                            .map_gamut(space, mapping)?
                            .map_gamut(RgbSpace::Srgb, GamutMapping::Clip)?
                            .encode()?
                            .components();
                        let actual = image.get_pixel(x, y).0;
                        let expected = rgb.map(|v| (v.clamp(0., 1.) * 255.).round() as u8);
                        anyhow::ensure!(
                            actual[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 4),
                            "{space:?}/{mapping:?} scale={backing_scale} xy={xy:?}: {actual:?} != {expected:?}"
                        );
                        samples += 1;
                    }
                }
            }
            let parameters = Parameters {
                simulated: 3.,
                luminance: 0.18,
                reduce_chroma: 0.,
                show_fill: 1.,
                source_x: 0.3,
                source_y: 0.6,
                source_visible: 1.,
                overlays: 1.,
            };
            let image = capture(&mut renderer, &shader, parameters, backing_scale)?;
            anyhow::ensure!(
                image.get_pixel(20, 20).0 == [41, 59, 77, 255],
                "Empty plot enclosure must show the host background"
            );
            image.save(format!("/tmp/chromaticity-shader-{}x.png", backing_scale as u32))?;
            let no_fill = capture(&mut renderer, &shader, Parameters { show_fill: 0., ..parameters }, backing_scale)?;
            anyhow::ensure!(
                image.pixels().zip(no_fill.pixels()).filter(|(a, b)| a.0 != b.0).count() > 1000,
                "Fill toggle must affect GPU output"
            );
        }
        anyhow::ensure!(samples > 500, "insufficient CPU/GPU reference samples: {samples}");
        Ok(())
    }
}
