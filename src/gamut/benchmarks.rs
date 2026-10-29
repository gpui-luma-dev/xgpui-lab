//! In-app adaptation of gamut-lab's checks.rs: draw the real lab and read back its scene.
use super::*;
use anyhow::{Context as _, Result, ensure};
use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, img, size};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

const FRAMES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    Texture,
    Instanced,
    Gradient,
    Sdr,
    Controls,
}
impl Case {
    const ALL: [Self; 5] = [Self::Texture, Self::Instanced, Self::Gradient, Self::Sdr, Self::Controls];
    fn id(self) -> &'static str {
        match self {
            Self::Texture => "texture",
            Self::Instanced => "instanced",
            Self::Gradient => "gradient",
            Self::Sdr => "sdr",
            Self::Controls => "controls",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Texture => "Wide texture",
            Self::Instanced => "Wide instanced",
            Self::Gradient => "Wide gradient",
            Self::Sdr => "SDR baseline",
            Self::Controls => "Current lab controls",
        }
    }
    fn settings(self, current: &Settings) -> Settings {
        if self == Self::Controls {
            return current.clone();
        }
        Settings {
            tab: "Presentation".into(),
            route: match self {
                Self::Instanced => 1,
                Self::Gradient => 2,
                _ => 0,
            },
            wide: self != Self::Sdr,
            ..Settings::default()
        }
    }
}

#[derive(Clone)]
pub(super) struct Settings {
    tab: SharedString,
    preset: usize,
    policy: GamutMapping,
    translucent: bool,
    overlay: bool,
    half_opacity: bool,
    reverse: bool,
    dark: bool,
    route: usize,
    wide: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            tab: "Presentation".into(),
            preset: 1,
            policy: GamutMapping::Clip,
            translucent: false,
            overlay: true,
            half_opacity: false,
            reverse: false,
            dark: true,
            route: 0,
            wide: true,
        }
    }
}
impl Settings {
    pub(super) fn from_page(page: &GamutPage) -> Self {
        Self {
            tab: page.last_lab_tab.clone(),
            preset: page.preset,
            policy: page.policy,
            translucent: page.translucent,
            overlay: page.overlay,
            half_opacity: page.half_opacity,
            reverse: page.reverse,
            dark: page.dark,
            route: page.route,
            wide: page.requested_wide,
        }
    }
    fn apply(&self, page: &mut GamutPage, cx: &mut Context<GamutPage>) {
        page.active_tab = self.tab.clone();
        page.last_lab_tab = self.tab.clone();
        page.preset = self.preset;
        page.policy = self.policy;
        page.translucent = self.translucent;
        page.overlay = self.overlay;
        page.half_opacity = self.half_opacity;
        page.reverse = self.reverse;
        page.dark = self.dark;
        page.route = self.route;
        page.requested_wide = self.wide;
        for (action, switch) in &page.switches {
            let checked = if *action == Action::Alpha {
                self.translucent
            } else {
                self.overlay
            };
            switch.update(cx, |switch, cx| switch.set_data(checked, cx));
        }
    }
}

struct Capture {
    image: image::RgbaImage,
    report: Value,
}

fn median(values: &[f64]) -> f64 {
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.
    } else {
        values[middle]
    }
}

