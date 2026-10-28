//! Separate native diagnostic process: every glyph uses GPUI's actual text atlas.
use super::*;
use std::borrow::Cow;

pub fn run(backend: &str, frames: usize) -> Result<()> {
    run_with_output(backend, frames, None)
}
fn run_with_output(backend: &str, frames: usize, output: Option<std::path::PathBuf>) -> Result<()> {
    anyhow::ensure!(matches!(backend, "native" | "parley"), "expected native or parley text backend");
    let platform = gpui_platform::current_platform(false);
    let text: Arc<dyn PlatformTextSystem> = if backend == "parley" {
        Engine::new()?.backend.clone()
    } else {
        platform.text_system()
    };
    let application = Application::with_platform_and_text_system(platform, text).with_assets(crate::assets::Assets);
    application.text_system().add_fonts(vec![
        Cow::Borrowed(samples::PLEX),
        Cow::Borrowed(samples::INTER),
        Cow::Borrowed(paragraph::FONTS[0]),
        Cow::Borrowed(paragraph::FONTS[1]),
    ])?;
    let name = backend.to_owned();
    let benchmark_output = output.clone();
    let report_backend = name.clone();
    application.run(move |cx| {
        if let Err(error) = gpui_luma::init(cx) {
            eprintln!("Luma initialization: {error:#}");
            cx.quit();
            return;
        }
        gpui_luma::focus::bind_default_focus_keys(cx);
        gpui_luma::key_handling::bind_default_control_keys(cx);
        cx.on_action(crate::quit);
        cx.bind_keys([KeyBinding::new("cmd-q", crate::Quit, None)]);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1100.), px(850.)), cx))),
                show: frames > 0 || output.is_none(),
                focus: frames > 0 || output.is_none(),
                ..Default::default()
            },
            move |window, cx| {
                window.set_window_title(&format!("XGPUI Text Atlas · {name}"));
                cx.new(|cx| {
                    let page = NativeText::new(name, frames, cx);
                    if frames > 0 {
                        page.scroll.update(cx, |scroll, _| scroll.set_benchmark_output(output));
                    }
                    page
                })
            },
        );
        if let Err(error) = &result {
            eprintln!("Native text window: {error:#}");
            if benchmark_output.is_some() { std::process::exit(1); }
            cx.quit();
        }
        if frames == 0 && let (Some(output), Ok(handle)) = (benchmark_output, result) {
            let captured = cx.update_window(handle.into(), |_, window, cx| -> Result<()> {
                for _ in 0..5 { window.refresh(); window.draw(cx).clear(cx); }
                window.render_to_image()?.save(output.join("capture.png"))?;
                let report = serde_json::json!({"backend": report_backend, "frames": 5,
                    "checks": "real GPUI text shaping and glyph-atlas capture using the selected application backend", "errors": []});
                std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
                Ok(())
            });
            if let Err(error) = captured.and_then(|result| result) { eprintln!("Text atlas capture: {error:#}"); std::process::exit(1); }
            cx.quit();
        }
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
    Ok(())
}
struct NativeText {
    paragraph: Entity<paragraph::ParagraphLab>,
    scroll: Entity<workload::ScrollLab>,
    buttons: Vec<Entity<Button>>,
    active: usize,
    samples: Vec<samples::Sample>,
    backend: String,
}
impl NativeText {
    fn new(backend: String, frames: usize, cx: &mut Context<Self>) -> Self {
        let look = crate::theme::default_look();
        let engine = Engine::new().expect("registered source fonts");
        let paragraph = cx.new(|cx| {
            let mut page = paragraph::ParagraphLab::new(engine, vec![], look.clone(), cx);
            page.use_native_atlas();
            page
        });
        let scroll = cx.new(|cx| {
            let mut page = workload::ScrollLab::new(
                backend.clone(),
                workload::corpus(5000),
                samples::samples().unwrap().remove(2).font,
                frames,
                frames > 0,
            );
            page.init_controls(look.clone(), cx);
            page
        });
        let buttons = ["Glyph atlas samples", "Editable atlas paragraph", "Virtualized scrolling"]
            .into_iter()
            .enumerate()
            .map(|(id, label)| {
                radix::Button::new(format!("native-text-{id}"))
                    .look(look.as_ref())
                    .outline()
                    .label(label)
                    .on_click(cx, move |this: &mut Self, cx| {
                        this.active = id;
                        cx.notify();
                    })
                    .spawn(cx)
            })
            .collect();
        Self {
            paragraph,
            scroll,
            buttons,
            active: if frames > 0 { 2 } else { 0 },
            samples: samples::samples().unwrap(),
            backend,
        }
    }
}
impl Render for NativeText {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .font_family("IBM Plex Sans")
            .bg(rgb(0x111827))
            .text_color(rgb(0xeff4fc))
            .child(format!("{} backend · real GPUI shaping / glyph atlas · independent process", self.backend))
            .child(div().flex().gap_3().children(self.buttons.clone()));
        match self.active {
            1 => root = root.child(div().flex_1().min_h_0().child(self.paragraph.clone())),
            2 => root = root.child(div().flex_1().min_h_0().child(self.scroll.clone())),
            _ => {
                let rows = div()
                    .id("native-atlas-samples")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .children(self.samples.iter().map(|sample| {
                        div().flex_shrink_0().flex().flex_col().gap_2().child(sample.label.clone()).child(
                            div()
                                .text_size(px(28.))
                                .child(StyledText::new(sample.text.clone()).with_runs(sample.runs())),
                        )
                    }));
                root = root.child(rows);
            }
        }
        root
    }
}

