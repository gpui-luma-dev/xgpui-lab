//! Render-start cadence adapted from media-lab/ui_perf.rs for the embedded MediaPage.
use super::{Completion, check_cancel};
use super::super::*;
use gpui::Window;
use serde_json::json;
use std::{sync::atomic::AtomicBool, time::Instant};

#[derive(Default)]
struct Samples(Vec<f64>);
impl Samples {
    fn report(&self) -> serde_json::Value {
        let mut sorted = self.0.clone();
        sorted.sort_by(f64::total_cmp);
        let rank = |p: f64| sorted.get(((sorted.len() as f64 * p).ceil() as usize).saturating_sub(1)).copied();
        json!({"raw_ms": self.0, "p50_ms": rank(0.5), "p95_ms": rank(0.95), "max_ms": sorted.last(),
            "over_25_ms": sorted.iter().filter(|&&value| value > 25.).count(),
            "over_50_ms": sorted.iter().filter(|&&value| value > 50.).count()})
    }
}

pub(crate) struct Run {
    completion: Completion,
    cancel: Arc<AtomicBool>,
    started: Instant,
    previous: Option<Instant>,
    stage: usize,
    baseline: Samples,
    burst: Samples,
    cached: Samples,
    source_size: [u32; 2],
}
impl Run {
    pub fn new(page: &MediaPage, completion: Completion, cancel: Arc<AtomicBool>) -> Self {
        let source = &page.fixtures.as_ref().expect("cadence fixtures prepared").samples[page.selected];
        Self {
            completion,
            cancel,
            started: Instant::now(),
            previous: None,
            stage: 0,
            baseline: Samples::default(),
            burst: Samples::default(),
            cached: Samples::default(),
            source_size: [source.width, source.height],
        }
    }
    pub fn tick(&mut self, page: &mut MediaPage, window: &mut Window) -> bool {
        let error = check_cancel(&self.cancel)
            .err()
            .map(|error| error.to_string())
            .or_else(|| page.error.clone())
            .or_else(|| (self.started.elapsed().as_secs() > 90).then(|| "cadence run timed out".into()));
        if let Some(error) = error {
            self.finish(page, vec![error]);
            return false;
        }
        let now = Instant::now();
        let dt = self.previous.replace(now).map(|previous| now.duration_since(previous).as_secs_f64() * 1000.);
        match self.stage {
            0 if page.image.as_ref().is_some_and(|image| image.upload_status() == UploadStatus::Ready) => {
                self.stage = 1;
                self.previous = None;
            }
            1 => {
                if let Some(dt) = dt {
                    self.baseline.0.push(dt);
                }
                if self.baseline.0.len() >= 180 {
                    page.completed = 0;
                    page.waiting = 12;
                    self.stage = 2;
                }
            }
            2 => {
                if let Some(dt) = dt {
                    self.burst.0.push(dt);
                }
                if page.waiting == 0
                    && page.burst_jobs.is_empty()
                    && page.burst_images.is_empty()
                    && page.completed == 12
                {
                    self.stage = 3;
                }
            }
            3 => {
                if let Some(dt) = dt {
                    self.cached.0.push(dt);
                }
                if self.cached.0.len() >= 180 {
                    self.finish(page, Vec::new());
                    return false;
                }
            }
            _ => {}
        }
        window.request_animation_frame();
        true
    }
    fn finish(&mut self, page: &mut MediaPage, errors: Vec<String>) {
        page.sweep = false;
        *self.completion.borrow_mut() = Some(Ok(json!({
            "schema_version": 1, "profile": super::profile(), "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "description": "Visible embedded Resource Limits lab with zoom sweep: warm primary, 180 baseline intervals, 12 fresh decode/mip/upload snapshots, 180 cached intervals. Render-start cadence includes UI, driver and scheduling, not GPU/presentation timing. Generated fixtures precede measurement. Burst previews retire after host submission; cached phase returns to baseline geometry. No intervals discarded.",
            "source_size": self.source_size, "decode_workers": page.job_limit, "staging_budget_bytes": ([32,64,128][page.budget_index] * MIB),
            "baseline": self.baseline.report(), "burst": self.burst.report(), "cached": self.cached.report(),
            "completed_sources": page.completed, "errors": errors
        })));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cadence_report_keeps_long_stalls_and_empty_samples_are_not_zero() {
        let samples = Samples(vec![16., 17., 300., 16.]).report();
        assert_eq!(samples["raw_ms"].as_array().unwrap().len(), 4);
        assert_eq!(samples["p95_ms"], 300.);
        assert_eq!(samples["over_50_ms"], 1);
        assert!(Samples::default().report()["p95_ms"].is_null());
    }
}
