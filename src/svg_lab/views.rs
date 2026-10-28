use super::*;
use gpui::{prelude::*, AnyElement, Bounds, ContentMask, Corners, Render, Window, canvas, div, point, px, rgb};
use xgpui_svg::{svg_image, path_effects::PathEffect};
use crate::theme::ColorVizLookExt;

impl Render for SvgPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_choices(cx);
        self.density = window.scale_factor();
        self.motion_allowed = !cx.reduce_motion();
        self.gallery_columns = ((f32::from(window.viewport_size().width) - 140.) / 232.).floor().max(1.) as usize;
        if self.animate && self.motion_allowed && self.active_tab == "Path Effects" {
            window.request_animation_frame();
        }
        let actions: &[Action] = match self.active_tab.as_ref() {
            "Path Effects" => &[Action::Dash, Action::Animate, Action::Background, Action::Reset],
            "Scheduling & Cache" => &[
                Action::Gallery,
                Action::Builtin,
                Action::Eager,
                Action::Jobs,
                Action::Pool,
                Action::Tiles,
                Action::Budget,
                Action::Background,
            ],
            "Benchmarks" => &[Action::Open, Action::Builtin, Action::Benchmark, Action::Budget],
            "Scaling" => &[
                Action::Open,
                Action::Builtin,
                Action::Zoom1,
                Action::Zoom2,
                Action::Zoom4,
                Action::Zoom8,
                Action::Zoom16,
                Action::DevicePixels,
                Action::ScalingBudget,
                Action::Preview,
                Action::Settle,
                Action::Background,
                Action::Reset,
            ],
            _ => &[Action::Open, Action::Builtin, Action::Clip, Action::Shadow, Action::Background],
        };
        let parameters: &[Parameter] = match self.active_tab.as_ref() {
            "Path Effects" => &[Parameter::Trim, Parameter::Phase, Parameter::Width],
            "Benchmarks" | "Scheduling & Cache" => &[Parameter::Zoom],
            "Scaling" => &[Parameter::Zoom, Parameter::PanX, Parameter::PanY],
            _ => &[],
        };
        let result = match self.active_tab.as_ref() {
            "Path Effects" => self.path_view(),
            "Scheduling & Cache" => self.gallery(cx),
            "Benchmarks" => Ok(self.benchmark_view()),
            "Scaling" => self.scaling(cx),
            _ => self.rendering(cx),
        };
        div()
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if event.pressed_button != Some(gpui::MouseButton::Left) {
                    this.drag_position = None;
                } else if let Some(previous) = this.drag_position.replace(event.position) {
                    this.camera.pan += event.position - previous;
                    this.copy_camera(cx);
                    cx.notify();
                } else {
                    this.drag_position = None;
                }
            }))
            .on_mouse_up(gpui::MouseButton::Left, cx.listener(|this, _, _, _| this.drag_position = None))
            .on_mouse_up_out(gpui::MouseButton::Left, cx.listener(|this, _, _, _| this.drag_position = None))
            .id("svg-lab-scroll")
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .text_color(self.look.chrome().body_text)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .children(
                        self.buttons
                            .iter()
                            .filter(|(action, _)| {
                                actions.contains(action)
                                    && !(matches!(action, Action::Clip | Action::Shadow)
                                        && self.asset.as_ref().is_ok_and(|asset| asset.source.is_some()))
                            })
                            .map(|(action, button)| {
                                let id = format!("svg-action-{action:?}");
                                div().id(id.clone()).debug_selector(move || id.clone()).child(button.clone())
                            }),
                    )
                    .children(
                        self.choices
                            .iter()
                            .filter(|(action, _)| actions.contains(action))
                            .map(|(_, choice)| choice.clone()),
                    ),
            )
            .child(div().flex().flex_wrap().gap_4().children(
                self.sliders.iter().filter(|(parameter, _)| parameters.contains(parameter)).map(
                    |(parameter, slider)| {
                        div()
                            .w(px(190.))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(format!("{}: {:.2}", parameter.config().0, self.values[*parameter as usize]))
                            .child(slider.clone())
                    },
                ),
            ))
            .when(self.active_tab != "Rendering", |view| {
                view.child(format!(
                    "Output budget: {:.2} MiB · Zoom {:.2}× · Display scale {:.2}×",
                    self.policy.max_output_bytes as f64 / MIB as f64,
                    self.values[0],
                    self.density
                ))
            })
            .when(self.loading, |view| view.child("Loading artwork in the background…"))
            .children(
                self.load_error
                    .as_ref()
                    .map(|error| div().child(format!("Load failed; previous artwork retained: {error}"))),
            )
            .child(match result {
                Ok(view) => view,
                Err(error) => div().child(format!("SVG lab unavailable: {error:#}")).into_any_element(),
            })
    }
}

