//! Cached native-color programs shared by the spectrum, swatches and SDK tracks.
use std::sync::Arc;
use gpui::{Bounds, Pixels, Window};
use gpui_luma_color::{
    color_field::ColorFieldBackgroundPainter,
    color_slider::{ColorSliderDelegate, ColorSliderTrackContext, color_spec::Hsv, types::Axis},
};
use xgpui_shader::{
    WindowShaderExt,
    instanced::{InstanceField, ShaderInstance, ShaderProgram},
};
use super::source_field::{self, FieldSettings, ENDPOINT_STRIP_WIDTH};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Parameters {
    pub color: [f32; 4],
    pub source_space: f32,
    pub kind: f32,
    pub channel: f32,
    pub vertical: f32,
    pub radius: f32,
    pub strip_width: f32,
    pub reversed: f32,
    pub sdr_fallback: f32,
}
impl ShaderInstance for Parameters {
    const FIELDS: &'static [InstanceField] = &[
        InstanceField { name: "color", offset: 0, components: 4 },
        InstanceField { name: "source_space", offset: 16, components: 1 },
        InstanceField { name: "kind", offset: 20, components: 1 },
        InstanceField { name: "channel", offset: 24, components: 1 },
        InstanceField { name: "vertical", offset: 28, components: 1 },
        InstanceField { name: "radius", offset: 32, components: 1 },
        InstanceField { name: "strip_width", offset: 36, components: 1 },
        InstanceField { name: "reversed", offset: 40, components: 1 },
        InstanceField { name: "sdr_fallback", offset: 44, components: 1 },
    ];
}
impl Parameters {
    pub fn field(source: usize, saturation: f32) -> Self {
        Self {
            color: [saturation, 1., 0., 1.],
            kind: 0.,
            strip_width: ENDPOINT_STRIP_WIDTH,
            ..Self::solid(source, [0.; 3], 0.)
        }
    }
    pub fn with_contour(mut self, enabled: bool) -> Self {
        self.color[1] = if enabled { 1. } else { 0. };
        self
    }
    pub fn solid(source: usize, rgb: [f64; 3], radius: f32) -> Self {
        Self {
            color: [rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.],
            source_space: source as f32,
            kind: 1.,
            channel: 0.,
            vertical: 0.,
            radius,
            strip_width: 0.,
            reversed: 0.,
            sdr_fallback: 0.,
        }
    }
    pub fn track(source: usize, rgb: [f64; 3], channel: usize, context: &ColorSliderTrackContext, radius: f32) -> Self {
        Self {
            kind: 2.,
            channel: channel as f32,
            vertical: if context.axis == Axis::Vertical { 1. } else { 0. },
            reversed: if context.reversed { 1. } else { 0. },
            ..Self::solid(source, rgb, radius)
        }
    }
}

pub(super) fn program(source: usize) -> anyhow::Result<ShaderProgram<Parameters>> {
    let code = format!("{}\n{}", xgpui_color::WGSL_COLOR_MATH, include_str!("hv_field.wgsl"));
    match source {
        0 => ShaderProgram::new(&code),
        1 => ShaderProgram::new_linear_p3(&code),
        2 => ShaderProgram::new_linear_rec2020(&code),
        3 => ShaderProgram::new_linear_rec2020(&code),
        _ => anyhow::bail!("unsupported source space"),
    }
}

