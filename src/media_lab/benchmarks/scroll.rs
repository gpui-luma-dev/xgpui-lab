//! Real scroll-container stress test; cadence is measured at render start.
use anyhow::Result;
use gpui::{prelude::*, *};
use serde::Serialize;
use std::{path::PathBuf, time::Instant};
use xgpui_media::{
    FrameUploadLimits, ImageJob, LoadStatus, MediaImage, MediaLimits, MediaLoader, UploadLimits, UploadStatus,
    image_view,
};

const SOURCES: usize = 4;
const ROWS: usize = 24;
const ROW: f32 = 280.;
const MIB: usize = 1024 * 1024;
#[derive(Default, Serialize)]
struct Cadence {
    raw_ms: Vec<f64>,
    p50_ms: f64,
    p95_ms: f64,
    max_ms: f64,
    over_two_periods: usize,
    over_50_ms: usize,
}
impl Cadence {
    fn finish(&mut self) {
        let mut values = self.raw_ms.clone();
        values.sort_by(f64::total_cmp);
        if values.is_empty() {
            return;
        }
        self.p50_ms = values[(values.len() as f64 * 0.5).ceil() as usize - 1];
        self.p95_ms = values[(values.len() as f64 * 0.95).ceil() as usize - 1];
        self.max_ms = *values.last().unwrap();
        self.over_two_periods = values.iter().filter(|&&v| v > 1000. / 30.).count();
        self.over_50_ms = values.iter().filter(|&&v| v > 50.).count();
    }
    fn acceptable(&self) -> bool {
        !self.raw_ms.is_empty()
            && self.p95_ms <= 20.
            && self.over_two_periods as f64 / self.raw_ms.len() as f64 <= 0.01
            && self.over_50_ms == 0
    }
}
#[derive(Serialize)]
struct Report {
    schema_version: u32,
    profile: &'static str,
    os: &'static str,
    arch: &'static str,
    target_hz: u32,
    reported_display_interval_ms: Option<f64>,
    description: &'static str,
    acceptance_criteria: &'static str,
    source_size: [u32; 2],
    source_payload_bytes: usize,
    frame_upload_limit: usize,
    frame_byte_limit: usize,
    baseline: Cadence,
    loading: Cadence,
    cached: Cadence,
    replacements_completed: usize,
    admission_retries: usize,
    sampled_peak_workers: usize,
    sampled_peak_staging_bytes: usize,
    scroll_min_y: f32,
    scroll_max_y: f32,
    errors: Vec<String>,
    provisional_60hz_pass: bool,
}
struct Pending {
    index: usize,
    job: Option<ImageJob>,
    image: Option<MediaImage>,
}
pub struct ScrollPerf {
    _directory: tempfile::TempDir,
    paths: Vec<PathBuf>,
    loader: MediaLoader,
    images: Vec<Option<MediaImage>>,
    pending: Option<Pending>,
    scroll: ScrollHandle,
    completion: super::Completion,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    report: Report,
    stage: usize,
    started: Instant,
    phase_started: Instant,
    previous: Option<Instant>,
    next_load: Instant,
    auto_scroll: bool,
}
pub struct Prepared {
    directory: tempfile::TempDir,
    paths: Vec<PathBuf>,
}
pub fn prepare(cancel: &std::sync::atomic::AtomicBool) -> Result<Prepared> {
    let directory = tempfile::Builder::new().prefix("media-scroll-").tempdir()?;
    let mut paths = vec![];
    for index in 0..SOURCES {
        super::check_cancel(cancel)?;
        let pixels = image::RgbaImage::from_fn(3072, 2048, |x, y| {
            let noise = (x.wrapping_mul(1664525) ^ y.wrapping_mul(1013904223) ^ (index as u32 * 7919)) >> 24;
            image::Rgba([(x / 12 + index as u32 * 47) as u8, (y / 8 + index as u32 * 31) as u8, noise as u8, 255])
        });
        let path = directory.path().join(format!("large-{index}.png"));
        pixels.save(&path)?;
        paths.push(path);
    }
    super::check_cancel(cancel)?;
    Ok(Prepared { directory, paths })
}
impl ScrollPerf {
    pub fn new(
        prepared: Prepared,
        completion: super::Completion,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        let Prepared { directory, paths } = prepared;
        let loader = MediaLoader::with_upload_policy(
            MediaLimits { max_image_bytes: 36 * MIB, ..Default::default() },
            UploadLimits::default(),
            FrameUploadLimits {
                max_uploads: 1,
                // Full-chain staging includes 256-byte row padding.
                max_bytes: 33 * MIB,
            },
        )?;
        let now = Instant::now();
        Ok(Self {
            _directory: directory,
            paths,
            loader,
            images: vec![None; SOURCES],
            pending: None,
            scroll: ScrollHandle::new(),
            completion,
            cancel,
            stage: 0,
            started: now,
            phase_started: now,
            previous: None,
            next_load: now,
            auto_scroll: true,
            report: Report {
                schema_version: 1,
                profile: super::profile(),
                os: std::env::consts::OS,
                arch: std::env::consts::ARCH,
                target_hz: 60,
                reported_display_interval_ms: None,
                description: "Native scroll container: 24 cards sharing four retained 3072x2048 images, 1200 points/s triangular scrolling; warmup, 5s cached baseline, 30s repeated file decode/mip/upload replacement, drain, 5s cached scrolling. New load at most every 350ms with one pending replacement; old image stays visible until replacement is host-submitted. Same card geometry in all phases. Generated PNGs and OS file cache do not model cold disk. Raw render-start intervals include long gaps and boundary-trigger work, not GPU/presentation/input timing. Keep window foreground at fixed 60 Hz; app has no artificial frame cap. Generation precedes measurement. Pool/resource peaks are sampled, not total process memory.",
                acceptance_criteria: "Provisional 60 Hz check: all three phases p95 <=20ms, <=1% intervals >33.333ms and no >50ms gaps; loading p95 <= baseline p95+2ms; >=30 replacements; >1000 points observed scroll travel; no errors. A failure needs investigation, not sample filtering. Display mode must be independently verified.",
                source_size: [3072, 2048],
                source_payload_bytes: 0,
                frame_upload_limit: 1,
                frame_byte_limit: 33 * MIB,
                baseline: Cadence::default(),
                loading: Cadence::default(),
                cached: Cadence::default(),
                replacements_completed: 0,
                admission_retries: 0,
                sampled_peak_workers: 0,
                sampled_peak_staging_bytes: 0,
                scroll_min_y: 0.,
                scroll_max_y: 0.,
                errors: vec![],
                provisional_60hz_pass: false,
            },
        })
    }
    fn phase(&self) -> &'static str {
        [
            "Warmup",
            "Cached baseline · 5 seconds",
            "Sustained loading · 30 seconds",
            "Draining",
            "Cached scrolling · 5 seconds",
            "Complete",
        ][self.stage]
    }
    fn transition(&mut self, stage: usize, now: Instant) {
        self.stage = stage;
        self.phase_started = now;
    }
    fn poll(&mut self) {
        let Some(pending) = &mut self.pending else {
            return;
        };
        if let Some(job) = &mut pending.job {
            if let Some(image) = job.try_take() {
                self.report.source_payload_bytes = image.payload_bytes();
                pending.image = Some(image);
                pending.job = None;
            } else if let LoadStatus::Failed(error) = job.status() {
                self.report.errors.push(error);
                self.pending = None;
                return;
            }
        }
        if let Some(image) = &pending.image {
            match image.upload_status() {
                UploadStatus::Ready => {
                    self.images[pending.index] = Some(image.clone());
                    if self.stage != 0 {
                        self.report.replacements_completed += 1;
                    }
                    self.pending = None;
                }
                UploadStatus::Failed(error) => {
                    self.report.errors.push(error);
                    self.pending = None;
                }
                _ => {}
            }
        }
    }
    fn start_load(&mut self, index: usize) {
        match self.loader.load(&self.paths[index]) {
            Ok(job) => self.pending = Some(Pending { index, job: Some(job), image: None }),
            Err(_) => self.report.admission_retries += 1,
        }
    }
    fn finish(&mut self) {
        self.report.baseline.finish();
        self.report.loading.finish();
        self.report.cached.finish();
        self.report.provisional_60hz_pass = self.report.baseline.acceptable()
            && self.report.loading.acceptable()
            && self.report.cached.acceptable()
            && self.report.loading.p95_ms <= self.report.baseline.p95_ms + 2.
            && self.report.scroll_max_y - self.report.scroll_min_y > 1000.
            && self.report.replacements_completed >= 30
            && self.report.errors.is_empty()
            && !cfg!(debug_assertions);
        *self.completion.borrow_mut() = Some(serde_json::to_value(&self.report).map_err(anyhow::Error::from));
        self.stage = 5;
        self.auto_scroll = false;
    }
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stage == 5 {
            return;
        }
        if let Err(error) = super::check_cancel(&self.cancel) {
            self.report.errors.push(error.to_string());
            self.finish();
            return;
        }
        let now = Instant::now();
        if self.report.reported_display_interval_ms.is_none() {
            self.report.reported_display_interval_ms = window.refresh_interval(cx).map(|v| v.as_secs_f64() * 1000.);
        }
        let interval = self.previous.replace(now).map(|v| now.duration_since(v).as_secs_f64() * 1000.);
        if let Some(interval) = interval {
            match self.stage {
                1 => self.report.baseline.raw_ms.push(interval),
                2 | 3 => self.report.loading.raw_ms.push(interval),
                4 => self.report.cached.raw_ms.push(interval),
                _ => {}
            }
        }
        self.poll();
        let stats = self.loader.upload_stats();
        self.report.sampled_peak_workers = self.report.sampled_peak_workers.max(stats.active_jobs);
        self.report.sampled_peak_staging_bytes = self.report.sampled_peak_staging_bytes.max(stats.staging_bytes);
        if self.started.elapsed().as_secs() > 90 {
            self.report.errors.push("90 second test timeout".into());
            self.finish();
            return;
        }
        match self.stage {
            0 => {
                if self.pending.is_none() {
                    if let Some(index) = self.images.iter().position(Option::is_none) {
                        self.start_load(index);
                    } else if stats.staging_bytes == 0 {
                        self.transition(1, now);
                        self.previous = None;
                    }
                }
            }
            1 if self.phase_started.elapsed().as_secs_f64() >= 5. => self.transition(2, now),
            2 => {
                if self.phase_started.elapsed().as_secs_f64() >= 30. {
                    self.transition(3, now);
                } else if self.pending.is_none() && now >= self.next_load {
                    self.start_load(self.report.replacements_completed % SOURCES);
                    self.next_load = now + std::time::Duration::from_millis(350);
                }
            }
            3 if self.pending.is_none() && stats.staging_bytes == 0 => self.transition(4, now),
            4 if self.phase_started.elapsed().as_secs_f64() >= 5. => {
                self.finish();
                return;
            }
            _ => {}
        }
        window.request_animation_frame();
    }
}
impl Render for ScrollPerf {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.stage == 0 && cx.reduce_motion() && self.report.errors.is_empty() {
            self.report.errors.push("reduced motion suppresses automated scrolling".into());
        }
        self.tick(window, cx);
        let observed_y = -f32::from(self.scroll.offset().y);
        self.report.scroll_min_y = self.report.scroll_min_y.min(observed_y);
        self.report.scroll_max_y = self.report.scroll_max_y.max(observed_y);
        let height = (f32::from(window.viewport_size().height) - 180.).max(200.);
        if self.auto_scroll && !cx.reduce_motion() {
            let max = (ROWS as f32 * ROW - height).max(0.);
            let travel = (self.started.elapsed().as_secs_f32() * 1200.).rem_euclid(2. * max.max(1.));
            let y = if travel > max { 2. * max - travel } else { travel };
            self.scroll.set_offset(point(px(0.), px(-y)));
        }
        if self.stage == 5 && self.auto_scroll {
            window.request_animation_frame();
        }
        let mut thumbnails = div().flex().gap_2();
        for image in self.images.iter().flatten().chain(self.pending.iter().filter_map(|p| p.image.as_ref())) {
            thumbnails = thumbnails.child(image_view(image.clone()).w(px(64.)).h(px(40.)).rounded(px(4.)));
        }
        let rows = (0..ROWS).map(|index| {
            let source = index % SOURCES;
            let mut card = div().h(px(ROW)).flex_shrink_0().p_2().flex().gap_3().bg(rgb(if index % 2 == 0 {
                0x19232c
            } else {
                0x22303a
            }));
            card =
                card.child(div().w(px(150.)).child(format!("Card {} · source {}\n3072 × 2048", index + 1, source + 1)));
            if let Some(image) = &self.images[source] {
                card = card.child(image_view(image.clone()).w(px(900.)).h(px(260.)).rounded(px(12.)));
            } else {
                card = card.child("Loading generated large image…");
            }
            card
        });
        div()
            .size_full()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(0x101820))
            .text_color(rgb(0xe8eef4))
            .child(format!(
                "60 Hz baseline · {} · {} replacements · {} errors · {:.1}s",
                self.phase(),
                self.report.replacements_completed,
                self.report.errors.len(),
                self.started.elapsed().as_secs_f32()
            ))
            .child("24 scrolling cards · four 3072×2048 sources · one upload / 33 MiB per scene · keep foreground")
            .child(thumbnails)
            .child(
                div()
                    .id("large-image-feed")
                    .h(px(height))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| cx.notify()))
                    .children(rows),
            )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn criteria_keep_long_stalls_and_reject_empty_samples() {
        let mut samples = Cadence::default();
        assert!(!samples.acceptable());
        samples.raw_ms = vec![16.67; 100];
        samples.finish();
        assert!(samples.acceptable());
        samples.raw_ms.push(300.);
        samples.finish();
        assert_eq!(samples.max_ms, 300.);
        assert_eq!(samples.over_50_ms, 1);
        assert!(!samples.acceptable());
    }
}
