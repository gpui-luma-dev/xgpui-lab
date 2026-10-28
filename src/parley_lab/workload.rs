//! A virtualized list using ordinary GPUI text layout and atlas rendering.
use gpui::{
    Context, Font, FontWeight, FrameDurationSnapshot, Render, ScrollStrategy, SharedString, StyledText, TextRun,
    TextSystem, UniformListScrollHandle, Window, WindowTextSystem, div, prelude::*, px, rgb, uniform_list,
};
use std::{hint::black_box, sync::Arc, time::Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

const STYLES: [(f32, f32); 4] = [(18., 400.), (24., 400.), (24., 900.), (18., 400.)];

pub fn corpus(count: usize) -> Vec<SharedString> {
    let phrases = [
        "office affine efficient — café naïve e\u{301} Ångström",
        "Layout, fallback and variable fonts: Hamburgefonts 012345",
        "A quick brown fox jumps over the lazy dog; fi fl ffi",
        "Metrics and glyphs: AV To ffi / office affine / résumé",
    ];
    (0..count)
        .map(|i| {
            format!("{i:05}  {}  [{:08x}]", phrases[i % phrases.len()], (i as u64).wrapping_mul(2654435761)).into()
        })
        .collect()
}

fn styled_font(base: &Font, weight: f32) -> Font {
    Font { weight: FontWeight(weight), ..base.clone() }
}

fn runs(text: &str, font: &Font) -> [TextRun; 1] {
    [TextRun { len: text.len(), font: font.clone(), color: rgb(0xeff4fc).into(), ..Default::default() }]
}

fn rss() -> Option<f64> {
    let pid = sysinfo::get_current_pid().ok()?;
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), ProcessRefreshKind::new().with_memory());
    system.process(pid).map(|p| p.memory() as f64 / 1048576.)
}

fn memory(value: Option<f64>) -> String {
    value.map_or_else(|| "unavailable".into(), |v| format!("{v:.1} MiB"))
}

/// Separate CPU diagnostic. Retain layouts to verify real GPUI cache hits.
/// Font/backend state is shared; this does not measure scrolling or GPU work.
pub fn measure(text: Arc<TextSystem>, lines: &[SharedString], base: &Font) -> anyhow::Result<String> {
    let window = WindowTextSystem::new(text);
    let before = rss();
    let mut report = format!("Scroll corpus: {} distinct lines; process RSS before {}\n", lines.len(), memory(before));
    for (size, weight) in STYLES {
        let font = styled_font(base, weight);
        let start = Instant::now();
        let layouts = lines
            .iter()
            .map(|line| window.layout_line(line, px(size), &runs(line, &font), None))
            .collect::<Vec<_>>();
        let first = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        for (line, original) in lines.iter().zip(&layouts) {
            let cached = window.layout_line(line, px(size), &runs(line, &font), None);
            anyhow::ensure!(Arc::ptr_eq(original, &cached), "expected corpus layout cache hit");
            black_box(cached);
        }
        let repeat = start.elapsed().as_secs_f64() * 1000.;
        report.push_str(&format!("{size:.0}px / weight {weight:.0}: first pass {first:.2}ms ({:.2}us/line), cached pass {repeat:.2}ms; RSS {}\n", first * 1000. / lines.len() as f64, memory(rss())));
    }
    report.push_str("First style pass is cold for these strings; later styles share backend state. Final 18/400 reuses prior layouts. RSS includes retained diagnostic layouts, fonts and the whole process; excludes separate GPU memory.\n");
    Ok(report)
}

pub struct ScrollLab {
    controls: Vec<gpui::Entity<gpui_luma::controls::button::Button>>,
    pending_start: bool,
    name: String,
    lines: Vec<SharedString>,
    font: Font,
    size: f32,
    weight: f32,
    scroll: UniformListScrollHandle,
    running: bool,
    frame: usize,
    limit: usize,
    exit_after: bool,
    seen: Vec<bool>,
    baseline: Option<FrameDurationSnapshot>,
    phase_baseline: Option<FrameDurationSnapshot>,
    rss_start: Option<f64>,
    rss_now: Option<f64>,
}

