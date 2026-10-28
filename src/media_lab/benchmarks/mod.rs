//! Media workloads embedded from media-lab: no source checkout, Cargo or child processes at runtime.
mod atlas;
pub(super) mod cadence;
mod scroll;
mod upload;
mod video;

use super::*;
use anyhow::{Context as _, Result, ensure};
use gpui::{prelude::*, App, AsyncApp, AnyWindowHandle, Render, Window, WindowBounds, WindowOptions, div, img, px, size};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

type Completion = Rc<RefCell<Option<Result<Value>>>>;
fn profile() -> &'static str {
    if cfg!(debug_assertions) { "debug" } else { "release" }
}
fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    ensure!(!cancel.load(Ordering::Relaxed), "benchmark cancelled");
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    Upload,
    Cadence,
    Scroll,
    Atlas,
    Video,
}
impl Case {
    const ALL: [Self; 5] = [Self::Upload, Self::Cadence, Self::Scroll, Self::Atlas, Self::Video];
    fn id(self) -> &'static str {
        match self {
            Self::Upload => "upload",
            Self::Cadence => "cadence",
            Self::Scroll => "scroll",
            Self::Atlas => "atlas",
            Self::Video => "video",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Upload => "Upload benchmark",
            Self::Cadence => "Visible cadence",
            Self::Scroll => "Sustained scrolling",
            Self::Atlas => "Atlas / mipmap capture",
            Self::Video => "Native video checks",
        }
    }
}

fn run_background(case: Case, output: &Path, cancel: &AtomicBool) -> Result<Value> {
    std::fs::create_dir_all(output)?;
    check_cancel(cancel)?;
    let report = output.join("report.json");
    let png = output.join("capture.png");
    let started = Instant::now();
    let mut result = match case {
        Case::Upload => {
            upload::run(&report, &png, cancel)?;
            serde_json::from_slice(&std::fs::read(&report)?)?
        }
        Case::Atlas => {
            let fixtures = Fixtures::new(None)?;
            atlas::run(&fixtures, &png, cancel)?;
            json!({"description": "Generated app fixtures rendered through the GPUI atlas and dedicated mipmapped media textures. Every source must upload exactly once.", "errors": []})
        }
        Case::Video => {
            #[cfg(target_os = "macos")]
            {
                video::run(&png, cancel)?;
                json!({"formats": ["BGRA8", "NV12 full BT.601", "NV12 video BT.601", "NV12 full BT.709", "NV12 video BT.709"],
                    "checks": ["RGB/range/matrix", "rounded corners", "opacity", "content mask", "import reuse after resize"], "errors": []})
            }
            #[cfg(not(target_os = "macos"))]
            {
                json!({"outcome": "unavailable", "description": "The generated CoreVideo source requires macOS Metal.", "errors": []})
            }
        }
        _ => anyhow::bail!("visible workload requires a native window"),
    };
    result["elapsed_wall_ms"] = json!(started.elapsed().as_secs_f64() * 1000.);
    Ok(result)
}

enum Prepared {
    Cadence(Fixtures),
    Scroll(scroll::Prepared),
}
fn prepare_visible(case: Case, cancel: &AtomicBool) -> Result<Prepared> {
    check_cancel(cancel)?;
    match case {
        Case::Cadence => Ok(Prepared::Cadence(Fixtures::new(None)?)),
        Case::Scroll => Ok(Prepared::Scroll(scroll::prepare(cancel)?)),
        _ => anyhow::bail!("not a visible workload"),
    }
}
struct Visible {
    handle: AnyWindowHandle,
    completion: Completion,
}
fn open_visible(prepared: Prepared, look: Arc<Look>, cancel: Arc<AtomicBool>, cx: &mut App) -> Result<Visible> {
    check_cancel(&cancel)?;
    ensure!(!cx.reduce_motion(), "automated cadence/scrolling runs require reduced motion to be off");
    let completion: Completion = Default::default();
    let bounds = Bounds::centered(None, size(px(1400.), px(1000.)), cx);
    let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
    let handle = match prepared {
        Prepared::Cadence(fixtures) => cx
            .open_window(options, |window, cx| {
                window.set_window_title("XGPUI Lab · Media cadence benchmark");
                cx.new(|cx| {
                    let mut page = MediaPage::new(look, cx);
                    page.fixtures = Some(fixtures);
                    page.activate("Resource Limits".into(), cx);
                    page.reload();
                    page.sweep = true;
                    page.cadence = Some(cadence::Run::new(&page, completion.clone(), cancel));
                    page
                })
            })?
            .into(),
        Prepared::Scroll(prepared) => {
            let run = scroll::ScrollPerf::new(prepared, completion.clone(), cancel)?;
            cx.open_window(options, |window, cx| {
                window.set_window_title("XGPUI Lab · Media sustained scrolling");
                cx.new(|_| run)
            })?
            .into()
        }
    };
    cx.activate(true);
    Ok(Visible { handle, completion })
}
async fn wait_visible(run: Visible, cancel: &AtomicBool, cx: &AsyncApp) -> Result<Value> {
    let started = Instant::now();
    let result = loop {
        let completed = run.completion.borrow_mut().take();
        if let Some(result) = completed {
            break result;
        }
        if let Err(error) = check_cancel(cancel) {
            break Err(error);
        }
        if started.elapsed() > Duration::from_secs(120) {
            break Err(anyhow::anyhow!("visible benchmark timed out"));
        }
        if cx.update(|cx| run.handle.update(cx, |_, _, _| ())).is_err() {
            break Err(anyhow::anyhow!("benchmark window closed before measurement completed"));
        }
        cx.background_executor().timer(Duration::from_millis(100)).await;
    };
    let _ = cx.update(|cx| run.handle.update(cx, |_, window, _| window.remove_window()));
    result
}