#[derive(Clone)]
pub(super) struct NativeColors(Arc<[ShaderProgram<Parameters>; 4]>);
impl NativeColors {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self(Arc::new([program(0)?, program(1)?, program(2)?, program(3)?])))
    }
    pub fn paint(&self, bounds: Bounds<Pixels>, parameters: Parameters, window: &mut Window) -> anyhow::Result<()> {
        let mut parameters = parameters;
        let source = parameters.source_space as usize;
        let program = if source > 0 && window.color_status().effective() != gpui::WindowColorMode::WideGamutSdr {
            parameters.sdr_fallback = 1.;
            &self.0[0]
        } else {
            &self.0[source]
        };
        window.paint_custom_shader(bounds, program, parameters)
    }
    pub fn field_painter(&self, settings: FieldSettings) -> ColorFieldBackgroundPainter {
        let colors = self.clone();
        Arc::new(move |bounds, _, window, _| {
            colors.paint(
                bounds,
                Parameters::field(settings.source_index(), settings.saturation()).with_contour(settings.contour()),
                window,
            )
        })
    }
    pub fn thumb_painter(&self, settings: FieldSettings) -> ColorFieldBackgroundPainter {
        let colors = self.clone();
        Arc::new(move |bounds, hsv, window, _| {
            let rgb: gpui::Rgba = hsv.to_hsla_ext().into();
            colors.paint(
                bounds,
                Parameters::solid(
                    settings.source_index(),
                    [rgb.r as f64, rgb.g as f64, rgb.b as f64],
                    bounds.size.width.as_f32() / 2.,
                ),
                window,
            )
        })
    }
    pub fn track(&self, source: usize, rgb: [f64; 3], channel: usize) -> Arc<dyn ColorSliderDelegate> {
        Arc::new(NativeTrack { colors: self.clone(), source, rgb, channel: TrackChannel::Rgb(channel) })
    }
    pub fn saturation_track(&self, source: usize, mut hsv: Hsv) -> Arc<dyn ColorSliderDelegate> {
        hsv.s = 1.;
        let rgb: gpui::Rgba = hsv.to_hsla_ext().into();
        Arc::new(NativeTrack {
            colors: self.clone(),
            source,
            rgb: [rgb.r as f64, rgb.g as f64, rgb.b as f64],
            channel: TrackChannel::Saturation,
        })
    }
}
#[derive(Clone, Copy)]
enum TrackChannel {
    Rgb(usize),
    Saturation,
}
struct NativeTrack {
    colors: NativeColors,
    source: usize,
    rgb: [f64; 3],
    channel: TrackChannel,
}
impl NativeTrack {
    fn sample(&self, context: &ColorSliderTrackContext, position: f32) -> [f64; 3] {
        let position = if context.reversed {
            1. - position as f64
        } else {
            position as f64
        };
        let mut rgb = self.rgb;
        match self.channel {
            TrackChannel::Rgb(channel) => rgb[channel] = position,
            TrackChannel::Saturation => {
                let value = rgb.into_iter().fold(0., f64::max);
                rgb = rgb.map(|channel| value + (channel - value) * position);
            }
        }
        rgb
    }
}
impl ColorSliderDelegate for NativeTrack {
    fn paint_domain_track(&self, context: &ColorSliderTrackContext, bounds: Bounds<Pixels>, window: &mut Window) {
        let radius = context
            .corner_radii
            .top_left
            .map(|r| r.to_pixels(window.rem_size()).as_f32())
            .unwrap_or(bounds.size.height.as_f32() / 2.);
        if let Err(error) = self.colors.paint(
            bounds,
            Parameters::track(
                self.source,
                self.rgb,
                match self.channel {
                    TrackChannel::Rgb(index) => index,
                    TrackChannel::Saturation => 3,
                },
                context,
                radius,
            ),
            window,
        ) {
            tracing::error!("Native color slider: {error:#}");
        }
    }
    fn get_color_for_context(&self, context: &ColorSliderTrackContext, position: f32) -> gpui::Hsla {
        let rgb = self.sample(context, position);
        source_field::srgb_reference(rgb, source_field::SPACES[self.source])
            .map(Into::into)
            .unwrap_or_else(|_| gpui::black())
    }
    fn thumb_painter(
        &self,
        context: &ColorSliderTrackContext,
        position: f32,
    ) -> Option<gpui_luma_color::color_slider::color_thumb::ColorThumbPainter> {
        let colors = self.colors.clone();
        let source = self.source;
        let rgb = self.sample(context, position);
        Some(Arc::new(move |bounds, window, _| {
            colors.paint(bounds, Parameters::solid(source, rgb, bounds.size.width.as_f32() / 2.), window)
        }))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn portable_native_programs_validate() {
        super::NativeColors::new().expect("sRGB, P3, Rec2020 and Adobe RGB native programs");
    }
}

#[cfg(all(test, feature = "test-support", target_os = "macos"))]
mod gpu_tests {
    use super::{program, Parameters};
    use crate::color_demos::{native_cpu_field, source_field::SPACES, italy_model::half};
    use gpui::{Bounds, ContentMask, DevicePixels, ScaledPixels, Scene, ShaderDraw, point, size};
    use xgpui_color::{EncodedRgb, LinearRgb, RgbSpace};

