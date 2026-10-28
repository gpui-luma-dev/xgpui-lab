//! The xgpui text-lab corpus, diagnostics and editable paragraphs in the unified shell.
mod samples;
mod timings;
mod paragraph;
mod raster;
pub(crate) mod native;
mod workload;
mod line;

use std::{sync::Arc};
use anyhow::Result;
use gpui::{prelude::*, *};
use gpui_luma::{
    prelude::HasPresenter,
    controls::{button::Button, slider::Slider},
};
use gpui_luma_look_radix::{self as radix, Look};
use crate::theme::ColorVizLookExt;

struct Engine {
    backend: Arc<xgpui_parley::ParleyTextSystem>,
    system: Arc<TextSystem>,
    cache: WindowTextSystem,
}
impl Engine {
    fn new() -> Result<Arc<Self>> {
        let mut fonts = parley::FontContext::new();
        for data in [samples::PLEX, samples::INTER, paragraph::FONTS[0], paragraph::FONTS[1]] {
            fonts.collection.register_fonts(parley::fontique::Blob::new(Arc::new(data.to_vec())), None);
        }
        let backend = Arc::new(xgpui_parley::ParleyTextSystem::new(fonts, "IBM Plex Sans")?);
        let system = Arc::new(TextSystem::new(backend.clone()));
        let cache = WindowTextSystem::new(system.clone());
        Ok(Arc::new(Self { backend, system, cache }))
    }
}
struct Row {
    label: String,
    text: String,
    font: Font,
    width: f32,
    height: f32,
    image: Arc<RenderImage>,
    details: String,
    faces: usize,
}