fn capture(case: Case, current: &Settings, look: Arc<Look>, cx: &mut App) -> Result<Capture> {
    let settings = case.settings(current);
    let bounds = Bounds::centered(None, size(px(1100.), px(920.)), cx);
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            show: false,
            focus: false,
            ..Default::default()
        },
        |window, cx| {
            window.set_window_title("XGPUI Lab · Gamut benchmark");
            cx.new(|cx| {
                let mut page = GamutPage::new(look, None, cx);
                settings.apply(&mut page, cx);
                page
            })
        },
    )?;
    // Always close the temporary window, including negotiation/readback failures.
    let result = (|| -> Result<Capture> {
        let mode = if settings.wide {
            WindowColorMode::WideGamutSdr
        } else {
            WindowColorMode::Default
        };
        handle.update(cx, |page, window, _| -> Result<()> {
            window.set_color_mode(mode).context("negotiating benchmark presentation")?;
            ensure!(page.samples.is_ok(), "Gamut color references could not be initialized");
            if settings.wide && settings.route > 0 {
                page.painter.as_ref().map_err(|error| anyhow::anyhow!("{error:#}"))?;
            }
            Ok(())
        })??;
        let errors = handle.read(cx)?.paint_errors.clone();
        cx.update_window(handle.into(), |_, window, cx| -> Result<Capture> {
            let status = window.color_status();
            let fallback = settings.wide && status.effective() != WindowColorMode::WideGamutSdr;
            let mut draw_ms = Vec::with_capacity(FRAMES);
            let mut readback_ms = Vec::with_capacity(FRAMES);
            let mut image = None;
            for _ in 0..FRAMES {
                window.refresh();
                let started = Instant::now();
                window.draw(cx).clear(cx);
                draw_ms.push(started.elapsed().as_secs_f64() * 1000.);
                ensure!(errors.borrow().is_empty(), "Gamut painting failed: {}", errors.borrow().join("; "));
                let started = Instant::now();
                image = Some(window.render_to_image().context("reading back Gamut lab pixels")?);
                readback_ms.push(started.elapsed().as_secs_f64() * 1000.);
            }
            let image = image.context("benchmark produced no frame")?;
            ensure!(image.width() >= 960 && image.height() >= 640, "incomplete Gamut viewport");
            let report = json!({
                "case": case.id(), "label": case.label(), "outcome": if fallback { "sRGB fallback" } else { "completed" },
                "app_version": env!("CARGO_PKG_VERSION"), "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
                "tab": settings.tab.as_ref(), "route": (["texture", "instanced", "gradient"][settings.route]),
                "settings": { "palette": reference::PRESETS[settings.preset], "mapping": format!("{:?}", settings.policy),
                    "transparency": settings.translucent, "overlays": settings.overlay, "half_opacity": settings.half_opacity,
                    "reverse": settings.reverse, "dark_background": settings.dark },
                "requested": format!("{mode:?}"), "accepted": format!("{:?}", status.requested()),
                "effective": format!("{:?}", status.effective()), "display_wide_gamut": status.display_wide_gamut(),
                "supports_wide_gamut_sdr": status.supports_wide_gamut_sdr(), "fallback": format!("{:?}", status.fallback()),
                "frames": FRAMES, "width": image.width(), "height": image.height(), "scale_factor": window.scale_factor(),
                "draw_ms": draw_ms, "render_readback_ms": readback_ms,
                "first_draw_ms": draw_ms[0], "warm_draw_median_ms": median(&draw_ms[1..]),
                "first_render_readback_ms": readback_ms[0], "warm_render_readback_median_ms": median(&readback_ms[1..]),
                "measurement": "CPU layout/paint and render + readback; excludes PNG encoding and file writes. Not GPU execution time or display calibration.",
                "preview": "Clipped sRGB PNG; does not preserve or verify physical P3 output."
            });
            Ok(Capture { image, report })
        })?
    })();
    let closed = cx.update_window(handle.into(), |_, window, _| window.remove_window());
    result.and_then(|capture| {
        closed?;
        Ok(capture)
    })
}

fn save_capture(mut capture: Capture, output: &Path, case: Case) -> Result<Value> {
    std::fs::create_dir_all(output)?;
    capture.report["pixel_checks"] = super::compositing_checks::verify(&output.join("compositing"))?;
    capture.image.save(output.join(format!("{}.png", case.id())))?;
    std::fs::write(output.join(format!("{}.json", case.id())), serde_json::to_vec_pretty(&capture.report)?)?;
    Ok(capture.report)
}

fn write_summary(output: &Path, results: &[Value]) -> Result<()> {
    std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&json!({"cases": results}))?)?;
    Ok(())
}