    fn scene(source: usize, parameters: Parameters, width: u32, height: u32, scale: f32) -> anyhow::Result<Scene> {
        let program = program(source)?;
        let bounds = Bounds::new(
            point(ScaledPixels(0.), ScaledPixels(0.)),
            size(ScaledPixels(width as f32), ScaledPixels(height as f32)),
        );
        let mut data = [[0.; 4]; gpui::SHADER_DATA_SLOTS];
        for (i, value) in bytemuck::cast_slice::<_, f32>(&[parameters]).iter().enumerate() {
            data[i / 4][i % 4] = *value;
        }
        let mut scene = Scene::default();
        scene.insert_primitive(gpui::Primitive::Shader(ShaderDraw {
            order: 0,
            bounds,
            content_mask: ContentMask { bounds },
            opacity: 1.,
            scale_factor: scale,
            program: program.scene_program(),
            data,
            path_mask: None,
            glyph_mask: None,
        }));
        scene.finish();
        Ok(scene)
    }
    fn assert_pixel(actual: [u16; 4], expected: [f64; 3], tolerance: f64) -> anyhow::Result<()> {
        for (actual, expected) in actual[..3].iter().copied().map(half).zip(expected) {
            anyhow::ensure!((actual - expected).abs() < tolerance, "native P3 pixel {actual} != {expected}");
        }
        Ok(())
    }

    #[test]
    #[ignore = "requires a native Metal device"]
    fn native_field_swatches_and_tracks_preserve_source_primaries_at_1x_and_2x() -> anyhow::Result<()> {
        let mut renderer = gpui_apple::metal_renderer::MetalHeadlessRenderer::new();
        let status = renderer.set_color_mode(gpui::WindowColorMode::WideGamutSdr)?;
        anyhow::ensure!(status.effective() == gpui::WindowColorMode::WideGamutSdr, "wide presentation required");
        for scale in [1., 2.] {
            let width = (300. * scale) as u32;
            let height = (100. * scale) as u32;
            for (source, space) in SPACES.into_iter().enumerate() {
                for saturation in [0., 0.62, 1.] {
                    let cpu = native_cpu_field::pixels(source, width, height, scale, saturation)?;
                    let mut parameters = Parameters::field(source, saturation);
                    parameters.color[1] = 0.; // Compare the underlying field independently of its contour.
                    let image = renderer.render_scene_to_linear_p3(
                        &scene(source, parameters, width, height, scale)?,
                        size(DevicePixels(width as i32), DevicePixels(height as i32)),
                    )?;
                    for y in (2..height - 2).step_by(13) {
                        for x in (2..width - 2).step_by(17) {
                            let index = (y * width + x) as usize;
                            let linear = cpu[index];
                            let expected =
                                LinearRgb::new(space, [linear[0] as f64, linear[1] as f64, linear[2] as f64])?
                                    .convert(RgbSpace::DisplayP3)?
                                    .components();
                            assert_pixel(image.pixels[index], expected, if source == 0 { 0.006 } else { 0.003 })
                                .map_err(|error| {
                                    anyhow::anyhow!("field {space:?}, scale={scale}, ({x},{y}): {error}")
                                })?;
                        }
                    }
                }
                // Wide source green must remain native, rather than pass through a clipped sRGB intermediate.
                let rgb = [0., 1., 0.];
                let expected = EncodedRgb::new(space, rgb)?.decode()?.convert(RgbSpace::DisplayP3)?.components();
                let solid = renderer.render_scene_to_linear_p3(
                    &scene(source, Parameters::solid(source, rgb, 0.), width, height, scale)?,
                    size(DevicePixels(width as i32), DevicePixels(height as i32)),
                )?;
                assert_pixel(
                    solid.pixels[(height / 2 * width + width / 2) as usize],
                    expected,
                    if source == 0 { 0.006 } else { 0.003 },
                )
                .map_err(|error| anyhow::anyhow!("solid {space:?}: {error}"))?;
                if source == 1 {
                    anyhow::ensure!(
                        half(solid.pixels[(height / 2 * width + width / 2) as usize][0]).abs() < 0.001,
                        "P3 green must not be clipped through sRGB"
                    );
                }
                if source == 2 {
                    anyhow::ensure!(expected.iter().any(|v| *v < 0. || *v > 1.), "Rec2020 fixture must exceed P3");
                }
                for channel in 0..4 {
                    let mut parameters = Parameters::solid(source, [0.86, 0.32, 0.58], 0.);
                    parameters.kind = 2.;
                    parameters.channel = channel as f32;
                    let track = renderer.render_scene_to_linear_p3(
                        &scene(source, parameters, width, height, scale)?,
                        size(DevicePixels(width as i32), DevicePixels(height as i32)),
                    )?;
                    for x in (2..width - 2).step_by(19) {
                        let mut rgb = [0.86, 0.32, 0.58];
                        let position = (x as f64 + 0.5) / width as f64;
                        if channel == 3 {
                            rgb = rgb.map(|channel| 0.86 + (channel - 0.86) * position);
                        } else {
                            rgb[channel] = position;
                        }
                        let expected =
                            EncodedRgb::new(space, rgb)?.decode()?.convert(RgbSpace::DisplayP3)?.components();
                        assert_pixel(
                            track.pixels[(height / 2 * width + x) as usize],
                            expected,
                            if source == 0 { 0.006 } else { 0.003 },
                        )
                        .map_err(|error| anyhow::anyhow!("track {space:?}, scale={scale}, x={x}: {error}"))?;
                    }
                }
            }
        }
        renderer.set_color_mode(gpui::WindowColorMode::Default)?;
        Ok(())
    }