pub struct ParleyPage {
    pub active_tab: SharedString,
    look: Arc<Look>,
    engine: Result<Arc<Engine>>,
    paragraph: Option<Entity<paragraph::ParagraphLab>>,
    line: Option<Entity<line::LinePage>>,
    scroll: Entity<workload::ScrollLab>,
    children: Vec<std::process::Child>,
    child_error: Option<String>,
    scroll_report: Option<Result<String>>,
    buttons: Vec<Entity<Button>>,
    sliders: Vec<Slider>,
    font_size: f32,
    raster_scale: f32,
    rows: Option<Result<Vec<Row>>>,
    report: Option<Result<Vec<timings::Timing>>>,
    measuring: bool,
}
impl ParleyPage {
    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let engine = Engine::new();
        // Native reference rows use the same registered source fixtures.
        let _ = cx
            .text_system()
            .add_fonts(vec![std::borrow::Cow::Borrowed(samples::INTER), std::borrow::Cow::Borrowed(samples::PLEX)]);
        let paragraph = engine.as_ref().ok().map(|engine| {
            let timings = Vec::new();
            cx.new(|cx| paragraph::ParagraphLab::new(engine.clone(), timings, look.clone(), cx))
        });
        let line = engine
            .as_ref()
            .ok()
            .map(|engine| cx.new(|cx| line::LinePage::new(engine.clone(), look.clone(), cx)));
        let scroll = cx.new(|cx| {
            let mut page = workload::ScrollLab::new(
                "Shell native backend".into(),
                workload::corpus(5000),
                samples::samples().unwrap().remove(2).font,
                0,
                false,
            );
            page.init_controls(look.clone(), cx);
            page
        });
        let buttons = [
            "Run CPU diagnostics",
            "Reset preview",
            "Open Parley glyph atlas",
            "Open native glyph atlas",
            "Measure scroll corpus CPU",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            radix::Button::new(format!("parley-action-{index}"))
                .look(look.as_ref())
                .outline()
                .label(label)
                .on_click(cx, move |this: &mut Self, cx| {
                    if index == 0 {
                        this.measure(cx);
                    } else if index == 2 || index == 3 {
                        match std::env::current_exe().and_then(|exe| {
                            std::process::Command::new(exe)
                                .arg(if index == 2 {
                                    "--native-text-backend=parley"
                                } else {
                                    "--native-text-backend=native"
                                })
                                .spawn()
                        }) {
                            Ok(child) => {
                                this.children.push(child);
                                this.child_error = None;
                            }
                            Err(error) => this.child_error = Some(error.to_string()),
                        }
                    } else if index == 4 {
                        this.measure_scroll(cx);
                    } else {
                        this.font_size = 28.;
                        this.raster_scale = 2.;
                        this.rows = None;
                        for (index, slider) in this.sliders.iter().enumerate() {
                            slider.update(cx, |slider, cx| slider.set_value(if index == 0 { 28. } else { 2. }, cx));
                        }
                    }
                    cx.notify();
                })
                .spawn(cx)
        })
        .collect();
        let sliders = [false, true]
            .into_iter()
            .map(|scale| {
                radix::Slider::new(if scale {
                    "parley-raster-scale"
                } else {
                    "parley-font-size"
                })
                .look(look.as_ref())
                .size(radix::SliderSize::One)
                .range(if scale { 1.0..4.0 } else { 12.0..64.0 })
                .step(if scale { 0.25 } else { 1. })
                .value(if scale { 2. } else { 28. })
                .on_value_changed(cx, move |this: &mut Self, _, value, cx| {
                    if scale {
                        this.raster_scale = value;
                    } else {
                        this.font_size = value;
                    }
                    this.rows = None;
                    cx.notify();
                })
                .spawn(cx)
            })
            .collect();
        Self {
            active_tab: "Text Shaping".into(),
            look,
            engine,
            paragraph,
            line,
            scroll,
            children: vec![],
            child_error: None,
            scroll_report: None,
            buttons,
            sliders,
            font_size: 28.,
            raster_scale: 2.,
            rows: None,
            report: None,
            measuring: false,
        }
    }
    pub fn activate(&mut self, tab: SharedString, cx: &mut Context<Self>) {
        if let Some(paragraph) = &self.paragraph {
            paragraph.update(cx, |page, cx| {
                page.set_mode(tab == "Line Layout");
                cx.notify();
            });
        }
        self.active_tab = tab;
        cx.notify();
    }
    fn make_rows(&self) -> Result<Vec<Row>> {
        let engine = self.engine.as_ref().map_err(|e| anyhow::anyhow!("{e:#}"))?;
        samples::samples()?
            .into_iter()
            .map(|sample| {
                let line = engine.cache.layout_line(&sample.text, px(self.font_size), &sample.runs(), None);
                let height = f32::from(line.ascent + line.descent).ceil() + 4.;
                let width = f32::from(line.width).ceil() + 4.;
                let image = raster::render(
                    &*engine.backend,
                    &line.runs,
                    px(self.font_size),
                    size(px(width), px(height)),
                    self.raster_scale,
                    f32::from(line.ascent) + 2.,
                )?;
                let details = line
                    .runs
                    .iter()
                    .map(|run| {
                        format!(
                            "face {:?}: {} glyphs, byte clusters {:?}",
                            run.font_id,
                            run.glyphs.len(),
                            run.glyphs.iter().map(|g| g.index).collect::<Vec<_>>()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" · ");
                let faces = line.runs.iter().map(|run| run.font_id).collect::<std::collections::HashSet<_>>().len();
                Ok(Row {
                    faces,
                    label: sample.label,
                    text: sample.text,
                    font: sample.font,
                    width,
                    height,
                    image,
                    details,
                })
            })
            .collect()
    }
    fn measure(&mut self, cx: &mut Context<Self>) {
        if self.measuring {
            return;
        }
        self.measuring = true;
        let scale = self.raster_scale;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let engine = Engine::new()?;
                    let rows = samples::samples()?;
                    let line = timings::measure(engine.system.clone(), &*engine.backend, &rows, 20, scale)?;
                    let paragraph = paragraph::measure(engine.system.clone(), 20)?;
                    Ok::<_, anyhow::Error>((line, paragraph))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.measuring = false;
                this.report = Some(match result {
                    Ok((line, timings)) => {
                        if let Some(paragraph) = &this.paragraph {
                            paragraph.update(cx, |page, cx| {
                                page.set_timings(timings);
                                cx.notify();
                            });
                        }
                        Ok(line)
                    }
                    Err(error) => Err(error),
                });
                cx.notify();
            });
        })
        .detach();
    }
}
impl ParleyPage {
    fn measure_scroll(&mut self, cx: &mut Context<Self>) {
        if self.measuring {
            return;
        }
        self.measuring = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let engine = Engine::new()?;
                    workload::measure(
                        engine.system.clone(),
                        &workload::corpus(5000),
                        &samples::samples()?.remove(2).font,
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.measuring = false;
                this.scroll_report = Some(result);
                cx.notify();
            });
        })
        .detach();
    }
}
impl Drop for ParleyPage {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Render for ParleyPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if self.active_tab == "Scrolling" {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(div().flex().gap_3().children(self.buttons.iter().skip(2).cloned()))
                .children(self.scroll_report.as_ref().map(|report| {
                    div().child(match report {
                        Ok(text) => text.clone(),
                        Err(error) => format!("Scroll CPU diagnostic: {error:#}"),
                    })
                }))
                .child(div().flex_1().min_h_0().child(self.scroll.clone()))
                .into_any_element();
        }
        if self.active_tab == "Line Layout" {
            return div().size_full().children(self.line.clone()).into_any_element();
        }
        if self.active_tab == "Paragraphs" {
            return match &self.paragraph {
                Some(page) => div().size_full().child(page.clone()).into_any_element(),
                None => div()
                    .p_4()
                    .child(format!("Parley initialization failed: {:?}", self.engine.as_ref().err()))
                    .into_any_element(),
            };
        }
        self.children.retain_mut(|child| child.try_wait().ok().flatten().is_none());
        if self.rows.is_none() {
            self.rows = Some(self.make_rows());
        }
        let mut root = div()
            .id("parley-lab-scroll")
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .text_color(self.look.chrome().body_text)
            .child(div().flex().flex_wrap().gap_3().children(self.buttons.iter().take(4).cloned()))
            .children(
                self.child_error.as_ref().map(|error| div().child(format!("Native atlas window failed: {error}"))),
            )
            .child(
                div().flex().gap_4().children(
                    self.sliders
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| *index == 0 || self.active_tab == "Rasterization")
                        .map(|(index, slider)| {
                            div()
                                .w(px(230.))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(if index == 0 {
                                    format!("Font size: {:.0}px", self.font_size)
                                } else {
                                    format!("Raster scale: {:.2}×", self.raster_scale)
                                })
                                .child(slider.clone())
                        }),
                ),
            )
            .child("Parley shaping and outline rasterization · isolated backend · text-lab font fixtures")
            .when(self.measuring, |root| root.child("Running CPU diagnostics…"));
        match self.rows.as_ref().unwrap() {
            Err(error) => root = root.child(format!("Text lab unavailable: {error:#}")),
            Ok(rows) => {
                for (index, row) in rows.iter().enumerate() {
                    if self.active_tab == "Fonts & Fallback" && index < 2 {
                        continue;
                    }
                    let mut panel = div().flex_shrink_0().flex().flex_col().gap_2().child(row.label.clone());
                    if self.active_tab == "Text Shaping" {
                        panel = panel
                            .child(format!("Source: {}", row.text))
                            .child(format!("Width {:.2}px · {} resolved faces", row.width - 4., row.faces))
                            .child(row.details.clone());
                    } else if self.active_tab == "Fonts & Fallback" {
                        panel = panel
                            .child(format!(
                                "Requested {} · weight {} · fallback {:?}",
                                row.font.family, row.font.weight.0, row.font.fallbacks
                            ))
                            .child(format!("Resolved {} face(s) · {}", row.faces, row.details))
                            .child(
                                div()
                                    .bg(rgb(0xffffff))
                                    .p_3()
                                    .child(img(row.image.clone()).w(px(row.width)).h(px(row.height))),
                            )
                            .child(
                                div()
                                    .text_size(px(self.font_size))
                                    .font(row.font.clone())
                                    .child(format!("Native reference: {}", row.text)),
                            );
                    } else {
                        let pixels = row.image.size(0);
                        let coverage = row
                            .image
                            .as_bytes(0)
                            .unwrap()
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .filter(|pixel| pixel[3] > 0)
                            .count();
                        panel=panel.child(div().bg(rgb(0xffffff)).p_3().child(img(row.image.clone()).w(px(row.width)).h(px(row.height))))
                            .child(format!("CPU mask composite {}×{} device px · {} covered pixels · {} bytes",i32::from(pixels.width),i32::from(pixels.height),coverage,row.image.as_bytes(0).unwrap().len()))
                            .child("CPU mask inspection. Open the Parley atlas window above to exercise the real GPUI glyph atlas, native paragraph paint and selection.");
                    }
                    if let Some(Ok(report)) = &self.report {
                        let t = &report[index];
                        panel=panel.child(format!("28px diagnostic · layout first/warm {:.1}/{:.2} µs · CPU raster first/warm {:.1}/{:.1} µs · width {:.2}px · {} faces · {} unique glyphs",t.layout_first,t.layout_warm,t.raster_first,t.raster_warm,t.width,t.faces,t.glyphs));
                    }
                    root = root.child(panel);
                }
            }
        }
        if let Some(Err(error)) = &self.report {
            root = root.child(format!("Diagnostic failed: {error:#}"));
        }
        root.child("CPU diagnostics: 20 warm iterations, median; exact GPUI layout cache hits and direct glyph rebuilds. Excludes frame/GPU timing. Native rows use the shell's existing backend.").into_any_element()
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::{ParleyPage, samples};
    use gpui::TestAppContext;
    use gpui_luma::controls::button::ButtonEvent;

    #[test]
    fn parley_tabs_render_actual_masks_and_luma_diagnostics() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| ParleyPage::new(crate::theme::default_look(), cx));
        for tab in ["Text Shaping", "Line Layout", "Paragraphs", "Fonts & Fallback", "Rasterization"] {
            page.update(cx, |page, cx| page.activate(tab.into(), cx));
            cx.run_until_parked();
        }
        cx.update(|_, cx| {
            let page = page.read(cx);
            let rows = page.rows.as_ref().unwrap().as_ref().unwrap();
            assert_eq!(rows.len(), samples::samples().unwrap().len());
            assert!(rows[2].faces > 1, "fallback sample must actually resolve multiple faces");
            for row in rows {
                assert!(
                    row.image.as_bytes(0).unwrap().as_chunks::<4>().0.iter().any(|pixel| pixel[3] > 0),
                    "{} must contain visible glyphs",
                    row.label
                );
            }
        });
        let run = cx.update(|_, cx| page.read(cx).buttons[0].clone());
        run.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert!(!page.measuring);
            let report = page.report.as_ref().unwrap().as_ref().unwrap();
            assert_eq!(report.len(), 6);
            assert!(report.iter().all(|row| row.glyphs > 0 && row.width > 0.));
        });
    }
}
