//! Reproducible upload-phase CPU measurement, not a GPU timestamp benchmark.
use anyhow::{Context as _, Result, bail, ensure};
use gpui::*;
use image::ImageEncoder as _;
use serde::Serialize;
use std::{
    any::Any,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use xgpui_media::{FrameUploadLimits, LoadStatus, MediaImage, MediaLimits, MediaLoader, UploadLimits, UploadStatus};

const MIB: usize = 1024 * 1024;
const SOURCES: usize = 6;
const WAVES: usize = 3;
const CACHED_FRAMES: usize = 120;
const FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);
const COLORS: [[u8; 3]; SOURCES] =
    [[220, 60, 40], [48, 192, 64], [48, 96, 224], [224, 184, 48], [176, 64, 208], [48, 192, 192]];

#[derive(Default, Debug)]
struct Trace {
    prepare_ms: f64,
    calls: usize,
    backend: String,
}
struct TimedImage {
    image: MediaImage,
    trace: Arc<Mutex<Trace>>,
}
impl std::fmt::Debug for TimedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimedImage").finish_non_exhaustive()
    }
}
impl TimedImage {
    fn measure(&self, backend: &dyn Any, work: impl FnOnce() -> Result<Box<dyn Any>>) -> Result<Box<dyn Any>> {
        let started = Instant::now();
        let result = work();
        let elapsed = started.elapsed().as_secs_f64() * 1000.;
        let mut trace = self.trace.lock().unwrap();
        trace.prepare_ms += elapsed;
        trace.calls += 1;
        if trace.backend.is_empty() {
            #[cfg(target_os = "macos")]
            if let Some(context) = backend.downcast_ref::<gpui_apple::custom_draw::MetalCustomContext>() {
                trace.backend = format!("Metal / {}", context.device.name());
            }
            let _ = backend;
            if trace.backend.is_empty() {
                trace.backend = "native (backend details unavailable through this lab build)".into();
            }
        }
        result
    }
}
impl CustomContent for TimedImage {
    fn prepare(&self, backend: &dyn Any, extent: Size<DevicePixels>) -> Result<Box<dyn Any>> {
        self.measure(backend, || self.image.prepare(backend, extent))
    }
    fn prepare_in_frame(
        &self,
        backend: &dyn Any,
        request: &PreparationRequest,
        frame: &PreparationFrame,
    ) -> Result<Box<dyn Any>> {
        self.measure(backend, || self.image.prepare_in_frame(backend, request, frame))
    }
}
#[derive(Serialize)]
struct Summary {
    samples: usize,
    mean_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    max_ms: f64,
    cpu_calls_over_16_67_ms: usize,
}
impl Summary {
    fn new(values: &[f64]) -> Result<Self> {
        ensure!(!values.is_empty() && values.iter().all(|v| v.is_finite() && *v >= 0.), "invalid benchmark samples");
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let rank = |p: f64| sorted[((sorted.len() as f64 * p).ceil() as usize).saturating_sub(1)];
        Ok(Self {
            samples: values.len(),
            mean_ms: values.iter().sum::<f64>() / values.len() as f64,
            p50_ms: rank(0.5),
            p95_ms: rank(0.95),
            p99_ms: rank(0.99),
            max_ms: *sorted.last().unwrap(),
            cpu_calls_over_16_67_ms: values.iter().filter(|v| **v > FRAME_INTERVAL.as_secs_f64() * 1000.).count(),
        })
    }
}
#[derive(Serialize)]
struct FrameSample {
    render_scene_cpu_ms: f64,
    summed_source_prepare_cpu_ms: f64,
    observed_new_submissions: usize,
}
#[derive(Serialize)]
struct Wave {
    index: usize,
    observed_drain_wall_ms: f64,
    frames: Vec<FrameSample>,
}
#[derive(Serialize)]
struct Pool {
    allocated_bytes: usize,
    cached_bytes: usize,
    allocations: usize,
    reuses: usize,
}
impl From<ImageStagingPoolStats> for Pool {
    fn from(s: ImageStagingPoolStats) -> Self {
        Self {
            allocated_bytes: s.allocated_bytes,
            cached_bytes: s.cached_bytes,
            allocations: s.allocations,
            reuses: s.reuses,
        }
    }
}
#[derive(Serialize)]
struct Report {
    schema_version: u32,
    profile: &'static str,
    os: &'static str,
    architecture: &'static str,
    backend: String,
    source_count: usize,
    source_dimensions: [u32; 2],
    placements_per_source: usize,
    retained_mip_payload_bytes: usize,
    decode_and_mip_wall_ms: Vec<f64>,
    upload_workers: usize,
    staging_budget_bytes: usize,
    frame_upload_count_limit: usize,
    frame_upload_byte_limit: usize,
    target_frame_interval_ms: f64,
    upload_render_scene_cpu: Summary,
    upload_source_prepare_cpu: Summary,
    cached_render_scene_cpu: Summary,
    cached_source_prepare_cpu: Summary,
    observed_peak_active_upload_workers: usize,
    observed_peak_staging_reservations_bytes: usize,
    pool_before_waves: Pool,
    pool_after_waves: Pool,
    uploads_per_source: Vec<usize>,
    waves: Vec<Wave>,
    cached_frames: Vec<FrameSample>,
    validated_capture: String,
    caveats: Vec<&'static str>,
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds::new(point(ScaledPixels(x), ScaledPixels(y)), size(ScaledPixels(w), ScaledPixels(h)))
}
fn scene(images: &[MediaImage], trace: &Arc<Mutex<Trace>>) -> Scene {
    let mut scene = Scene::default();
    let bounds = rect(0., 0., 900., 600.);
    scene.insert_primitive(Quad {
        bounds,
        content_mask: ContentMask { bounds },
        background: rgb(0x111619).into(),
        ..Default::default()
    });
    for (index, image) in images.iter().enumerate() {
        let x = (index % 3) as f32 * 300.;
        let y = (index / 3) as f32 * 300.;
        let content: Arc<dyn CustomContent> = Arc::new(TimedImage { image: image.clone(), trace: trace.clone() });
        for bounds in [rect(x + 12., y + 12., 256., 256.), rect(x + 224., y + 224., 64., 64.)] {
            scene.insert_primitive(CustomDraw {
                order: 0,
                bounds,
                content_mask: ContentMask { bounds },
                corner_radii: Corners::all(ScaledPixels(12.)),
                opacity: 1.,
                coverage: None,
                content: content.clone(),
            });
        }
    }
    scene.finish();
    scene
}
fn fixture(path: &Path, dimension: u32, color: [u8; 3], seed: u32) -> Result<()> {
    let mut rgba = image::RgbaImage::new(dimension, dimension);
    let mut random = seed;
    for (x, y, pixel) in rgba.enumerate_pixels_mut() {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        let rgb = if x >= dimension / 4 && x < dimension * 3 / 4 && y >= dimension / 4 && y < dimension * 3 / 4 {
            color
        } else {
            [(random & 255) as u8, ((random >> 8) & 255) as u8, ((random >> 16) & 255) as u8]
        };
        *pixel = image::Rgba([rgb[0], rgb[1], rgb[2], 255]);
    }
    image::codecs::png::PngEncoder::new_with_quality(
        std::fs::File::create(path)?,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::NoFilter,
    )
    .write_image(rgba.as_raw(), dimension, dimension, image::ExtendedColorType::Rgba8)?;
    Ok(())
}
fn decode(loader: &MediaLoader, path: &Path, cancel: &std::sync::atomic::AtomicBool) -> Result<(MediaImage, f64)> {
    let start = Instant::now();
    let mut job = loader.load(path)?;
    loop {
        super::check_cancel(cancel)?;
        if let Some(image) = job.try_take() {
            return Ok((image, start.elapsed().as_secs_f64() * 1000.));
        }
        if let LoadStatus::Failed(error) = job.status() {
            bail!(error);
        }
        ensure!(start.elapsed() < Duration::from_secs(60), "benchmark decode timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn ready(images: &[MediaImage]) -> Result<Vec<bool>> {
    images
        .iter()
        .map(|image| match image.upload_status() {
            UploadStatus::Failed(error) => bail!(error),
            UploadStatus::Ready => Ok(true),
            _ => Ok(false),
        })
        .collect()
}
fn draw(
    renderer: &mut dyn PlatformHeadlessRenderer,
    scene: &Scene,
    images: &[MediaImage],
    trace: &Arc<Mutex<Trace>>,
) -> Result<FrameSample> {
    let before = ready(images)?;
    {
        let mut trace = trace.lock().unwrap();
        trace.prepare_ms = 0.;
        trace.calls = 0;
    }
    let started = Instant::now();
    renderer.render_scene(scene, size(DevicePixels(900), DevicePixels(600)))?;
    let elapsed = started.elapsed().as_secs_f64() * 1000.;
    let after = ready(images)?;
    let submissions = before.iter().zip(after).filter(|(old, new)| !**old && *new).count();
    ensure!(submissions <= 1, "per-frame source upload limit exceeded");
    let trace = trace.lock().unwrap();
    ensure!(trace.calls == images.len() * 2, "benchmark source placements were not prepared");
    Ok(FrameSample {
        render_scene_cpu_ms: elapsed,
        summed_source_prepare_cpu_ms: trace.prepare_ms,
        observed_new_submissions: submissions,
    })
}
fn pace(start: Instant) {
    std::thread::sleep(FRAME_INTERVAL.saturating_sub(start.elapsed()));
}

pub fn run(report_path: &Path, png: &Path, cancel: &std::sync::atomic::AtomicBool) -> Result<()> {
    ensure!(report_path != png, "benchmark JSON and PNG outputs must use distinct paths");
    super::check_cancel(cancel)?;
    let mut renderer = gpui_platform::current_headless_renderer()?.context("native headless renderer unavailable")?;
    let loader = MediaLoader::with_upload_policy(
        MediaLimits {
            max_jobs: 2,
            max_resident_bytes: 256 * MIB,
            max_image_bytes: 32 * MIB,
            max_encoded_bytes: 32 * MIB,
        },
        UploadLimits { max_jobs: 2, max_staging_bytes: 64 * MIB },
        FrameUploadLimits {
            max_uploads: 1,
            // Full-chain staging includes 256-byte row padding.
            max_bytes: 33 * MIB,
        },
    )?;
    let directory = tempfile::Builder::new().prefix("xgpui-upload-bench-").tempdir()?;
    let trace = Arc::new(Mutex::new(Trace::default()));
    // Compile pipelines and allocate the render target before timed waves.
    let warm_path = directory.path().join("warm.png");
    fixture(&warm_path, 256, COLORS[0], 1)?;
    let (warm, _) = decode(&loader, &warm_path, cancel)?;
    let warm_scene = scene(std::slice::from_ref(&warm), &trace);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !ready(std::slice::from_ref(&warm))?[0] || loader.upload_stats().staging_bytes > 0 {
        super::check_cancel(cancel)?;
        let tick = Instant::now();
        draw(&mut *renderer, &warm_scene, std::slice::from_ref(&warm), &trace)?;
        ensure!(Instant::now() < deadline, "warmup timeout");
        pace(tick);
    }
    renderer.render_scene_to_image(&warm_scene, size(DevicePixels(900), DevicePixels(600)))?;
    drop(warm_scene);
    drop(warm);

    let mut images = Vec::new();
    let mut decode_ms = Vec::new();
    for (index, color) in COLORS.into_iter().enumerate() {
        super::check_cancel(cancel)?;
        let path = directory.path().join(format!("source-{index}.png"));
        fixture(&path, 2048, color, index as u32 + 17)?;
        let (image, millis) = decode(&loader, &path, cancel)?;
        images.push(image);
        decode_ms.push(millis);
    }
    let scene = scene(&images, &trace);
    let pool_before = loader.staging_pool_stats();
    let mut waves = Vec::new();
    let mut peak_jobs = 0;
    let mut peak_bytes = 0;
    for index in 0..WAVES {
        super::check_cancel(cancel)?;
        if index > 0 {
            for image in &images {
                image.retry_upload();
            }
        }
        let start = Instant::now();
        let mut frames = Vec::new();
        loop {
            super::check_cancel(cancel)?;
            let tick = Instant::now();
            frames.push(draw(&mut *renderer, &scene, &images, &trace)?);
            let status = loader.upload_stats();
            peak_jobs = peak_jobs.max(status.active_jobs);
            peak_bytes = peak_bytes.max(status.staging_bytes);
            ensure!(
                status.active_jobs <= 2 && status.staging_bytes <= 64 * MIB,
                "upload admission exceeded configured bounds"
            );
            if ready(&images)?.iter().all(|v| *v) && status.active_jobs == 0 && status.staging_bytes == 0 {
                break;
            }
            ensure!(start.elapsed() < Duration::from_secs(30), "upload wave did not drain");
            pace(tick);
        }
        ensure!(
            frames.iter().map(|f| f.observed_new_submissions).sum::<usize>() == SOURCES,
            "benchmark did not observe every source submission in this wave"
        );
        let drain_ms = start.elapsed().as_secs_f64() * 1000.;

        waves.push(Wave { index: index + 1, observed_drain_wall_ms: drain_ms, frames });
    }
    let mut cached_frames = Vec::new();
    for _ in 0..CACHED_FRAMES {
        super::check_cancel(cancel)?;
        let tick = Instant::now();
        cached_frames.push(draw(&mut *renderer, &scene, &images, &trace)?);
        pace(tick);
    }
    // Pixel readback waits for rendering, deliberately outside timing samples.
    let pixels = renderer.render_scene_to_image(&scene, size(DevicePixels(900), DevicePixels(600)))?;
    for (index, expected) in COLORS.iter().enumerate() {
        for offset in [140, 256] {
            let actual = pixels.get_pixel((index % 3) as u32 * 300 + offset, (index / 3) as u32 * 300 + offset).0;
            ensure!(
                (0..3).all(|c| actual[c].abs_diff(expected[c]) <= 2),
                "final source {index} placement {offset} missing/wrong: {actual:?}"
            );
        }
        ensure!(images[index].upload_count() == WAVES, "source {index} upload/cache reuse count incorrect");
    }
    pixels.save(png)?;
    let pool_after = loader.staging_pool_stats();
    ensure!(
        pool_after.allocated_bytes <= 64 * MIB && pool_after.cached_bytes <= pool_after.allocated_bytes,
        "staging pool accounting exceeded capacity"
    );
    #[cfg(target_os = "macos")]
    ensure!(pool_after.reuses > pool_before.reuses, "benchmark did not exercise staging-pool reuse");
    let upload_cpu: Vec<_> = waves.iter().flat_map(|w| w.frames.iter().map(|f| f.render_scene_cpu_ms)).collect();
    let upload_prepare: Vec<_> =
        waves.iter().flat_map(|w| w.frames.iter().map(|f| f.summed_source_prepare_cpu_ms)).collect();
    let cached_cpu: Vec<_> = cached_frames.iter().map(|f| f.render_scene_cpu_ms).collect();
    let cached_prepare: Vec<_> = cached_frames.iter().map(|f| f.summed_source_prepare_cpu_ms).collect();
    let report = Report {
        schema_version: 1,
        profile: super::profile(),
        os: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        backend: trace.lock().unwrap().backend.clone(),
        source_count: SOURCES,
        source_dimensions: [2048, 2048],
        placements_per_source: 2,
        retained_mip_payload_bytes: images.iter().map(|i| i.payload_bytes()).sum(),
        decode_and_mip_wall_ms: decode_ms,
        upload_workers: 2,
        staging_budget_bytes: 64 * MIB,
        frame_upload_count_limit: 1,
        frame_upload_byte_limit: 33 * MIB,
        target_frame_interval_ms: FRAME_INTERVAL.as_secs_f64() * 1000.,
        upload_render_scene_cpu: Summary::new(&upload_cpu)?,
        upload_source_prepare_cpu: Summary::new(&upload_prepare)?,
        cached_render_scene_cpu: Summary::new(&cached_cpu)?,
        cached_source_prepare_cpu: Summary::new(&cached_prepare)?,
        observed_peak_active_upload_workers: peak_jobs,
        observed_peak_staging_reservations_bytes: peak_bytes,
        pool_before_waves: pool_before.into(),
        pool_after_waves: pool_after.into(),
        uploads_per_source: images.iter().map(|i| i.upload_count()).collect(),
        waves,
        cached_frames,
        validated_capture: png.display().to_string(),
        caveats: vec![
            "CPU wall timings of render_scene and instrumented source preparation; not GPU timestamps or real window/UI frame rates.",
            "Sources decode/mip before timing; generated PNG creation and pipeline warmup are excluded. Decode/mip wall time is reported separately.",
            "Observed staging drain includes host polling and frame pacing; Metal/wgpu leases include completion callbacks. D3D11 reservations currently release at observed submission. Ready is host-submitted, not GPU-complete.",
            "Frame pacing sleeps and final pixel readback are excluded. Driver backpressure and instrumentation may affect CPU samples.",
            "Nearest-rank percentiles; sample counts are reported. Background system load is uncontrolled; this is one run, not a zero-stutter guarantee.",
            "Observed peaks are sampled once per rendered frame; source/staging budgets do not represent total process memory or VRAM.",
        ],
    };
    std::fs::write(report_path, serde_json::to_string_pretty(&report)? + "\n")?;

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn timing_summary_uses_nearest_rank_and_rejects_invalid_samples() -> Result<()> {
        let s = Summary::new(&[4., 1., 3., 2.])?;
        assert_eq!((s.samples, s.mean_ms, s.p50_ms, s.p95_ms, s.max_ms), (4, 2.5, 2., 4., 4.));
        assert!(Summary::new(&[]).is_err());
        assert!(Summary::new(&[f64::NAN]).is_err());
        Ok(())
    }
}
