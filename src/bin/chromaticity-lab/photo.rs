//! WebKit Italy reference images, prepared offline as pixels + linear-P3 metadata.
use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
const SOURCE: &str = "https://webkit.org/blog-files/color-gamut/";
const BUFFERS: [&[u8]; 3] = [
    include_bytes!("../../../assets/colorimetry/photos/Italy-sRGB.rgba16f"),
    include_bytes!("../../../assets/colorimetry/photos/Italy-P3.rgba16f"),
    include_bytes!("../../../assets/colorimetry/photos/Italy-oog.rgba16f"),
];
fn pixels(bytes: &[u8]) -> Vec<[f32; 4]> {
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|pixel| std::array::from_fn(|i| half(u16::from_le_bytes([pixel[i * 2], pixel[i * 2 + 1]])) as f32))
        .collect()
}
struct Prepared {
    jobs: Vec<ImageJob>,
}
fn prepare(cancelled: &AtomicBool) -> anyhow::Result<Prepared> {
    // This fixed fixture set submits all three versions together. Bound its
    // loader explicitly instead of exceeding the default two-job admission.
    let loader = MediaLoader::new(xgpui_media::MediaLimits { max_jobs: BUFFERS.len(), ..Default::default() })?;
    let mut jobs = Vec::new();
    for bytes in BUFFERS {
        anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "Photo preparation cancelled");
        jobs.push(loader.load_pixel_buffer(
            600,
            400,
            pixels(bytes),
            xgpui_media::pipeline::PixelColor::ExtendedLinear {
                primaries: ImagePrimaries::DisplayP3,
                alpha: AlphaMode::Straight,
            },
            ImagePrimaries::DisplayP3,
            GamutMapping::Clip,
        )?);
    }
    Ok(Prepared { jobs })
}
pub(super) struct Photo {
    receiver: Option<mpsc::Receiver<anyhow::Result<Prepared>>>,
    cancelled: Arc<AtomicBool>,
    jobs: Vec<ImageJob>,
    images: Vec<Option<MediaImage>>,
    status: String,
    error: Option<String>,
    bounds: Arc<std::sync::Mutex<[Option<Bounds<ScaledPixels>>; 3]>>,
    report: String,
}
impl Drop for Photo {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
impl Photo {
    pub(super) fn new() -> Self {
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = sender.send(prepare(&flag));
        });
        Self {
            receiver: Some(receiver),
            cancelled,
            jobs: vec![],
            images: vec![None, None, None],
            status: "Preparing WebKit’s three Italy reference images…".into(),
            error: None,
            bounds: Arc::new(std::sync::Mutex::new([None; 3])),
            report: "Displayed comparison: not checked.".into(),
        }
    }
    fn poll(&mut self, window: &mut Window) {
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(Ok(prepared)) => {
                    self.status = "WebKit’s original sRGB, Display P3 and highlighted JPEGs • 600 × 400 • prepared as linear Display P3 pixels using ColorSync.".into();
                    self.jobs = prepared.jobs;
                    self.receiver = None;
                }
                Ok(Err(error)) => {
                    self.status = format!("Photo preparation failed: {error:#}");
                    self.error = Some(self.status.clone());
                    self.receiver = None;
                }
                Err(mpsc::TryRecvError::Empty) => window.request_animation_frame(),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = "Photo preparation failed: worker stopped".into();
                    self.error = Some(self.status.clone());
                    self.receiver = None;
                }
            }
        }
        for (job, image) in self.jobs.iter_mut().zip(&mut self.images) {
            if let Some(ready) = job.try_take() {
                *image = Some(ready);
            }
            match job.status() {
                xgpui_media::LoadStatus::Loading => window.request_animation_frame(),
                xgpui_media::LoadStatus::Failed(error) => self.error = Some(error),
                _ => {}
            }
        }
        for image in self.images.iter().flatten() {
            if let xgpui_media::UploadStatus::Failed(error) = image.upload_status() {
                self.error = Some(error);
            }
        }
    }
    pub(super) fn check(&mut self, window: &mut Window) {
        let result = (|| -> anyhow::Result<String> {
            anyhow::ensure!(
                window.color_status().effective() == WindowColorMode::WideGamutSdr,
                "Select Wide gamut for Metal FP16 readback"
            );
            anyhow::ensure!(
                self.images
                    .first()
                    .and_then(Option::as_ref)
                    .is_some_and(|image| image.upload_status() == xgpui_media::UploadStatus::Ready),
                "Wait for photo upload"
            );
            let bounds = *self.bounds.lock().unwrap();
            let frame = window.render_to_linear_p3()?;
            let read = |index: usize| -> anyhow::Result<[f64; 4]> {
                let b = bounds[index].ok_or_else(|| anyhow::anyhow!("Comparison panel has not painted"))?;
                let x = (b.origin.x.0 + b.size.width.0 * 0.86).floor() as usize;
                let y = (b.origin.y.0 + b.size.height.0 * 0.55).floor() as usize;
                anyhow::ensure!(
                    x < frame.size.width.0 as usize && y < frame.size.height.0 as usize,
                    "Show both comparison panels before checking"
                );
                Ok(frame.pixels[y * frame.size.width.0 as usize + x].map(half))
            };
            let preview = read(0)?;
            let native = read(1)?;
            let delta = (0..3).map(|i| (preview[i] - native[i]).abs()).fold(0., f64::max);
            Ok(format!(
                "Displayed Metal FP16 at the orange umbrella: sRGB {:.5?}; native {:.5?}; max channel difference {delta:.5}. These are composition values before OS display mapping, not monitor measurements.",
                preview, native
            ))
        })();
        self.report = result.unwrap_or_else(|error| format!("Displayed comparison: {error:#}"));
    }
    fn picture(&self, selection: usize, wide: bool, primary: bool) -> impl IntoElement {
        let image = self.images[selection].clone();
        let recorded = self.bounds.clone();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                if primary {
                    recorded.lock().unwrap()[selection] = Some(bounds.scale(window.scale_factor()));
                }
                if let Some(image) = &image {
                    wake_for_upload(image, bounds, window);
                    let geometry = CustomGeometry::RoundedRect { bounds, corner_radii: Default::default() };
                    if selection == 1 && wide {
                        if let Ok(image) = image.wide(None) {
                            window.paint_custom(geometry, Arc::new(image));
                        }
                    } else {
                        window.paint_custom(geometry, Arc::new(image.clone()));
                    }
                }
            },
        )
        .size_full()
    }
}
impl Lab {
    pub(super) fn render_photo(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(photo) = &mut self.photo {
            photo.poll(window);
        }
        let labels = ["sRGB", "Display P3", "Out of sRGB areas"];
        let mut body = div().id("photo-gamut-scroll").size_full().overflow_y_scroll().p_6().bg(rgb(0x202833)).text_color(rgb(0xe2eaf3)).flex().flex_col().gap_4()
            .when(self.standalone_navigation, |body| body.child(self.button("← Chromaticity & metadata".into(), 27, cx)))
            .child(div().text_2xl().child("Italy — WebKit photographic gamut comparison"))
            .child(self.window_mode_controls(window,cx))
            .child("Compare the orange/red umbrellas in sRGB and Display P3. Select Wide gamut to preserve the P3 version’s wider colors; Standard limits output to sRGB. The third image is WebKit’s supplied highlighted version, with the photograph visible around the marked areas. The highlight colors are diagnostic markings baked into WebKit’s third image, so they remain visible in both modes. These are the same three source images used by their web demo.")
            .child(format!("Currently showing: {} • effective {:?} • display wide gamut {:?}",labels[self.photo_selection],window.color_status().effective(),window.color_status().display_wide_gamut()));
        if let Some(photo) = &self.photo {
            body = body
                .child(photo.status.clone())
                .child(div().flex().flex_wrap().gap_4().children((0..3).map(|selection| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .id(ElementId::Name(format!("photo-thumbnail-{selection}").into()))
                                .w(px(180.))
                                .h(px(120.))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.photo_selection = selection;
                                    cx.notify();
                                }))
                                .child(photo.picture(selection, self.wide, false)),
                        )
                        .child(self.button(labels[selection].into(), 24 + selection, cx))
                })))
                .child(div().w_full().max_w(px(900.)).aspect_ratio(1.5).flex_shrink_0().child(photo.picture(
                    self.photo_selection,
                    self.wide,
                    false,
                )))
                .child(photo.error.clone().unwrap_or_default());
        }
        body.child(div().id("webkit-italy-credit").cursor_pointer().text_color(rgb(0x82caff))
            .child("Italy images courtesy of WebKit’s Wide Gamut demo · original examples and source ↗")
            .on_click(|_, _, cx| cx.open_url(SOURCE)))
            .child("Original WebKit JPEGs retained; embedded profiles interpreted offline with macOS ColorSync. The lab renders prepared pixels with explicit linear Display P3 metadata. Visible differences depend on window mode and your display; this is SDR, not a monitor measurement.")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::prelude::v1::test;
    #[cfg(target_os = "macos")]
    #[test]
    fn photo_native_and_preview_produce_different_metal_composition_pixels() -> anyhow::Result<()> {
        use std::time::{Duration, Instant};
        let source_pixels: Vec<_> = BUFFERS[..2].iter().map(|bytes| pixels(bytes)[220 * 600 + 515]).collect();
        let mut jobs: Vec<_> = source_pixels
            .iter()
            .map(|pixel| {
                MediaLoader::default().load_pixel_buffer(
                    1,
                    1,
                    vec![*pixel],
                    ImageColor {
                        primaries: ImagePrimaries::DisplayP3,
                        transfer: TransferFunction::Linear,
                        alpha: AlphaMode::Straight,
                    }
                    .into(),
                    ImagePrimaries::DisplayP3,
                    GamutMapping::Clip,
                )
            })
            .collect::<Result<_, _>>()?;
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut images = Vec::new();
        for job in &mut jobs {
            loop {
                if let Some(image) = job.try_take() {
                    images.push(image);
                    break;
                }
                anyhow::ensure!(Instant::now() < deadline, "photo preparation timed out");
                if let xgpui_media::LoadStatus::Failed(error) = job.status() {
                    anyhow::bail!(error);
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        let mut scene = gpui::Scene::default();
        for (index, (x, wide)) in [(0., false), (32., true)].into_iter().enumerate() {
            let image = &images[index];
            let bounds =
                Bounds::new(point(ScaledPixels(x), ScaledPixels(0.)), size(ScaledPixels(32.), ScaledPixels(32.)));
            scene.insert_primitive(gpui::CustomDraw {
                order: gpui::DrawOrder::default(),
                bounds,
                content_mask: gpui::ContentMask { bounds },
                corner_radii: Default::default(),
                opacity: 1.,
                coverage: None,
                content: if wide {
                    Arc::new(image.wide(None)?)
                } else {
                    Arc::new(image.clone())
                },
            });
        }
        scene.finish();
        let mut renderer = gpui_apple::metal_renderer::MetalHeadlessRenderer::new();
        renderer.set_color_mode(WindowColorMode::WideGamutSdr)?;
        let frame = loop {
            let frame =
                renderer.render_scene_to_linear_p3(&scene, size(gpui::DevicePixels(64), gpui::DevicePixels(32)))?;
            if images.iter().all(|image| image.upload_status() == xgpui_media::UploadStatus::Ready) {
                break frame;
            }
            for image in &images {
                if let xgpui_media::UploadStatus::Failed(error) = image.upload_status() {
                    anyhow::bail!(error);
                }
            }
            anyhow::ensure!(Instant::now() < deadline, "photo upload timed out");
            std::thread::sleep(Duration::from_millis(2));
        };
        // The second placement may finish a shared upload after the first used
        // its placeholder. Measure a fresh frame after all placements are ready.
        let _ = frame;
        let frame = renderer.render_scene_to_linear_p3(&scene, size(gpui::DevicePixels(64), gpui::DevicePixels(32)))?;
        let preview = frame.pixels[16 * 64 + 16].map(half);
        let native = frame.pixels[16 * 64 + 48].map(half);
        anyhow::ensure!(
            preview[0] > 0.3 && native[0] > 0.3,
            "comparison captured placeholders instead of orange umbrella"
        );
        let delta = (0..3).map(|i| (native[i] - preview[i]).abs()).fold(0., f64::max);
        eprintln!("Photo Metal FP16: preview {preview:?}; native {native:?}; max channel delta {delta}");
        anyhow::ensure!(delta > 0.02, "photo paint paths collapsed: {preview:?} / {native:?}");
        Ok(())
    }
    #[test]
    fn all_three_reference_images_finish_preparation() -> anyhow::Result<()> {
        use std::time::{Duration, Instant};
        let mut prepared = prepare(&AtomicBool::new(false))?;
        assert_eq!(prepared.jobs.len(), 3);
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut images = Vec::new();
        for job in &mut prepared.jobs {
            loop {
                if let Some(image) = job.try_take() {
                    images.push(image);
                    break;
                }
                if let xgpui_media::LoadStatus::Failed(error) = job.status() {
                    anyhow::bail!(error);
                }
                anyhow::ensure!(Instant::now() < deadline, "Italy image preparation timed out");
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        assert_eq!(images.len(), 3);
        Ok(())
    }
    #[test]
    fn webkit_assets_are_pinned_and_prepared_pixels_are_valid() {
        let originals: [&[u8]; 3] = [
            include_bytes!("../../../assets/colorimetry/photos/Italy-sRGB.jpg"),
            include_bytes!("../../../assets/colorimetry/photos/Italy-P3.jpg"),
            include_bytes!("../../../assets/colorimetry/photos/Italy-oog.jpg"),
        ];
        let hashes = [
            "66f01f04a4cfd12ccc4607b243dce66cbff67ee7dc7c6c486238417f6e46e943",
            "8e78d38839fa044bbcc0dec99c91c82561205bc691d00c4128528d89459bb7ed",
            "ce527523ad5c59350aec553cd705b64fca6483d78d6cf64b6a7ef223e1f4d619",
        ];
        for (bytes, expected) in originals.into_iter().zip(hashes) {
            assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected);
        }
        for bytes in BUFFERS {
            assert_eq!(bytes.len(), 600 * 400 * 8);
            assert!(pixels(bytes).iter().all(|p| p.iter().all(|v| v.is_finite()) && p[3] == 1.));
        }
        assert!(prepare(&AtomicBool::new(true)).is_err());
    }
}