impl SvgPage {
    fn rendering(&self, cx: &mut Context<Self>) -> anyhow::Result<AnyElement> {
        let asset = self.asset.as_ref().map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let camera = camera::Camera { scene_size: asset.logical, ..Default::default() };
        let fixed = asset.fixed.size(0);
        let mut view = div().id("svg-rendering-comparison").debug_selector(||"svg-rendering-comparison".into())
            .flex().flex_col().gap_3()
            .child("Rendering · raster, SVG and native path comparison")
            .child("Fixed 1× artwork size. Compare drawing methods, clipping and shadows here; use Scaling for pan, zoom and raster quality.")
            .child(self.source_label())
            .child(format!("Parse {:.3} ms · Baseline raster {:.3} ms · Clip {} · Shadow {}",asset.parse_ms,asset.raster_ms,self.clipped,self.shadow));
        let mut panels = div().flex().flex_wrap().gap_4();
        for (mode, label) in [
            format!("Fixed raster · {}×{} px", i32::from(fixed.width), i32::from(fixed.height)),
            "SVG renderer · display-density raster".into(),
            "Native GPUI paths".into(),
        ]
        .into_iter()
        .enumerate()
        {
            let panel = div().w(px(320.)).h(px(260.)).flex_shrink_0().overflow_hidden().bg(rgb(if self.dark {
                0x111827
            } else {
                0xf1f5f9
            }));
            let panel = if mode == 1 {
                panel.child(
                    svg_image("svg-rendering-progressive", asset.document.clone())
                        .logical_size(asset.logical)
                        .policy(RasterPolicy::default())
                        .on_status_change(cx.listener(|this, status: &SvgStatus, _, cx| {
                            this.rendering_status = Some(status.clone());
                            cx.notify();
                        })),
                )
            } else if mode == 2 && asset.source.is_some() {
                panel.child("Imported artwork has no automatic SVG-to-native-path conversion")
            } else {
                panel.child(self.texture_or_native(camera, mode == 2))
            };
            panels = panels.child(div().flex_shrink_0().flex().flex_col().gap_2().child(label).child(panel));
        }
        view = view.child(panels);
        if let Some(status) = &self.rendering_status {
            view = view
                .child(format!(
                    "SVG {:?} · Displayed {:?} · Raster {:?} ms",
                    status.phase, status.displayed, status.raster_ms
                ))
                .children(status.error.as_ref().map(|error| div().child(format!("Raster error: {error}"))));
        }
        Ok(view.child("Native paths approximate the built-in SVG's gradients and shadow; this compares capabilities rather than pixel equivalence.").into_any_element())
    }

    pub(super) fn source_label(&self) -> String {
        self.asset
            .as_ref()
            .ok()
            .and_then(|asset| asset.source.as_ref())
            .map_or_else(|| "Built-in ribbon/leaf".to_owned(), |path| path.display().to_string())
    }