    #[test]
    #[ignore = "requires a native Metal device"]
    fn srgb_contour_and_stripes_are_confined_to_wide_field_gamut_regions() -> anyhow::Result<()> {
        let mut renderer = gpui_apple::metal_renderer::MetalHeadlessRenderer::new();
        renderer.set_color_mode(gpui::WindowColorMode::WideGamutSdr)?;
        for scale in [1., 2.] {
            let width = (360. * scale) as u32;
            let height = (160. * scale) as u32;
            for (source, space) in SPACES.into_iter().enumerate() {
                for saturation in [0., 0.1, 0.62, 1.] {
                    let parameters = Parameters::field(source, saturation);
                    let mut plain = parameters;
                    plain.color[1] = 0.;
                    let render = |renderer: &mut gpui_apple::metal_renderer::MetalHeadlessRenderer, parameters| {
                        renderer.render_scene_to_linear_p3(
                            &scene(source, parameters, width, height, scale)?,
                            size(DevicePixels(width as i32), DevicePixels(height as i32)),
                        )
                    };
                    let base = render(&mut renderer, plain)?;
                    let outlined = render(&mut renderer, parameters)?;
                    let cpu = native_cpu_field::pixels(source, width, height, scale, saturation)?;
                    let margin = |x: u32, y: u32| -> anyhow::Result<f64> {
                        let pixel = cpu[(y * width + x) as usize];
                        let rgb = LinearRgb::new(space, [pixel[0] as f64, pixel[1] as f64, pixel[2] as f64])?
                            .convert(RgbSpace::Srgb)?
                            .components();
                        Ok(rgb
                            .into_iter()
                            .fold(f64::INFINITY, f64::min)
                            .min(1. - rgb.into_iter().fold(f64::NEG_INFINITY, f64::max)))
                    };
                    let mut changed = 0;
                    for y in 2..height - 2 {
                        for x in 2..width - 2 {
                            let index = (y * width + x) as usize;
                            if base.pixels[index][..3]
                                .iter()
                                .zip(&outlined.pixels[index][..3])
                                .any(|(&a, &b)| (half(a) - half(b)).abs() > 0.015)
                            {
                                changed += 1;
                                let gradient = (margin(x + 1, y)? - margin(x - 1, y)?).abs()
                                    + (margin(x, y + 1)? - margin(x, y - 1)?).abs();
                                anyhow::ensure!(
                                    margin(x, y)? < -1e-6 || margin(x, y)?.abs() < gradient * 2. + 1e-6,
                                    "only out-of-gamut pixels and the sRGB boundary may change"
                                );
                                anyhow::ensure!(x < width - (6. * scale) as u32, "endpoint strip unchanged");
                            }
                        }
                    }
                    if source == 0 || saturation == 0. {
                        anyhow::ensure!(changed == 0, "no contour for an entirely sRGB field: source={source}, saturation={saturation}, scale={scale}, changed={changed}");
                    } else {
                        anyhow::ensure!(changed > 0 && changed < width * height / 2,
                            "wide field must retain visible colors between stripes, source={source}, saturation={saturation}, changed={changed}");
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    #[ignore = "requires a native Metal device"]
    fn native_cpu_images_retain_p3_and_rec2020_pixels() -> anyhow::Result<()> {
        use std::{
            sync::Arc,
            time::{Duration, Instant},
        };
        use xgpui_media::{
            MediaLoader, MediaLimits, UploadStatus,
            pipeline::{PixelColor, AlphaMode},
        };
        let mut renderer = gpui_apple::metal_renderer::MetalHeadlessRenderer::new();
        renderer.set_color_mode(gpui::WindowColorMode::WideGamutSdr)?;
        let loader = MediaLoader::new(MediaLimits::default())?;
        for source in [1, 2, 3] {
            let width = 64;
            let height = 32;
            let pixels = native_cpu_field::pixels(source, width, height, 1., 1.)?;
            let primaries = super::super::native_cpu_field::primaries(source);
            let storage = super::super::native_cpu_field::storage_primaries(source);
            let mut job = loader.load_pixel_buffer(
                width,
                height,
                pixels.clone(),
                PixelColor::ExtendedLinear { primaries, alpha: AlphaMode::Straight },
                storage,
                xgpui_color::GamutMapping::Clip,
            )?;
            let start = Instant::now();
            let image = loop {
                if let Some(image) = job.try_take() {
                    break image;
                }
                anyhow::ensure!(start.elapsed() < Duration::from_secs(10), "CPU raster preparation timeout");
                std::thread::sleep(Duration::from_millis(1));
            };
            let bounds = Bounds::new(
                point(ScaledPixels(0.), ScaledPixels(0.)),
                size(ScaledPixels(width as f32), ScaledPixels(height as f32)),
            );
            let mut scene = Scene::default();
            scene.insert_primitive(gpui::CustomDraw {
                order: 0,
                bounds,
                content_mask: ContentMask { bounds },
                corner_radii: Default::default(),
                opacity: 1.,
                content: Arc::new(image.wide(None)?),
                coverage: None,
            });
            scene.finish();
            let snapshot = loop {
                let snapshot = renderer
                    .render_scene_to_linear_p3(&scene, size(DevicePixels(width as i32), DevicePixels(height as i32)))?;
                if image.upload_status() == UploadStatus::Ready {
                    break snapshot;
                }
                anyhow::ensure!(start.elapsed() < Duration::from_secs(10), "CPU native upload timeout");
                std::thread::sleep(Duration::from_millis(2));
            };
            for index in (0..pixels.len()).step_by(29) {
                let rgb = pixels[index];
                let expected = LinearRgb::new(SPACES[source], [rgb[0] as f64, rgb[1] as f64, rgb[2] as f64])?
                    .convert(RgbSpace::DisplayP3)?
                    .components();
                assert_pixel(snapshot.pixels[index], expected, 0.003)?;
            }
        }
        Ok(())
    }
}