fn record(case: Case, result: Result<Value>, output: &Path) -> Result<Value> {
    std::fs::create_dir_all(output)?;
    let mut report = result.unwrap_or_else(|error| json!({"error": format!("{error:#}")}));
    report["case"] = json!(case.id());
    report["label"] = json!(case.label());
    report["profile"] = json!(profile());
    report["os"] = json!(std::env::consts::OS);
    report["arch"] = json!(std::env::consts::ARCH);
    let failed = report.get("error").is_some()
        || report.get("errors").and_then(Value::as_array).is_some_and(|errors| !errors.is_empty());
    if failed {
        report["outcome"] = json!("failed");
    } else if report.get("outcome").is_none() {
        report["outcome"] = json!("completed");
    }
    let mut previews = std::fs::read_dir(output)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect::<Vec<_>>();
    previews.sort();
    report["previews"] = json!(previews);
    std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

fn write_summary(output: &Path, results: &[Value], cancelled: bool) -> Result<()> {
    std::fs::create_dir_all(output)?;
    std::fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&json!({
            "cases": results, "cancelled": cancelled, "profile": profile()
        }))?,
    )?;
    Ok(())
}

pub(super) struct Benchmarks {
    look: Arc<Look>,
    buttons: Vec<Entity<Button>>,
    cancel_button: Entity<Button>,
    reveal: Entity<Button>,
    cancel: Arc<AtomicBool>,
    busy: bool,
    status: String,
    output: Option<PathBuf>,
    results: Vec<Value>,
    quit_on_finish: bool,
}
impl Drop for Benchmarks {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Benchmarks {
    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let buttons = std::iter::once((None, "Run all"))
            .chain(Case::ALL.map(|case| (Some(case), case.label())))
            .enumerate()
            .map(|(index, (case, label))| {
                radix::Button::new(format!("media-benchmark-{index}"))
                    .look(&look)
                    .outline()
                    .label(label)
                    .on_click(cx, move |this: &mut Self, cx| this.start(case, None, cx))
                    .spawn(cx)
            })
            .collect();
        let cancel_button = radix::Button::new("media-benchmark-cancel")
            .look(&look)
            .outline()
            .label("Cancel")
            .on_click(cx, |this: &mut Self, cx| {
                this.cancel.store(true, Ordering::Relaxed);
                this.status = "Cancelling…".into();
                cx.notify();
            })
            .spawn(cx);
        let reveal = radix::Button::new("media-benchmark-reveal")
            .look(&look)
            .outline()
            .label("Reveal results")
            .on_click(cx, |this: &mut Self, cx| {
                if let Some(path) = &this.output {
                    cx.reveal_path(path);
                }
            })
            .spawn(cx);
        Self { look, buttons, cancel_button, reveal, cancel: Arc::new(AtomicBool::new(false)), busy: false,
            status: "Choose a workload. Visible cadence and scrolling open temporary windows; scrolling runs for about 40 seconds after warmup.".into(),
            output: None, results: Vec::new(), quit_on_finish: false }
    }
    fn start(&mut self, case: Option<Case>, output: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let output = match output.map_or_else(
            || tempfile::Builder::new().prefix("xgpui-media-benchmarks-").tempdir().map(|dir| dir.keep()),
            Ok,
        ) {
            Ok(output) => output,
            Err(error) => {
                self.status = format!("Cannot create result folder: {error}");
                cx.notify();
                return;
            }
        };
        self.cancel = Arc::new(AtomicBool::new(false));
        self.busy = true;
        self.results.clear();
        self.output = Some(output.clone());
        self.status = "Preparing media benchmarks…".into();
        for button in &self.buttons {
            button.update(cx, |button, cx| button.set_enabled(false, cx));
        }
        let cases = case.map_or_else(|| Case::ALL.to_vec(), |case| vec![case]);
        let cancel = self.cancel.clone();
        let look = self.look.clone();
        cx.spawn(async move |this, cx| {
            for case in cases {
                if cancel.load(Ordering::Relaxed) { break; }
                if this.update(cx, |this, cx| { this.status = format!("Running {}…", case.label()); cx.notify(); }).is_err() { return; }
                let case_output = output.join(case.id());
                let result = if matches!(case, Case::Cadence | Case::Scroll) {
                    let token = cancel.clone();
                    let prepared = cx.background_spawn(async move { prepare_visible(case, &token) }).await;
                    match prepared {
                        Ok(prepared) => {
                            let opened = this.update(cx, |_, cx| open_visible(prepared, look.clone(), cancel.clone(), cx));
                            match opened { Ok(Ok(run)) => wait_visible(run, &cancel, cx).await, Ok(Err(error)) => Err(error), Err(_) => return }
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    let token = cancel.clone();
                    let path = case_output.clone();
                    cx.background_spawn(async move { run_background(case, &path, &token) }).await
                };
                let saved = cx.background_spawn(async move { record(case, result, &case_output) }).await;
                if this.update(cx, |this, cx| {
                    this.results.push(saved.unwrap_or_else(|error| json!({"case": case.id(), "label": case.label(), "outcome": "failed", "error": format!("Cannot save result: {error:#}")})));
                    if let Err(error) = write_summary(&output, &this.results, cancel.load(Ordering::Relaxed)) {
                        this.results.push(json!({"outcome": "failed", "error": format!("Cannot save summary: {error}")}));
                        cancel.store(true, Ordering::Relaxed);
                    }
                    cx.notify();
                }).is_err() { return; }
            }
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if let Err(error) = write_summary(&output, &this.results, cancel.load(Ordering::Relaxed)) {
                    this.results.push(json!({"outcome": "failed", "error": format!("Cannot save summary: {error:#}")}));
                }
                let failures = this.results.iter().filter(|result| result["outcome"] == "failed").count();
                let unavailable = this.results.iter().filter(|result| result["outcome"] == "unavailable").count();
                this.status = format!("{}: {} workloads, {failures} failures, {unavailable} unavailable.", if cancel.load(Ordering::Relaxed) { "Cancelled" } else { "Finished" }, this.results.len());
                for button in &this.buttons { button.update(cx, |button, cx| button.set_enabled(true, cx)); }
                cx.notify();
                if this.quit_on_finish {

                    if failures > 0 || cancel.load(Ordering::Relaxed) { eprintln!("{}", this.status); std::process::exit(1); }
                    cx.quit();
                }
            });
        }).detach();
        cx.notify();
    }
}
impl Render for Benchmarks {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("media-benchmarks").size_full().overflow_y_scroll().p_4().flex().flex_col().gap_4()
            .text_color(crate::theme::ColorVizLookExt::chrome(self.look.as_ref()).body_text)
            .child("Media benchmarks")
            .child(div().flex().flex_wrap().gap_2().children(self.buttons.clone()).when(self.busy, |row| row.child(self.cancel_button.clone())))
            .child("Upload: six 2048×2048 sources, three upload waves, 120 cached frames. Cadence: 180 baseline intervals, a 12-source burst, 180 cached intervals. Scroll: 24 cards, four 3072×2048 sources, 5s baseline / 30s loading / 5s cached.")
            .child(format!("Build: {}. Timings describe CPU work and render-start cadence, not GPU execution or proof of displayed frames. Keep measurement windows foreground; debug builds are for functional checks.", profile()))
            .child(self.status.clone())
            .when_some(self.output.as_ref(), |root, output| root.child(format!("Results: {}", output.display())).child(self.reveal.clone()))
            .children(self.results.iter().map(|result| {
                let mut row = div().flex().flex_col().gap_2().child(format!("{} · {}", result["label"].as_str().unwrap_or("Workload"), result["outcome"].as_str().unwrap_or("unknown")));
                if let Some(error) = result["error"].as_str() { row = row.child(error.to_owned()); }
                if let Some(errors) = result["errors"].as_array() { for error in errors { row = row.child(error.to_string()); } }
                if let Some(description) = result["description"].as_str() { row = row.child(description.to_owned()); }
                if let Some(value) = result["upload_render_scene_cpu"]["p95_ms"].as_f64() { row = row.child(format!("Upload render CPU p95: {value:.3} ms")); }
                if result.get("baseline").is_some() { row = row.child(format!("Cadence p95: baseline {} ms · loading/burst {} ms · cached {} ms", result["baseline"]["p95_ms"], result.get("loading").unwrap_or(&result["burst"])["p95_ms"], result["cached"]["p95_ms"])); }
                if let Some(pass) = result["provisional_60hz_pass"].as_bool() { row = row.child(format!("Provisional release 60 Hz criteria met: {pass}")); }
                if let Some(previews) = result["previews"].as_array() { for path in previews.iter().filter_map(Value::as_str) { row = row.child(img(PathBuf::from(path)).max_w(px(760.))); } }
                row
            }))
    }
}

