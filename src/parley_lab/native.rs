//! Separate native diagnostic process: every glyph uses GPUI's actual text atlas.
use super::*;
use std::borrow::Cow;

pub fn run(backend: &str, frames: usize) -> Result<()> {
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
                ..Default::default()
            },
            move |window, cx| {
                window.set_window_title(&format!("XGPUI Text Atlas · {name}"));
                cx.new(|cx| NativeText::new(name, frames, cx))
            },
        );
        if let Err(error) = result {
            eprintln!("Native text window: {error:#}");
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
