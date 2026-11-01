//! CPU field rasterization with explicitly tagged linear floating-point storage.
use std::sync::{Arc, Mutex};
use gpui_luma_color::{color_field::ColorFieldBackgroundPainter, color_slider::color_spec::Hsv};
use xgpui_media::{MediaLoader, MediaLimits, ImageJob, MediaImage, LoadStatus};
use xgpui_media::pipeline::{PixelColor, ImagePrimaries, AlphaMode};
use xgpui_color::EncodedRgb;
use super::{
    source_field::{FieldSettings, SPACES, ENDPOINT_STRIP_WIDTH},
    hv_field_shader::NativeColors,
};

struct Cache {
    loader: MediaLoader,
    key: Option<(usize, u32, u32, u32)>,
    job: Option<ImageJob>,
    image: Option<MediaImage>,
}

pub(super) fn primaries(source: usize) -> ImagePrimaries {
    match SPACES[source] {
        xgpui_color::RgbSpace::Srgb => ImagePrimaries::Srgb,
        xgpui_color::RgbSpace::DisplayP3 => ImagePrimaries::DisplayP3,
        xgpui_color::RgbSpace::Rec2020 => ImagePrimaries::Rec2020,
        xgpui_color::RgbSpace::AdobeRgb1998 => ImagePrimaries::AdobeRgb1998,
    }
}

pub(super) fn storage_primaries(source: usize) -> ImagePrimaries {
    if SPACES[source] == xgpui_color::RgbSpace::AdobeRgb1998 {
        ImagePrimaries::Rec2020
    } else {
        primaries(source)
    }
}

pub(super) fn pixels(
    source: usize,
    width: u32,
    height: u32,
    scale: f32,
    saturation: f32,
) -> anyhow::Result<Vec<[f32; 4]>> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height {
        for x in 0..width {
            let uv = ((x as f32 + 0.5) / width as f32, (y as f32 + 0.5) / height as f32);
            let rgb: gpui::Rgba = if x as f32 + 0.5 >= width as f32 - ENDPOINT_STRIP_WIDTH * scale {
                if uv.1 < 0.5 {
                    gpui::white().into()
                } else {
                    gpui::black().into()
                }
            } else {
                Hsv { h: uv.0 * 360., s: saturation, v: 1. - uv.1, a: 1. }.to_hsla_ext().into()
            };
            let linear =
                EncodedRgb::new(SPACES[source], [rgb.r as f64, rgb.g as f64, rgb.b as f64])?.decode()?.components();
            pixels.push([linear[0] as f32, linear[1] as f32, linear[2] as f32, 1.]);
        }
    }
    Ok(pixels)
}

pub(super) fn painter(settings: FieldSettings, colors: NativeColors) -> anyhow::Result<ColorFieldBackgroundPainter> {
    let cache = Arc::new(Mutex::new(Cache {
        loader: MediaLoader::new(MediaLimits {
            max_jobs: 1,
            max_resident_bytes: 96 * 1024 * 1024,
            max_image_bytes: 32 * 1024 * 1024,
            max_encoded_bytes: 64 * 1024 * 1024,
        })?,
        key: None,
        job: None,
        image: None,
    }));
    Ok(Arc::new(move |bounds, _, window, _| {
        let source = settings.source_index();
        let result = (|| -> anyhow::Result<()> {
            let scale = window.scale_factor();
            let width = (bounds.size.width.as_f32() * scale).ceil().max(1.) as u32;
            let height = (bounds.size.height.as_f32() * scale).ceil().max(1.) as u32;
            let source = settings.source_index();
            let key = (source, width, height, settings.saturation().to_bits());
            let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
            if cache.key != Some(key) {
                cache.job = None;
                cache.image = None;
                let primaries = primaries(source);
                let pixels = pixels(source, width, height, scale, settings.saturation())?;
                cache.job = Some(cache.loader.load_pixel_buffer(
                    width,
                    height,
                    pixels,
                    PixelColor::ExtendedLinear { primaries, alpha: AlphaMode::Straight },
                    storage_primaries(source),
                    xgpui_color::GamutMapping::Clip,
                )?);
                cache.key = Some(key);
            }
            if let Some(job) = &mut cache.job {
                if let Some(image) = job.try_take() {
                    cache.image = Some(image);
                    cache.job = None;
                } else if let LoadStatus::Failed(error) = job.status() {
                    anyhow::bail!("Native CPU field: {error}");
                }
            }
            if let Some(image) = &cache.image {
                if image.upload_status() != xgpui_media::UploadStatus::Ready {
                    colors.paint(
                        bounds,
                        super::hv_field_shader::Parameters::field(source, settings.saturation())
                            .with_contour(settings.contour()),
                        window,
                    )?;
                }
                if !matches!(
                    image.upload_status(),
                    xgpui_media::UploadStatus::Ready | xgpui_media::UploadStatus::Failed(_)
                ) {
                    window.request_animation_frame();
                }
                window.paint_custom(
                    gpui::CustomGeometry::RoundedRect { bounds, corner_radii: Default::default() },
                    Arc::new(image.wide(None)?),
                );
            } else {
                colors.paint(
                    bounds,
                    super::hv_field_shader::Parameters::field(source, settings.saturation())
                        .with_contour(settings.contour()),
                    window,
                )?;
                window.request_animation_frame();
            }
            Ok(())
        })();
        if let Err(error) = result {
            tracing::error!("Native CPU field unavailable: {error:#}");
            return colors.paint(
                bounds,
                super::hv_field_shader::Parameters::field(source, settings.saturation())
                    .with_contour(settings.contour()),
                window,
            );
        }
        Ok(())
    }))
}