    pub(super) fn texture_or_native(&self, camera: camera::Camera, native: bool) -> AnyElement {
        let fixed = self.asset.as_ref().expect("validated SVG asset").fixed.clone();
        let clipped = self.clipped;
        let shadow = self.shadow;
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let origin = camera.origin(bounds);
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    if native {
                        scene::native(origin, camera.zoom, clipped, shadow, window);
                    } else {
                        let image_bounds = Bounds::new(
                            origin,
                            size(px(camera.scene_size.width * camera.zoom), px(camera.scene_size.height * camera.zoom)),
                        );
                        if let Err(error) =
                            window.paint_image(image_bounds, image_bounds, Corners::default(), fixed.clone(), 0, false)
                        {
                            eprintln!("SVG baseline paint: {error:#}");
                        }
                    }
                });
            },
        )
        .size_full()
        .into_any_element()
    }

    fn gallery(&self, cx: &mut Context<Self>) -> anyhow::Result<AnyElement> {
        let asset = self.asset.as_ref().map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let scheduler = self.scheduler.as_ref().map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let stats = scheduler.stats();
        let limits = scheduler.limits();
        let columns = self.gallery_columns;
        let row_height = 210.;
        let viewport_height = 520.;
        let offset = -f32::from(self.gallery_scroll.offset().y);
        let mut rows = Vec::new();
        for first in (0..self.tiles).step_by(columns) {
            let visible = self.eager
                || ((first / columns) as f32 * row_height >= offset - row_height
                    && (first / columns) as f32 * row_height < offset + viewport_height + row_height);
            let mut row = div().h(px(row_height)).flex_shrink_0().flex().gap_3();
            for index in first..(first + columns).min(self.tiles) {
                let (name, document) = if self.gallery_docs.is_empty() {
                    ("Built-in ribbon/leaf", asset.document.clone())
                } else {
                    let (name, doc) = &self.gallery_docs[index % self.gallery_docs.len()];
                    (name.as_str(), doc.clone())
                };
                let mut tile = div().w(px(220.)).flex_shrink_0().flex().flex_col().gap_1().child(name.to_string());
                if visible {
                    tile = tile.child(
                        svg_image(format!("svg-scheduled-{index}"), document)
                            .h(px(180.))
                            .zoom(self.values[0])
                            .policy(self.policy)
                            .scheduler(scheduler.clone())
                            .bg(rgb(if self.dark { 0x111827 } else { 0xf1f5f9 }))
                            .on_status_change(cx.listener(|_, _: &SvgStatus, _, cx| cx.notify())),
                    );
                } else {
                    tile = tile.child(div().h(px(180.)).child("Outside visible + overscan rows"));
                }
                row = row.child(tile);
            }
            rows.push(row);
        }
        Ok(div().flex().flex_col().gap_3()
            .child(format!("{} tiles · {} · {} documents · {} jobs / {} MiB pool", self.tiles, if self.eager {"eager"} else {"visible + overscan"}, self.gallery_docs.len().max(1), limits.max_jobs, limits.max_bytes/MIB))
            .child(format!("Members {} · Active {} · Queued {} · Retained {:.2} MiB · In-flight {:.2} MiB · Peak {:.2} MiB", stats.members, stats.active_jobs, stats.queued_jobs, stats.retained_bytes as f64/MIB as f64, stats.in_flight_bytes as f64/MIB as f64, stats.peak_managed_bytes as f64/MIB as f64))
            .child("Scroll to explore; Command-wheel or pinch to zoom. Open a folder for real artwork; outputs remain independent even when documents are shared.")
            .child(div().id("svg-gallery-scroll").h(px(viewport_height)).flex_shrink_0().overflow_y_scroll().track_scroll(&self.gallery_scroll)
                .on_scroll_wheel(cx.listener(|this,event:&gpui::ScrollWheelEvent,_,cx| {
                    if event.modifiers.platform { let delta=f32::from(event.delta.pixel_delta(px(16.)).y); this.camera.zoom_at(point(px(0.),px(0.)),(-delta*0.005).exp());this.copy_camera(cx);cx.stop_propagation(); } cx.notify();
                }))
                .on_pinch(cx.listener(|this,event:&gpui::PinchEvent,_,cx| {this.zoom_at(point(px(0.),px(0.)),(1.+event.delta).max(0.01),cx);cx.stop_propagation();}))
                .children(rows))
            .child("Budgets account for image reservations, not total RSS or driver VRAM.").into_any_element())
    }

    fn path_view(&self) -> anyhow::Result<AnyElement> {
        let paths = self.paths.as_ref().map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let mut view = div().flex().flex_col().gap_3().child(format!(
            "media-lab path effects · Trim {:.0}% · Dash pattern {} · Phase {:.0}",
            self.effective_trim() * 100.,
            self.dash,
            self.effective_phase()
        ));
        for (index, path) in paths.iter().enumerate() {
            view = view
                .child(format!(
                    "{} · Measured length {:.2}px",
                    ["Open Bézier curve", "Closed heart", "Multiple contours"][index],
                    f32::from(path.length())
                ))
                .child(
                    div().flex().flex_wrap().gap_3().children(
                        [
                            ("Original", PathEffect::new()),
                            ("Trimmed", paths::effect(self.effective_trim(), 0, 0.)?),
                            (
                                "Trimmed + dashed",
                                paths::effect(self.effective_trim(), self.dash, self.effective_phase())?,
                            ),
                        ]
                        .into_iter()
                        .map(|(label, effect)| {
                            div().w(px(300.)).flex().flex_col().gap_2().child(label).child(paths::panel(
                                path,
                                effect,
                                self.values[5],
                                0x70ddc6,
                                170.,
                            ))
                        }),
                    ),
                );
        }
        Ok(view.child("Measurement is retained; trim and dash are applied before native stroke tessellation. Trim uses total length across contours; dash phase restarts per retained contour.").into_any_element())
    }

    fn benchmark_view(&self) -> AnyElement {
        let mut view=div().flex().flex_col().gap_3().child("svg-lab CPU parse/raster observations")
            .child("Runs five bounded parse + raster samples in a background task. Times exclude GPU upload and presentation. These in-process observations are not the standalone lab's isolated-process corpus benchmark or GPU timing.");
        if self.benchmarking {
            view = view.child("CPU samples running…");
        }
        if let Some(result) = &self.bench_result {
            match result {
                Ok(rows) => {
                    view = view.children(rows.iter().cloned().map(|row| div().child(row)));
                }
                Err(error) => view = view.child(format!("Benchmark failed: {error}")),
            }
        }
        view.into_any_element()
    }
}