impl ScrollLab {
    pub fn init_controls(&mut self, look: std::sync::Arc<gpui_luma_look_radix::Look>, cx: &mut Context<Self>) {
        use gpui_luma::prelude::HasPresenter;
        self.controls = ["Run 600 frames", "Pause", "Size 18 / 24", "Weight 400 / 900"]
            .into_iter()
            .enumerate()
            .map(|(id, label)| {
                gpui_luma_look_radix::Button::new(format!("scroll-{id}"))
                    .look(look.as_ref())
                    .outline()
                    .label(label)
                    .on_click(cx, move |this: &mut Self, cx| {
                        if id == 0 {
                            this.pending_start = true;
                        } else {
                            this.running = false;
                            if id == 2 {
                                this.size = if this.size == 18. { 24. } else { 18. };
                            }
                            if id == 3 {
                                this.weight = if this.weight == 400. { 900. } else { 400. };
                            }
                            this.rss_now = rss();
                        }
                        cx.notify();
                    })
                    .spawn(cx)
            })
            .collect();
    }

    pub fn new(name: String, lines: Vec<SharedString>, font: Font, frames: usize, exit_after: bool) -> Self {
        let count = lines.len();
        let initial_rss = rss();
        Self {
            controls: vec![],
            pending_start: false,
            name,
            lines,
            font,
            size: 18.,
            weight: 400.,
            scroll: UniformListScrollHandle::new(),
            running: frames > 0,
            frame: 0,
            limit: if frames == 0 { 600 } else { frames },
            exit_after,
            seen: vec![false; count],
            baseline: None,
            phase_baseline: None,
            rss_start: initial_rss,
            rss_now: initial_rss,
        }
    }

    fn stats(window: &Window, baseline: Option<&FrameDurationSnapshot>) -> String {
        let mut s = window.frame_duration_snapshot();
        if let Some(b) = baseline {
            s.draw_duration_histogram.subtract(&b.draw_duration_histogram).expect("monotonic draw histogram");
            s.present_interval_histogram
                .subtract(&b.present_interval_histogram)
                .expect("monotonic present histogram");
        }
        let draw = s.draw_duration_histogram;
        let interval = s.present_interval_histogram;
        if draw.is_empty() {
            return "Waiting for a drawn frame".into();
        }
        let cadence = if interval.is_empty() {
            "unavailable (0 samples)".into()
        } else {
            format!(
                "{:.2}/{:.2} ms ({} samples)",
                interval.value_at_quantile(0.5) as f64 / 1e6,
                interval.value_at_quantile(0.95) as f64 / 1e6,
                interval.len()
            )
        };
        format!(
            "{} draws | CPU draw p50/p95/max {:.2}/{:.2}/{:.2} ms | submit interval p50/p95 {cadence}",
            draw.len(),
            draw.value_at_quantile(0.5) as f64 / 1e6,
            draw.value_at_quantile(0.95) as f64 / 1e6,
            draw.max() as f64 / 1e6
        )
    }

