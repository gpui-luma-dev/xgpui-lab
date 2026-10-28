//! Embedded atlas/mipmap capture adapted from media-lab/headless.rs.
use super::super::fixtures::Fixtures;
use anyhow::{Result, bail, ensure};
use gpui::*;
use std::{
    borrow::Cow,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use xgpui_media::{LoadStatus, MediaImage, MediaLoader, UploadStatus};
fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds::new(point(ScaledPixels(x), ScaledPixels(y)), size(ScaledPixels(w), ScaledPixels(h)))
}
fn quad(scene: &mut Scene, bounds: Bounds<ScaledPixels>, color: u32) {
    scene.insert_primitive(Quad {
        bounds,
        content_mask: ContentMask { bounds },
        background: rgb(color).into(),
        ..Default::default()
    });
}
pub fn run(fixtures: &Fixtures, png: &Path, cancel: &std::sync::atomic::AtomicBool) -> Result<()> {
    let vivid = false;
    let adjustments = false;
    let mut renderer =
        gpui_platform::current_headless_renderer()?.ok_or_else(|| anyhow::anyhow!("no native headless renderer"))?;
    let loader = MediaLoader::default();
    let mut images: Vec<MediaImage> = Vec::new();
    let mut baseline = Vec::new();
    for sample in &fixtures.samples {
        let mut job = loader.load(&sample.path)?;
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            super::check_cancel(cancel)?;
            if let Some(image) = job.try_take() {
                images.push(image);
                break;
            }
            if let LoadStatus::Failed(error) = job.status() {
                bail!(error);
            }
            ensure!(Instant::now() < deadline, "decode timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut bgra = image::ImageReader::open(&sample.path)?.with_guessed_format()?.decode()?.into_rgba8();
        for pixel in bgra.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        baseline.push(Arc::new(RenderImage::new(vec![image::Frame::new(bgra)])));
    }
    let mut scene = Scene::default();
    let height = fixtures.samples.len() as i32 * 246 + 24;
    quad(&mut scene, rect(0., 0., 1000., height as f32), 0x111619);
    for (index, sample) in fixtures.samples.iter().enumerate() {
        for column in 0..2 {
            let panel = rect(16. + column as f32 * 492., 16. + index as f32 * 246., 476., 226.);
            for y in 0..10 {
                for x in 0..20 {
                    let tile = rect(panel.origin.x.0 + x as f32 * 24., panel.origin.y.0 + y as f32 * 24., 24., 24.);
                    let bounds = tile.intersect(&panel);
                    if !bounds.is_empty() {
                        quad(&mut scene, bounds, if (x + y) % 2 == 0 { 0x20282b } else { 0x252e31 });
                    }
                }
            }
            let fit = (panel.size.width.0 / sample.width as f32).min(panel.size.height.0 / sample.height as f32) * 0.88;
            let w = sample.width as f32 * fit;
            let h = sample.height as f32 * fit;
            let bounds = rect(
                panel.origin.x.0 + (panel.size.width.0 - w) * 0.5,
                panel.origin.y.0 + (panel.size.height.0 - h) * 0.5,
                w,
                h,
            );
            if column == 0 && !adjustments {
                let image = &baseline[index];
                let params = RenderImageParams { image_id: image.id, frame_index: 0 };
                let tile = renderer
                    .sprite_atlas()
                    .get_or_insert_with(params.into(), &mut || {
                        Ok(Some((image.size(0), Cow::Borrowed(image.as_bytes(0).unwrap()))))
                    })?
                    .unwrap();
                scene.insert_primitive(PolychromeSprite {
                    order: 0,
                    pad: 0,
                    grayscale: false.into(),
                    opacity: 1.,
                    bounds,
                    content_mask: ContentMask { bounds: panel },
                    corner_radii: Corners::all(ScaledPixels(14.)),
                    tile,
                });
            } else {
                scene.insert_primitive(CustomDraw {
                    order: 0,
                    bounds,
                    content_mask: ContentMask { bounds: panel },
                    corner_radii: Corners::all(ScaledPixels(14.)),
                    opacity: 1.,
                    coverage: None,
                    content: if adjustments {
                        use xgpui_media::pipeline::{ColorMatrix, ImageAdjustments, ImagePrimaries, Vignette};
                        let matrix =
                            ColorMatrix::identity(ImagePrimaries::Srgb).exposure(if column == 0 { 0. } else { 1. })?;
                        let vignette = if column == 0 {
                            None
                        } else {
                            Some(Vignette::new(0.6, 0.35, 0.65, 1.)?)
                        };
                        Arc::new(images[index].adjusted(ImageAdjustments::new(matrix, vignette)?))
                    } else {
                        Arc::new(images[index].filtered(if vivid {
                            xgpui_media::ImageFilter::new().brightness(1.4).contrast(1.5).saturation(1.8)
                        } else {
                            xgpui_media::ImageFilter::new()
                        }))
                    },
                });
            }
        }
    }
    scene.finish();
    let extent = size(DevicePixels(1000), DevicePixels(height));
    let deadline = Instant::now() + Duration::from_secs(30);
    let pixels = loop {
        super::check_cancel(cancel)?;
        let pixels = renderer.render_scene_to_image(&scene, extent)?;
        for image in &images {
            if let UploadStatus::Failed(error) = image.upload_status() {
                bail!(error);
            }
        }
        if images.iter().all(|i| i.upload_status() == UploadStatus::Ready) {
            break pixels;
        }
        ensure!(Instant::now() < deadline, "upload timed out");
        std::thread::sleep(Duration::from_millis(2));
    };
    ensure!(images.iter().all(|i| i.upload_count() == 1), "capture unexpectedly reuploaded a source");
    pixels.save(png)?;

    Ok(())
}