/// Internal entry point used by this app's benchmark panel. No source workers or runtime compilation.
pub fn benchmark(backend: &str, mode: &str, output: &std::path::Path) -> Result<()> {
    anyhow::ensure!(matches!(backend, "native" | "parley"), "unknown text backend");
    std::fs::create_dir_all(output)?;
    match mode {
        "scroll" => run_with_output(backend, 600, Some(output.to_path_buf())),
        "atlas" => run_with_output(backend, 0, Some(output.to_path_buf())),
        "cpu" => {
            let platform = gpui_platform::current_platform(false);
            let native = platform.text_system();
            let parley = if backend == "parley" {
                Some(Engine::new()?)
            } else {
                None
            };
            let text_backend: Arc<dyn PlatformTextSystem> =
                parley.as_ref().map_or_else(|| native.clone(), |engine| engine.backend.clone());
            let system = Arc::new(TextSystem::new(text_backend.clone()));
            system.add_fonts(vec![
                Cow::Borrowed(samples::PLEX),
                Cow::Borrowed(samples::INTER),
                Cow::Borrowed(paragraph::FONTS[0]),
                Cow::Borrowed(paragraph::FONTS[1]),
            ])?;
            let samples = samples::samples()?;
            let lines = timings::measure(system.clone(), &*text_backend, &samples, 20, 2.)?;
            let paragraphs = match paragraph::measure(system.clone(), 20) {
                Ok(timings) => serde_json::json!({"outcome": "completed", "timings": timings}),
                Err(error)
                    if error.root_cause().to_string() == "paragraph layout is unsupported by this text backend" =>
                {
                    serde_json::json!({"outcome": "unavailable", "reason": "This backend provides line shaping and glyph rasterization but does not implement the optional paragraph API."})
                }
                Err(error) => return Err(error),
            };
            let scroll = workload::measure(system, &workload::corpus(5000), &samples[2].font)?;
            let report = serde_json::json!({"backend": backend, "iterations": 20, "lines": lines,
                "sample_labels": samples.iter().map(|sample| &sample.label).collect::<Vec<_>>(),
                "paragraphs": paragraphs, "scroll_corpus_cpu": scroll,
                "measurement": "CPU shaping/cache and direct glyph rasterization; paragraph reflow; 5000-line corpus. Independent backend process; not GPU or UI frame timings.", "errors": []});
            std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
            Ok(())
        }
        _ => anyhow::bail!("unknown text benchmark mode"),
    }
}
