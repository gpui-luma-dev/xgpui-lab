//! CPU preview preparation and UI render cadence, not GPU execution timing.
use std::time::{Duration, Instant};
#[derive(Default)]
pub(super) struct Metrics {
    last: Option<Instant>,
    intervals: usize,
    elapsed: Duration,
    cpu: Duration,
    samples: usize,
    pub cpu_ms: f64,
    pub fps: Option<f64>,
}
impl Metrics {
    pub fn sample(&mut self, now: Instant, cpu: Duration, animated: bool) {
        if !animated {
            self.last = None;
            self.intervals = 0;
            self.elapsed = Duration::ZERO;
            self.cpu = Duration::ZERO;
            self.samples = 0;
            self.cpu_ms = cpu.as_secs_f64() * 1000.;
            self.fps = None;
            return;
        }
        self.cpu += cpu;
        self.samples += 1;
        if let Some(last) = self.last.replace(now) {
            self.elapsed += now.duration_since(last);
            self.intervals += 1;
        }
        if self.elapsed.as_secs_f64() >= 0.25 {
            self.fps = Some(self.intervals as f64 / self.elapsed.as_secs_f64());
            self.cpu_ms = self.cpu.as_secs_f64() * 1000. / self.samples as f64;
            self.intervals = 0;
            self.elapsed = Duration::ZERO;
            self.cpu = Duration::ZERO;
            self.samples = 0;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cadence_averages_samples_and_clears_when_stopped() {
        let mut metrics = Metrics::default();
        let start = Instant::now();
        for i in 0..=20 {
            metrics.sample(start + Duration::from_millis(i * 20), Duration::from_micros(200), true);
        }
        assert!((metrics.fps.unwrap() - 50.).abs() < 0.001);
        assert!((metrics.cpu_ms - 0.2).abs() < 0.001);
        metrics.sample(start + Duration::from_secs(2), Duration::ZERO, false);
        assert_eq!(metrics.fps, None);
    }
}

#[cfg(all(test, feature = "test-support"))]
mod control_tests {
    use super::super::*;
    #[test]
    fn unsupported_modes_reset_adjustments_and_supported_tabs_restore_controls() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            for renderer in [PreviewRenderer::ShaderPaint, PreviewRenderer::ShaderBrush] {
                view.preview_renderer = PreviewRenderer::Quads;
                view.image_adjustments.saturation = 40.;
                view.image_adjustments.vibrance = -20.;
                view.saturation_slider.update(cx, |slider, cx| slider.set_value(40., cx));
                view.vibrance_slider.update(cx, |slider, cx| slider.set_value(-20., cx));
                view.preview_renderer = renderer;
                view.sync_adjustment_support(cx);
                assert!(!view.adjustments_supported());
                assert!(view.image_adjustments.is_neutral());
                assert_eq!(view.saturation_slider.read(cx).value(), 0.);
                assert_eq!(view.vibrance_slider.read(cx).value(), 0.);
            }
            view.preview_renderer = PreviewRenderer::Quads;
            view.image_adjustments.saturation = 50.;
            view.image_adjustments.vibrance = 30.;
            view.preview_p3 = true;
            view.sync_adjustment_support(cx);
            assert!(!view.adjustments_supported());
            assert!(view.image_adjustments.is_neutral());
            view.selected_tab = BuilderTab::Mesh;
            view.sync_adjustment_support(cx);
            assert!(view.adjustments_supported());
            view.selected_tab = BuilderTab::Gradients;
            view.preview_p3 = false;
            view.sync_adjustment_support(cx);
            assert!(view.adjustments_supported());
        });
    }
}
