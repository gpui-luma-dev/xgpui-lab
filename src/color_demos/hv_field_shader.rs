//! App-owned GPU background for the SDK's white–hue–black field with a 6 px endpoint strip.
use std::sync::Arc;
use super::source_field::{FieldSettings, ENDPOINT_STRIP_WIDTH};
use gpui_luma_color::color_field::ColorFieldBackgroundPainter;
use xgpui_shader::{
    WindowShaderExt,
    instanced::{InstanceField, ShaderInstance, ShaderProgram},
};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Parameters {
    pub strip_width: f32,
    pub radius: f32,
    pub source_space: f32,
    pub padding1: f32,
}
impl ShaderInstance for Parameters {
    const FIELDS: &'static [InstanceField] = &[
        InstanceField { name: "strip_width", offset: 0, components: 1 },
        InstanceField { name: "radius", offset: 4, components: 1 },
        InstanceField { name: "source_space", offset: 8, components: 1 },
        InstanceField { name: "padding1", offset: 12, components: 1 },
    ];
}
pub(super) fn program() -> anyhow::Result<ShaderProgram<Parameters>> {
    ShaderProgram::new(&format!("{}\n{}", xgpui_color::WGSL_COLOR_MATH, include_str!("hv_field.wgsl")))
}
pub(super) fn painter(settings: FieldSettings) -> anyhow::Result<ColorFieldBackgroundPainter> {
    let program = program()?;
    Ok(Arc::new(move |bounds, _, window, _| {
        window.paint_custom_shader(
            bounds,
            &program,
            Parameters {
                strip_width: ENDPOINT_STRIP_WIDTH,
                radius: 0.,
                source_space: settings.source_index() as f32,
                padding1: 0.,
            },
        )
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn portable_shader_and_parameter_layout_validate() {
        super::program().expect("portable white–hue–black shader and 16-byte instance ABI");
    }
}

#[cfg(all(test, feature = "test-support", target_os = "macos"))]
mod gpu_tests {
    use super::{Parameters, program};
    use crate::color_demos::source_field::{FieldSettings, SPACES};
    use gpui::{
        Bounds, ContentMask, DevicePixels, PlatformHeadlessRenderer, Primitive, ScaledPixels, Scene, ShaderDraw, point,
        size,
    };
    use gpui_luma_color::{
        color_field::{ColorFieldModel2D},
        color_slider::color_spec::Hsv,
    };

    #[test]
    #[ignore = "requires a native Metal device"]
    fn native_hv_shader_matches_luma_model_at_1x_and_2x() -> anyhow::Result<()> {
        let shader = program()?;
        let mut renderer = gpui_apple::metal_renderer::MetalHeadlessRenderer::new();
        let mut checked = 0;
        for scale in [1., 2.] {
            let width = (600. * scale) as u32;
            let height = (190. * scale) as u32;
            let bounds = Bounds::new(
                point(ScaledPixels(0.), ScaledPixels(0.)),
                size(ScaledPixels(width as f32), ScaledPixels(height as f32)),
            );
            for (source_index, space) in SPACES.into_iter().enumerate() {
                let settings = FieldSettings::new(source_index);
                settings.resize(600.);
                let model = settings.model();
                for saturation in [0., 0.2, 0.7, 1.] {
                    let parameters =
                        Parameters { strip_width: 6., radius: 0., source_space: source_index as f32, padding1: 0. };
                    let mut data = [[0.; 4]; gpui::SHADER_DATA_SLOTS];
                    for (i, value) in bytemuck::cast_slice::<_, f32>(&[parameters]).iter().enumerate() {
                        data[i / 4][i % 4] = *value;
                    }
                    let mut scene = Scene::default();
                    scene.insert_primitive(Primitive::Shader(ShaderDraw {
                        order: 0,
                        bounds,
                        content_mask: ContentMask { bounds },
                        opacity: 1.,
                        scale_factor: scale,
                        program: shader.scene_program(),
                        data,
                        path_mask: None,
                        glyph_mask: None,
                    }));
                    scene.finish();
                    let image = renderer
                        .render_scene_to_image(&scene, size(DevicePixels(width as i32), DevicePixels(height as i32)))?;
                    for (x, y) in [(0, 0), (width - 1, 0), (0, height - 1), (width - 1, height - 1)] {
                        anyhow::ensure!(image.get_pixel(x, y).0[3] == 255, "square corners must remain opaque");
                    }
                    let inside = width - (3. * scale) as u32;
                    anyhow::ensure!(
                        image.get_pixel(inside, height / 4).0 == [255, 255, 255, 255],
                        "upper strip must be white"
                    );
                    anyhow::ensure!(
                        image.get_pixel(inside, height * 3 / 4).0 == [0, 0, 0, 255],
                        "lower strip must be black"
                    );
                    let outside = width - (7. * scale) as u32;
                    anyhow::ensure!(
                        image.get_pixel(outside, height / 4).0[1] < 250,
                        "strip must be only 6 logical pixels wide"
                    );
                    let hsv = Hsv { h: 0., s: saturation, v: 1., a: 1. };
                    for y in (8..height - 8).step_by(17) {
                        for x in (8..width - 8).step_by(23) {
                            let uv = ((x as f32 + 0.5) / width as f32, (y as f32 + 0.5) / height as f32);
                            let expected: gpui::Rgba = model.color_at_uv(&hsv, uv).into();
                            let expected =
                                [expected.r, expected.g, expected.b].map(|v| (v.clamp(0., 1.) * 255.).round() as u8);
                            let actual = image.get_pixel(x, y).0;
                            anyhow::ensure!(
                                actual[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 4),
                                "{space:?}, s={saturation}, scale={scale}, uv={uv:?}: {actual:?} != {expected:?}"
                            );
                            checked += 1;
                        }
                    }
                    // Regression for the reported P3 bands at 600 × 190 logical pixels.
                    // Exclude the deliberate endpoint strip. Rec.2020 can have steeper
                    // legitimate transfer-function slopes and is covered by pixel parity.
                    if saturation == 0.7 && space == xgpui_color::RgbSpace::DisplayP3 {
                        let limit = width - (6. * scale) as u32;
                        let mut largest_jump = 0u8;
                        for y in 0..height - 1 {
                            for x in 0..limit - 1 {
                                let pixel = image.get_pixel(x, y).0;
                                for next in [image.get_pixel(x + 1, y).0, image.get_pixel(x, y + 1).0] {
                                    for channel in 0..3 {
                                        largest_jump = largest_jump.max(pixel[channel].abs_diff(next[channel]));
                                    }
                                }
                            }
                        }
                        anyhow::ensure!(
                            largest_jump <= 32,
                            "{space:?}, scale={scale}: discontinuous preview, adjacent-pixel jump {largest_jump}/255"
                        );
                    }
                    if saturation == 0.7 {
                        image.save(format!("/tmp/hsv-field-source-{source_index}-{}x.png", scale as u32))?;
                    }
                }
            }
        }
        anyhow::ensure!(checked > 12000, "expected broad sampling of the field");
        Ok(())
    }
}