pub(crate) fn run_cli(args: &[String]) -> Result<()> {
    let name = args
        .iter()
        .position(|arg| arg == "--media-benchmark")
        .and_then(|index| args.get(index + 1))
        .context("expected --media-benchmark <all|upload|cadence|scroll|atlas|video>")?;
    let case = if name == "all" {
        None
    } else {
        Some(Case::ALL.into_iter().find(|case| case.id() == name).context("unknown Media benchmark")?)
    };
    let output = args
        .iter()
        .position(|arg| arg == "--output")
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from)
        .context("Media benchmarks require --output <directory>")?;
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    gpui_platform::application().with_assets(crate::assets::Assets).run(move |cx| {
        if let Err(error) = gpui_luma::init(cx) {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1100.), px(900.)), cx))),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| Benchmarks::new(crate::theme::default_look(), cx)),
        );
        match result {
            Ok(handle) => {
                let _ = handle.update(cx, |benchmarks, _, cx| {
                    benchmarks.quit_on_finish = true;
                    benchmarks.start(case, Some(output), cx);
                });
            }
            Err(error) => {
                eprintln!("Media benchmark window: {error:#}");
                std::process::exit(1);
            }
        }
        cx.activate(true);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_stops_before_native_work_and_preserves_an_empty_report() {
        let output = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(true);
        assert!(run_background(Case::Upload, output.path(), &cancel).unwrap_err().to_string().contains("cancelled"));
        write_summary(output.path(), &[], true).unwrap();
        let report: Value = serde_json::from_slice(&std::fs::read(output.path().join("report.json")).unwrap()).unwrap();
        assert_eq!(report["cancelled"], true);
        assert!(report["cases"].as_array().unwrap().is_empty());
    }
    #[cfg(feature = "test-support")]
    #[test]
    fn cancelled_queued_run_releases_busy_state_without_opening_a_workload_window() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| Benchmarks::new(crate::theme::default_look(), cx));
        let output = tempfile::tempdir().unwrap();
        let before = cx.update(|_, cx| cx.windows().len());
        page.update(cx, |page, cx| {
            page.start(None, Some(output.path().to_path_buf()), cx);
            page.cancel.store(true, Ordering::Relaxed);
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(!page.read(cx).busy);
            assert_eq!(cx.windows().len(), before);
            assert!(page.read(cx).status.starts_with("Cancelled"));
        });
        assert!(output.path().join("report.json").is_file());
    }
    #[test]
    fn workload_errors_are_failures_and_skipped_platforms_remain_unavailable() {
        let output = tempfile::tempdir().unwrap();
        let report = record(Case::Cadence, Ok(json!({"errors": ["timed out"]})), output.path()).unwrap();
        assert_eq!(report["outcome"], "failed");
        let report = record(Case::Video, Ok(json!({"outcome": "unavailable", "errors": []})), output.path()).unwrap();
        assert_eq!(report["outcome"], "unavailable");
        assert_eq!(
            record(Case::Atlas, Err(anyhow::anyhow!("readback failed")), output.path()).unwrap()["outcome"],
            "failed"
        );
    }
}