    fn start(&mut self, frames: usize, window: &Window, cx: &mut Context<Self>) {
        self.baseline = Some(window.frame_duration_snapshot());
        self.phase_baseline = self.baseline.clone();
        self.frame = 0;
        self.limit = frames;
        self.running = true;
        self.seen.fill(false);
        self.rss_start = rss();
        self.size = 18.;
        self.weight = 400.;
        self.scroll.scroll_to_item_strict(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.running {
            return;
        }
        self.frame += 1;
        let completed = ((self.frame - 1) * 4 / self.limit).min(3);
        if self.frame == self.limit || completed != (self.frame * 4 / self.limit).min(3) {
            println!(
                "{} phase {} / {}px weight {}: {}",
                self.name,
                completed + 1,
                STYLES[completed].0,
                STYLES[completed].1,
                Self::stats(window, self.phase_baseline.as_ref())
            );
            self.phase_baseline = Some(window.frame_duration_snapshot());
        }
        if self.frame >= self.limit {
            self.running = false;
            self.rss_now = rss();
            println!(
                "{} scroll: {} callbacks; {}/{} distinct rows built; {} | process RSS {} -> {} | scale {}x, release={}\nSubmission intervals include platform pacing; neither GPU time nor confirmed displayed FPS. UI includes list/header/profiler overhead.\n",
                self.name,
                self.frame,
                self.seen.iter().filter(|v| **v).count(),
                self.lines.len(),
                Self::stats(window, self.baseline.as_ref()),
                memory(self.rss_start),
                memory(self.rss_now),
                window.scale_factor(),
                !cfg!(debug_assertions)
            );
            if self.exit_after {
                cx.quit();
            }
        } else {
            (self.size, self.weight) = STYLES[(self.frame * 4 / self.limit).min(3)];
            self.scroll.scroll_to_item_strict((self.frame * 8) % self.lines.len(), ScrollStrategy::Top);
        }
        cx.notify();
    }
}

impl Render for ScrollLab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.pending_start {
            self.pending_start = false;
            self.start(600, window, cx);
        }
        if self.running {
            if self.baseline.is_none() {
                self.baseline = Some(window.frame_duration_snapshot());
                self.phase_baseline = self.baseline.clone();
            }
            let this = cx.entity();
            window.on_next_frame(move |window, cx| this.update(cx, |this, cx| this.tick(window, cx)));
        }
        let controls = div().flex().gap_3().children(self.controls.clone());
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgb(0x111827))
            .text_color(rgb(0xeff4fc))
            .font_family("IBM Plex Sans")
            .text_size(px(12.))
            .child(format!(
                "{} / scroll / {} lines / {}px / weight {}",
                self.name,
                self.lines.len(),
                self.size,
                self.weight
            ))
            .child(controls)
            .child(Self::stats(window, self.baseline.as_ref()))
            .child(format!(
                "Run {}/{} | distinct rows built {} | process RSS {}",
                self.frame,
                self.limit,
                self.seen.iter().filter(|v| **v).count(),
                memory(self.rss_now)
            ))
            .child("Draw = full CPU view; submit interval includes pacing. GPU memory/time not measured.")
            .child(
                uniform_list(
                    "scroll-corpus",
                    self.lines.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, _| {
                        let font = styled_font(&this.font, this.weight);
                        range
                            .map(|i| {
                                this.seen[i] = true;
                                let text = this.lines[i].clone();
                                let text_runs = runs(&text, &font);
                                div()
                                    .h(px(this.size + 8.))
                                    .text_size(px(this.size))
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .child(StyledText::new(text).with_runs(text_runs.to_vec()))
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0()
                .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify())),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui::PlatformTextSystem;

    #[test]
    fn corpus_styles_are_supported_by_parley() {
        let mut fonts = parley::FontContext::new();
        for data in [crate::parley_lab::samples::INTER, crate::parley_lab::samples::PLEX] {
            fonts
                .collection
                .register_fonts(parley::fontique::Blob::new(std::sync::Arc::new(data.to_vec())), None);
        }
        let backend = xgpui_parley::ParleyTextSystem::new(fonts, "IBM Plex Sans").unwrap();
        let base = crate::parley_lab::samples::samples().unwrap().remove(2).font;
        for (size, weight) in super::STYLES {
            let font = super::styled_font(&base, weight);
            let id = backend.font_id(&font).unwrap();
            for text in super::corpus(4) {
                backend
                    .try_layout_line(&text, gpui::px(size), &[gpui::FontRun { len: text.len(), font_id: id }])
                    .unwrap_or_else(|error| panic!("{size}px / weight {weight} / {text}: {error}"));
            }
        }
    }

    #[test]
    fn corpus_is_deterministic_and_unique() {
        let lines = super::corpus(5000);
        assert_eq!(lines, super::corpus(5000));
        assert_eq!(lines.iter().collect::<std::collections::HashSet<_>>().len(), 5000);
        assert!(lines.iter().all(|line| !line.contains(['\n', '\r'])));
    }
}
