//! Luma front end for real, isolated xgpui lab workers. No shell interpolation.
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    process::{Command, Stdio},
    io::{Read, Seek, SeekFrom},
};
use gpui::{prelude::*, *};
use gpui_luma::{controls::button::Button, prelude::HasPresenter};
use gpui_luma_look_radix::{self as radix, Look};

const PORTABLE_VALIDATION_NOTICE: &str = "Source-worker validation is unavailable in this portable archive. Use a source checkout with sibling xgpui and gpui-luma repositories, Rust, and the cached worker dependencies. Interactive labs and their built-in CPU benchmark views remain available.";

fn source_validation_available() -> bool {
    !cfg!(feature = "portable-app")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suite {
    Media,
    Parley,
    Shader,
    Gamut,
    Svg,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Job {
    MediaUpload,
    MediaCadence,
    MediaScroll,
    MediaCapture,
    VideoCapture,
    TextCompare,
    TextScrollNative,
    TextScrollParley,
    TextAtlas,
    PaintCapture,
    MeshCapture,
    WgslCapture,
    GamutTexture,
    GamutInstanced,
    GamutGradient,
    GamutSdr,
    GamutUi,
    SvgSingle,
    SvgCorpus,
}
impl Suite {
    fn jobs(self) -> Vec<(Job, &'static str)> {
        use Job::*;
        match self {
            Self::Media => vec![
                (MediaUpload, "Upload benchmark"),
                (MediaCadence, "Visible cadence report"),
                (MediaScroll, "Sustained scrolling report"),
                (MediaCapture, "Atlas / mipmap capture"),
                (VideoCapture, "Native video validation"),
            ],
            Self::Parley => vec![
                (TextCompare, "Native / Parley CPU comparison"),
                (TextScrollNative, "Native scroll / 600 frames"),
                (TextScrollParley, "Parley scroll / 600 frames"),
                (TextAtlas, "Open real Parley atlas window"),
            ],
            Self::Shader => vec![
                (PaintCapture, "Paint coverage validation"),
                (MeshCapture, "Mesh validation"),
                (WgslCapture, "WGSL validation"),
            ],
            Self::Gamut => vec![
                (GamutTexture, "Wide texture capture"),
                (GamutInstanced, "Wide instanced capture"),
                (GamutGradient, "Wide gradient capture"),
                (GamutSdr, "SDR baseline capture"),
                (GamutUi, "Capture actual controls"),
            ],
            Self::Svg => vec![(SvgSingle, "Isolated selected SVG"), (SvgCorpus, "Isolated corpus benchmark")],
        }
    }
}
pub struct ValidationPage {
    suite: Suite,
    buttons: Vec<(Job, Entity<Button>)>,
    utility: Vec<Entity<Button>>,
    busy: bool,
    cancel: Arc<AtomicBool>,
    output: Option<PathBuf>,
    status: String,
    selected: Option<PathBuf>,
    corpus: Option<PathBuf>,
    last_read: Instant,
    log: String,
}
impl Drop for ValidationPage {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl ValidationPage {
    pub fn new(suite: Suite, look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let buttons = suite
            .jobs()
            .into_iter()
            .map(|(job, label)| {
                (
                    job,
                    radix::Button::new(format!("validation-{job:?}"))
                        .look(look.as_ref())
                        .outline()
                        .label(label)
                        .enabled(source_validation_available())
                        .on_click(cx, move |this: &mut Self, cx| this.start(job, cx))
                        .spawn(cx),
                )
            })
            .collect();
        let utility = ["Cancel worker", "Reveal artifacts", "Choose SVG file…", "Choose corpus folder…"]
            .into_iter()
            .enumerate()
            .map(|(id, label)| {
                radix::Button::new(format!("validation-utility-{id}"))
                    .look(look.as_ref())
                    .outline()
                    .label(label)
                    .on_click(cx, move |this: &mut Self, cx| {
                        match id {
                            0 => this.cancel.store(true, Ordering::Relaxed),
                            1 => {
                                if let Some(path) = &this.output {
                                    cx.reveal_path(path);
                                }
                            }
                            _ => this.choose(id == 3, cx),
                        }
                        cx.notify();
                    })
                    .spawn(cx)
            })
            .collect();
        Self {
            suite,
            buttons,
            utility,
            busy: false,
            cancel: Arc::new(AtomicBool::new(false)),
            output: None,
            status: "Choose a validation workload. Results and raw logs are retained per run.".into(),
            selected: None,
            corpus: None,
            last_read: Instant::now(),
            log: String::new(),
        }
    }
    fn choose(&mut self, directory: bool, cx: &mut Context<Self>) {
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: !directory,
            directories: directory,
            multiple: false,
            prompt: Some(
                if directory {
                    "Choose the corpus matching svg-lab/bench/corpus.json"
                } else {
                    "Choose SVG artwork"
                }
                .into(),
            ),
        });
        cx.spawn(async move |this, cx| {
            let selected = picker.await;
            let _ = this.update(cx, |this, cx| {
                match selected {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            if directory {
                                this.corpus = Some(path);
                            } else {
                                this.selected = Some(path);
                            }
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => this.status = "File selection failed; previous selection retained.".into(),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn start(&mut self, job: Job, cx: &mut Context<Self>) {
        if !source_validation_available() {
            self.status = PORTABLE_VALIDATION_NOTICE.into();
            cx.notify();
            return;
        }
        if self.busy {
            return;
        }
        if job == Job::SvgSingle && self.selected.is_none() {
            self.status = "Choose an SVG file first.".into();
            return;
        }
        if job == Job::SvgCorpus && self.corpus.is_none() {
            self.status =
                "Choose the official corpus folder first; the source runner verifies its manifest hashes.".into();
            return;
        }
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let output =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("artifacts/validation").join(format!("{job:?}-{stamp}"));
        if let Err(error) = std::fs::create_dir_all(&output) {
            self.status = format!("Cannot create artifacts: {error}");
            return;
        }
        self.cancel = Arc::new(AtomicBool::new(false));
        self.busy = true;
        self.output = Some(output.clone());
        self.status = format!(
            "Running {job:?}. Source runners build offline in release; visible cadence/scroll jobs open a diagnostic window."
        );
        self.log.clear();
        let cancel = self.cancel.clone();
        let selected = self.selected.clone();
        let corpus = self.corpus.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { execute(job, &output, selected.as_deref(), corpus.as_deref(), &cancel) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.status = match result {
                    Ok(()) => {
                        "Completed. Inspect the report, capture and worker log below or reveal the artifact folder."
                            .into()
                    }
                    Err(error) => format!("Validation failed or cancelled: {error:#}. Raw logs are retained."),
                };
                this.last_read = Instant::now() - Duration::from_secs(1);
                cx.notify();
            });
        })
        .detach();
    }
}
impl Render for ValidationPage {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if !source_validation_available() {
            return div().id("portable-validation-status").size_full().p_4().child(PORTABLE_VALIDATION_NOTICE);
        }
        if self.last_read.elapsed() > Duration::from_millis(250) {
            if let Some(output) = &self.output {
                self.log = read_tail(&output.join("worker.log"));
            }
            self.last_read = Instant::now();
        }
        if self.busy {
            window.request_animation_frame();
        }
        let mut root=div().id("validation-scroll").size_full().overflow_y_scroll().p_4().flex().flex_col().gap_4()
            .child(format!("{:?} validation · isolated source workloads",self.suite))
            .child(div().flex().flex_wrap().gap_2().children(self.buttons.iter().map(|(_,button)|button.clone())))
            .child(div().flex().gap_2().children(self.utility.iter().take(2).cloned()))
            .child(self.status.clone())
            .child("Workers run serially in their own process group. Cancel stops this run's processes. Reports separate CPU work, submission cadence, memory estimates and captures; none claims GPU execution time.");
        if self.suite == Suite::Svg {
            root=root.child(div().flex().gap_2().children(self.utility.iter().skip(2).cloned()))
                .child(format!("SVG: {:?} · Corpus: {:?}",self.selected,self.corpus))
                .child("Corpus mode runs the original bench/run.py: one fresh process per manifest document/zoom, hash checks, raw JSON and previews. Supply the matching source corpus; it is not downloaded automatically.");
        }
        if let Some(output) = &self.output {
            root = root.child(format!("Artifacts: {}", output.display()));
            if let Ok(entries) = std::fs::read_dir(output) {
                let mut paths = entries.filter_map(Result::ok).map(|e| e.path()).collect::<Vec<_>>();
                paths.sort();
                for path in paths {
                    if path.extension().is_some_and(|ext| ext == "json") {
                        let text = std::fs::read_to_string(&path).unwrap_or_default();
                        root = root.child(format!(
                            "{}\n{}",
                            path.file_name().unwrap_or_default().to_string_lossy(),
                            text.chars().take(16000).collect::<String>()
                        ));
                    } else if path.extension().is_some_and(|ext| ext == "png") {
                        root = root.child(img(path).max_w(px(760.)));
                    }
                }
            }
        }
        root.child(div().text_size(px(12.)).child(self.log.clone()))
    }
}
fn read_tail(path: &Path) -> String {
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let size = file.metadata().map_or(0, |m| m.len());
    let _ = file.seek(SeekFrom::Start(size.saturating_sub(32000)));
    let mut bytes = Vec::new();
    let _ = file.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}
fn cargo() -> PathBuf {
    std::env::var_os("CARGO")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cargo/bin/cargo")))
        .unwrap_or_else(|| "cargo".into())
}
fn run(mut command: Command, output: &Path, cancel: &AtomicBool, finish: Option<&Path>) -> anyhow::Result<()> {
    use std::io::Write;
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(output.join("worker.log"))?;
    writeln!(log, "\nCOMMAND: {command:?}")?;
    command.stdout(Stdio::from(log.try_clone()?)).stderr(Stdio::from(log));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            anyhow::ensure!(status.success(), "worker exited {status}; inspect worker.log");
            return Ok(());
        }
        let reported = finish.is_some_and(Path::is_file);
        if cancel.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(1800) || reported {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGTERM);
            }
            #[cfg(not(unix))]
            {
                let _ = child.kill();
            }
            let stopped = Instant::now();
            while child.try_wait()?.is_none() {
                if stopped.elapsed() > Duration::from_secs(2) {
                    #[cfg(unix)]
                    unsafe {
                        libc::kill(-(child.id() as i32), libc::SIGKILL);
                    }
                    let _ = child.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.wait();
            anyhow::ensure!(reported, "worker cancelled or exceeded 30 minutes");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn execute(
    job: Job,
    output: &Path,
    selected: Option<&Path>,
    corpus: Option<&Path>,
    cancel: &AtomicBool,
) -> anyhow::Result<()> {
    use Job::*;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../xgpui").canonicalize()?;
    if matches!(job, TextAtlas | TextScrollNative | TextScrollParley) {
        let mut command = Command::new(std::env::current_exe()?);
        command.arg(if job == TextScrollNative {
            "--native-text-backend=native"
        } else {
            "--native-text-backend=parley"
        });
        if job != TextAtlas {
            command.arg("--scroll-frames=600");
        }
        return run(command, output, cancel, None);
    }
    let app = match job {
        MediaUpload | MediaCadence | MediaScroll | MediaCapture | VideoCapture => "media-lab",
        TextCompare => "text-lab",
        PaintCapture | MeshCapture | WgslCapture => "paint-lab",
        GamutTexture | GamutInstanced | GamutGradient | GamutSdr | GamutUi => "gamut-lab",
        SvgSingle | SvgCorpus => "svg-lab",
        _ => unreachable!(),
    };
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/source-runners");
    let mut build = Command::new(cargo());
    build
        .current_dir(&repo)
        .env("CARGO_TARGET_DIR", &target)
        .args(["build", "--release", "--offline", "--locked", "--manifest-path"])
        .arg(format!("apps/{app}/Cargo.toml"));
    if app == "paint-lab" {
        build.args(["--features", "portable-shaders"]);
    }
    run(build, output, cancel, None)?;
    let mut command = Command::new(target.join("release").join(format!("{app}{}", std::env::consts::EXE_SUFFIX)));
    command.current_dir(&repo);
    let png = output.join("capture.png");
    let report = output.join("report.json");
    let mut finish = None;
    match job {
        MediaUpload => {
            command.args(["--headless", "--upload-bench", "--bench-report"]).arg(&report).arg("--png").arg(&png);
        }
        MediaCadence => {
            command.arg("--ui-perf-report").arg(&report);
            finish = Some(report.as_path());
        }
        MediaScroll => {
            command.arg("--scroll-perf-report").arg(&report);
        }
        MediaCapture => {
            command.args(["--headless", "--png"]).arg(&png);
        }
        VideoCapture => {
            command.args(["--headless", "--video", "--png"]).arg(&png);
        }
        TextCompare => {
            command.args(["--backend", "both", "--timings-only", "--iterations", "20"]);
        }
        PaintCapture | MeshCapture | WgslCapture => {
            command.args(["--headless", "--frames", "5", "--png"]).arg(&png);
            if job == MeshCapture {
                command.args(["--effect", "mesh"]);
            }
            if job == WgslCapture {
                command.arg("--portable-shader");
            }
        }
        GamutTexture | GamutInstanced | GamutGradient | GamutSdr => {
            command.args(["--headless", "--frames", "5", "--png"]).arg(&png);
            match job {
                GamutInstanced => {
                    command.arg("--instanced");
                }
                GamutGradient => {
                    command.args(["--instanced", "--gradient"]);
                }
                GamutSdr => {
                    command.arg("--sdr");
                }
                _ => {}
            }
        }
        GamutUi => {
            command.arg("--ui-png").arg(&png);
            finish = Some(png.as_path());
        }
        SvgSingle => {
            command
                .arg("--benchmark")
                .arg(selected.unwrap())
                .args(["--zoom", "4", "--density", "2", "--iterations", "5", "--budget-mib", "16", "--preview"])
                .arg(&png);
        }
        SvgCorpus => {
            let binary = command.get_program().to_owned();
            command = Command::new("python3");
            command
                .current_dir(&repo)
                .arg(repo.join("apps/svg-lab/bench/run.py"))
                .arg("--binary")
                .arg(binary)
                .arg("--corpus")
                .arg(corpus.unwrap())
                .arg("--output")
                .arg(output)
                .arg("--previews")
                .arg(output.join("previews"));
        }
        _ => unreachable!(),
    }
    run(command, output, cancel, finish)?;
    verify_reports(output)
}

fn verify_reports(output: &Path) -> anyhow::Result<()> {
    for name in ["report.json", "raw.json"] {
        let path = output.join(name);
        if !path.is_file() {
            continue;
        }
        let report: serde_json::Value = serde_json::from_reader(std::fs::File::open(&path)?)?;
        if let Some(errors) = report.get("errors").and_then(|v| v.as_array()) {
            anyhow::ensure!(errors.is_empty(), "{name} reports {} workload errors", errors.len());
        }
        if let Some(cases) = report.get("cases").and_then(|v| v.as_array()) {
            let failures = cases
                .iter()
                .filter(|case| {
                    case.get("error").is_some()
                        || case.get("exit_code").and_then(|v| v.as_i64()).is_some_and(|code| code != 0)
                })
                .count();
            anyhow::ensure!(failures == 0, "{name} reports {failures} failed cases");
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(feature = "portable-app", feature = "test-support"))]
    #[::core::prelude::v1::test]
    fn portable_app_does_not_start_source_workers_or_create_source_artifacts() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| ValidationPage::new(Suite::Svg, crate::theme::default_look(), cx));
        page.update(cx, |page, cx| {
            page.start(Job::SvgSingle, cx);
            assert!(!page.busy);
            assert!(page.output.is_none());
            assert_eq!(page.status, PORTABLE_VALIDATION_NOTICE);
        });
        cx.run_until_parked();
    }
    #[::core::prelude::v1::test]
    fn worker_report_failures_are_not_presented_as_success() {
        let output = tempfile::tempdir().unwrap();
        std::fs::write(output.path().join("report.json"), r#"{"errors":["visible run timed out"]}"#).unwrap();
        assert!(verify_reports(output.path()).is_err());
        std::fs::write(output.path().join("report.json"), r#"{"errors":[]}"#).unwrap();
        std::fs::write(output.path().join("raw.json"), r#"{"cases":[{"exit_code":0},{"error":"timed out"}]}"#).unwrap();
        assert!(verify_reports(output.path()).is_err());
        std::fs::write(output.path().join("raw.json"), r#"{"cases":[{"exit_code":0}]}"#).unwrap();
        verify_reports(output.path()).unwrap();
    }
    // Reuse the Rust test executable instead of an OS-specific sleep command.
    #[::core::prelude::v1::test]
    fn cancellation_fixture_child() {
        if std::env::var_os("XGPUI_CANCELLATION_FIXTURE").as_deref() == Some(std::ffi::OsStr::new("1")) {
            std::thread::sleep(Duration::from_secs(30));
        }
    }
    #[::core::prelude::v1::test]
    fn cancelled_worker_is_reaped() {
        let output = tempfile::tempdir().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "validation::tests::cancellation_fixture_child", "--nocapture"]);
        command.env("XGPUI_CANCELLATION_FIXTURE", "1");
        let started = Instant::now();
        assert!(run(command, output.path(), &AtomicBool::new(true), None).is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