pub(super) struct Benchmarks {
    look: Arc<Look>,
    pub settings: Settings,
    buttons: Vec<Entity<Button>>,
    reveal: Entity<Button>,
    busy: bool,
    status: String,
    output: Option<PathBuf>,
    results: Vec<Value>,
}
impl Benchmarks {
    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let buttons = std::iter::once((None, "Run all"))
            .chain(Case::ALL.map(|case| (Some(case), case.label())))
            .enumerate()
            .map(|(index, (case, label))| {
                radix::Button::new(format!("gamut-benchmark-{index}"))
                    .look(&look)
                    .outline()
                    .label(label)
                    .on_click(cx, move |this: &mut Self, cx| this.start(case, cx))
                    .spawn(cx)
            })
            .collect();
        let reveal = radix::Button::new("gamut-benchmark-reveal")
            .look(&look)
            .outline()
            .label("Reveal results")
            .on_click(cx, |this: &mut Self, cx| {
                if let Some(path) = &this.output {
                    cx.reveal_path(path);
                }
            })
            .spawn(cx);
        Self {
            look,
            settings: Settings::default(),
            buttons,
            reveal,
            busy: false,
            status: "Choose a workload. Each run draws five frames using this app’s Gamut renderer and Luma controls."
                .into(),
            output: None,
            results: Vec::new(),
        }
    }
    fn start(&mut self, case: Option<Case>, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let output = match tempfile::Builder::new().prefix("xgpui-gamut-benchmarks-").tempdir() {
            Ok(directory) => directory.keep(),
            Err(error) => {
                self.status = format!("Cannot create results folder: {error}");
                cx.notify();
                return;
            }
        };
        let cases = case.map_or_else(|| Case::ALL.to_vec(), |case| vec![case]);
        self.busy = true;
        self.results.clear();
        self.output = Some(output.clone());
        self.status = "Running Gamut benchmarks…".into();
        for button in &self.buttons {
            button.update(cx, |button, cx| button.set_enabled(false, cx));
        }
        let settings = self.settings.clone();
        let look = self.look.clone();
        cx.spawn(async move |this, cx| {
            for case in cases {
                let captured = this.update(cx, |this, cx| {
                    this.status = format!("Running {}…", case.label());
                    cx.notify();
                    capture(case, &settings, look.clone(), cx)
                });
                let result = match captured {
                    Ok(Ok(captured)) => {
                        let output = output.clone();
                        cx.background_spawn(async move { save_capture(captured, &output, case) }).await
                    }
                    Ok(Err(error)) => Err(error),
                    Err(_) => return,
                };
                if this
                    .update(cx, |this, cx| {
                        this.results.push(result.unwrap_or_else(
                            |error| json!({"case": case.id(), "label": case.label(), "error": format!("{error:#}")}),
                        ));
                        if let Err(error) = write_summary(&output, &this.results) {
                            this.status = format!("Cannot save benchmark report: {error:#}");
                            this.busy = false;
                            for button in &this.buttons {
                                button.update(cx, |button, cx| button.set_enabled(true, cx));
                            }
                            cx.notify();
                            return false;
                        }
                        cx.notify();
                        true
                    })
                    .ok()
                    != Some(true)
                {
                    return;
                }
            }
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                let failures = this.results.iter().filter(|result| result.get("error").is_some()).count();
                let fallbacks = this.results.iter().filter(|result| result["outcome"] == "sRGB fallback").count();
                this.status = format!(
                    "Finished: {} workloads, {failures} failures, {fallbacks} sRGB fallbacks.",
                    this.results.len()
                );
                for button in &this.buttons {
                    button.update(cx, |button, cx| button.set_enabled(true, cx));
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
impl Render for Benchmarks {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("gamut-benchmarks").size_full().overflow_y_scroll().p_4().flex().flex_col().gap_4()
            .text_color(self.look.chrome().body_text)
            .child("Gamut benchmarks")
            .child(div().flex().flex_wrap().gap_2().children(self.buttons.clone()))
            .child("Measures CPU layout/paint and render + readback. PNG encoding and file writes are excluded. These are not GPU execution timings.")
            .child("Wide cases use the Presentation scene. Current lab controls captures the last Gamut tab and its settings. PNG previews are clipped sRGB; presentation negotiation is recorded separately.")
            .child(self.status.clone())
            .when_some(self.output.as_ref(), |root, output| root.child(format!("Results: {}", output.display())).child(self.reveal.clone()))
            .children(self.results.iter().map(|result| {
                let mut row = div().flex().flex_col().gap_2().child(result["label"].as_str().unwrap_or("Workload").to_owned());
                if let Some(error) = result["error"].as_str() { return row.child(format!("Failed: {error}")); }
                row = row.child(format!("{} · requested {} · effective {}", result["outcome"].as_str().unwrap_or("unknown"), result["requested"].as_str().unwrap_or("unknown"), result["effective"].as_str().unwrap_or("unknown")))
                    .child(format!("Warm median: draw {:.3} ms · render + readback {:.3} ms · {} frames", result["warm_draw_median_ms"].as_f64().unwrap_or(0.), result["warm_render_readback_median_ms"].as_f64().unwrap_or(0.), FRAMES));
                if let (Some(output), Some(case)) = (&self.output, result["case"].as_str()) {
                    row = row.child(img(output.join(format!("{case}.png"))).max_w(px(760.)));
                }
                row
            }))
    }
}

/// Developer smoke runs use the same embedded workloads and renderer as the Benchmarks tab.
pub(crate) fn run_cli(args: &[String]) -> Result<()> {
    let case = args
        .iter()
        .position(|arg| arg == "--gamut-benchmark")
        .and_then(|index| args.get(index + 1))
        .context("expected --gamut-benchmark <all|texture|instanced|gradient|sdr|controls>")?;
    let cases = if case == "all" {
        Case::ALL.to_vec()
    } else {
        vec![Case::ALL.into_iter().find(|candidate| candidate.id() == case).context("unknown Gamut benchmark")?]
    };
    let output = args
        .iter()
        .position(|arg| arg == "--output")
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from)
        .context("Gamut benchmarks require --output <directory>")?;
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    gpui_platform::application().with_assets(crate::assets::Assets).run(move |cx| {
        if let Err(error) = gpui_luma::init(cx) {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        let mut results = Vec::new();
        for case in cases {
            let result = capture(case, &Settings::default(), crate::theme::default_look(), cx)
                .and_then(|capture| save_capture(capture, &output, case));
            results.push(result.unwrap_or_else(|error| json!({"case": case.id(), "error": format!("{error:#}")})));
        }
        let saved = write_summary(&output, &results);

        if saved.is_err() || results.iter().any(|result| result.get("error").is_some()) {
            eprintln!("Gamut benchmarks failed: {results:#?}; report save: {saved:?}");
            std::process::exit(1);
        }
        cx.quit();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workloads_select_routes_and_preserve_the_current_controls_snapshot() {
        let current =
            Settings { tab: "Gamut Mapping".into(), route: 2, wide: false, overlay: false, ..Settings::default() };
        for (case, route, wide) in [
            (Case::Texture, 0, true),
            (Case::Instanced, 1, true),
            (Case::Gradient, 2, true),
            (Case::Sdr, 0, false),
        ] {
            let settings = case.settings(&current);
            assert_eq!(settings.tab.as_ref(), "Presentation");
            assert_eq!(settings.route, route);
            assert_eq!(settings.wide, wide);
        }
        let controls = Case::Controls.settings(&current);
        assert_eq!(controls.tab.as_ref(), "Gamut Mapping");
        assert_eq!(controls.route, 2);
        assert!(!controls.wide && !controls.overlay);
        assert_eq!(current.tab.as_ref(), "Gamut Mapping");
    }
    #[cfg(feature = "test-support")]
    #[test]
    fn benchmarks_tab_snapshots_controls_and_failed_readback_closes_its_window() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| GamutPage::new(crate::theme::default_look(), None, cx));
        page.update(cx, |page, cx| {
            page.preset = 2;
            page.route = 2;
            page.overlay = false;
            page.half_opacity = true;
            page.activate("Gamut Mapping".into(), cx);
            page.activate("Benchmarks".into(), cx);
        });
        cx.run_until_parked();
        let before = cx.update(|_, cx| cx.windows().len());
        cx.update(|_, cx| {
            let settings = page.read(cx).benchmarks.read(cx).settings.clone();
            assert_eq!(settings.tab.as_ref(), "Gamut Mapping");
            assert_eq!(settings.preset, 2);
            assert!(!settings.overlay && settings.half_opacity);
            // The mocked platform has no pixel readback. Failure must still close the hidden window.
            assert!(capture(Case::Sdr, &settings, crate::theme::default_look(), cx).is_err());
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert_eq!(cx.windows().len(), before);
            let page = page.read(cx);
            assert_eq!(page.active_tab.as_ref(), "Benchmarks");
            assert_eq!(page.route, 2);
            assert!(!page.overlay && page.half_opacity);
        });
    }
    #[test]
    fn reports_keep_failures_and_measurement_samples() {
        assert_eq!(median(&[9., 1., 5., 3.]), 4.);
        let output = tempfile::tempdir().unwrap();
        let results = vec![
            json!({"case": "texture", "draw_ms": [1., 2.]}),
            json!({"case": "gradient", "error": "readback failed"}),
        ];
        write_summary(output.path(), &results).unwrap();
        let report: Value = serde_json::from_slice(&std::fs::read(output.path().join("report.json")).unwrap()).unwrap();
        assert_eq!(report["cases"][0]["draw_ms"], json!([1., 2.]));
        assert_eq!(report["cases"][1]["error"], "readback failed");
    }
}
