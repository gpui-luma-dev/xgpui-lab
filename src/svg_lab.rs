//! Embedded svg-lab scene, progressive scheduling, and media-lab path effects.
mod scene;
mod paths;
mod views;
mod camera;
mod imports;
mod scaling;

use std::{sync::Arc, time::Instant};
use gpui::{AppContext, Context, Entity, RenderImage, SharedString, size};
use gpui_luma::{
    controls::{button::Button, slider::Slider},
    prelude::HasPresenter,
};
use gpui_luma_look_radix::{self as radix, Look};
use xgpui_svg::{SvgDocument, SvgStatus, SvgScheduler, SchedulerLimits, RasterPolicy, MIB, path_effects::MeasuredPath};

struct Asset {
    document: SvgDocument,
    fixed: Arc<RenderImage>,
    parse_ms: f64,
    raster_ms: f64,
    logical: gpui::Size<f32>,
    bytes: Vec<u8>,
    source: Option<std::path::PathBuf>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Clip,
    Shadow,
    Background,
    Budget,
    Jobs,
    Pool,
    Tiles,
    Dash,
    Reset,
    Benchmark,
    Open,
    Builtin,
    Gallery,
    Eager,
    Animate,
    Zoom1,
    Zoom2,
    Zoom4,
    Zoom8,
    Zoom16,
    DevicePixels,
    ScalingBudget,
    Preview,
    Settle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parameter {
    Zoom,
    PanX,
    PanY,
    Trim,
    Phase,
    Width,
}
impl Parameter {
    fn config(self) -> (&'static str, f32, f32, f32) {
        match self {
            Self::Zoom => ("Zoom", 0.25, 16., 1.),
            Self::PanX => ("Pan X", -600., 600., 0.),
            Self::PanY => ("Pan Y", -400., 400., 0.),
            Self::Trim => ("Trim", 0., 1., 0.68),
            Self::Phase => ("Dash phase", 0., 80., 0.),
            Self::Width => ("Stroke width", 2., 16., 4.),
        }
    }
}
pub struct SvgPage {
    look: Arc<Look>,
    active_tab: SharedString,
    asset: anyhow::Result<Asset>,
    status: Option<SvgStatus>,
    rendering_status: Option<SvgStatus>,
    policy: RasterPolicy,
    scheduler: anyhow::Result<SvgScheduler>,
    jobs: usize,
    pool_index: usize,
    tiles: usize,
    clipped: bool,
    shadow: bool,
    dark: bool,
    dash: usize,
    values: [f32; 6],
    paths: anyhow::Result<Vec<MeasuredPath>>,
    buttons: Vec<(Action, Entity<Button>)>,
    sliders: Vec<(Parameter, Slider)>,
    benchmarking: bool,
    bench_result: Option<Result<Vec<String>, String>>,
    density: f32,
    camera: camera::Camera,
    panel_bounds: [gpui::Bounds<gpui::Pixels>; 3],
    drag_position: Option<gpui::Point<gpui::Pixels>>,
    animate: bool,
    motion_allowed: bool,
    started: Instant,
    load_request: u64,
    loading: bool,
    load_error: Option<String>,
    gallery_docs: Vec<(String, SvgDocument)>,
    gallery_scroll: gpui::ScrollHandle,
    gallery_columns: usize,
    eager: bool,
}
impl SvgPage {
    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let buttons = [
            (Action::Zoom1, "1×"),
            (Action::Zoom2, "2×"),
            (Action::Zoom4, "4×"),
            (Action::Zoom8, "8×"),
            (Action::Zoom16, "16×"),
            (Action::DevicePixels, "1 source px / device px"),
            (Action::ScalingBudget, "Next scaling budget"),
            (Action::Preview, "Next preview resolution"),
            (Action::Settle, "Next settle delay"),
            (Action::Open, "Open SVG…"),
            (Action::Builtin, "Built-in scene"),
            (Action::Gallery, "Open gallery folder…"),
            (Action::Eager, "Toggle visible / eager"),
            (Action::Animate, "Animate / Pause paths"),
            (Action::Clip, "Toggle SVG clipping"),
            (Action::Shadow, "Toggle SVG shadow"),
            (Action::Background, "Toggle backdrop"),
            (Action::Budget, "Next output budget"),
            (Action::Jobs, "Next job limit"),
            (Action::Pool, "Next pool budget"),
            (Action::Tiles, "Next gallery size"),
            (Action::Dash, "Next dash pattern"),
            (Action::Reset, "Reset view"),
            (Action::Benchmark, "Run 5 CPU samples"),
        ]
        .map(|(action, label)| {
            (
                action,
                radix::Button::new(format!("svg-lab-{action:?}"))
                    .look(look.as_ref())
                    .outline()
                    .size(radix::ButtonSize::One)
                    .label(label)
                    .on_click(cx, move |this: &mut Self, cx| this.action(action, cx))
                    .spawn(cx),
            )
        })
        .to_vec();
        let sliders = [
            Parameter::Zoom,
            Parameter::PanX,
            Parameter::PanY,
            Parameter::Trim,
            Parameter::Phase,
            Parameter::Width,
        ]
        .map(|parameter| {
            let (_, min, max, value) = parameter.config();
            (
                parameter,
                radix::Slider::new(format!("svg-lab-{parameter:?}"))
                    .look(look.as_ref())
                    .size(radix::SliderSize::One)
                    .range(min..max)
                    .value(value as f64)
                    .step(if matches!(parameter, Parameter::Zoom | Parameter::Trim) {
                        0.01
                    } else {
                        1.
                    })
                    .on_value_changed(cx, move |this: &mut Self, _, value, cx| {
                        if parameter == Parameter::Zoom {
                            this.camera.zoom_at(gpui::point(gpui::px(0.), gpui::px(0.)), value / this.camera.zoom);
                            this.copy_camera(cx);
                        } else {
                            this.values[parameter as usize] = value;
                            this.camera.pan = gpui::point(gpui::px(this.values[1]), gpui::px(this.values[2]));
                        }
                        cx.notify();
                    })
                    .spawn(cx),
            )
        })
        .to_vec();
        Self {
            look,
            active_tab: "Rendering".into(),
            asset: Self::build(false, true, cx),
            status: None,
            rendering_status: None,
            policy: Default::default(),
            scheduler: SvgScheduler::new(SchedulerLimits { max_jobs: 2, max_bytes: 64 * MIB }),
            jobs: 2,
            pool_index: 2,
            tiles: 24,
            clipped: false,
            shadow: true,
            dark: true,
            dash: 1,
            values: [1., 0., 0., 0.68, 0., 4.],
            paths: paths::samples(),
            buttons,
            sliders,
            benchmarking: false,
            bench_result: None,
            density: 1.,
            camera: Default::default(),
            panel_bounds: [Default::default(); 3],
            drag_position: None,
            animate: false,
            motion_allowed: true,
            started: Instant::now(),
            load_request: 0,
            loading: false,
            load_error: None,
            gallery_docs: vec![],
            gallery_scroll: gpui::ScrollHandle::new(),
            gallery_columns: 3,
            eager: false,
        }
    }
    fn build(clipped: bool, shadow: bool, cx: &Context<Self>) -> anyhow::Result<Asset> {
        Self::from_bytes(&cx.svg_renderer(), scene::svg(clipped, shadow).into_bytes(), None)
    }

    pub fn activate(&mut self, tab: SharedString, cx: &mut Context<Self>) {
        self.drag_position = None;
        self.active_tab = tab;
        cx.notify();
    }
    fn action(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::Zoom1 | Action::Zoom2 | Action::Zoom4 | Action::Zoom8 | Action::Zoom16 | Action::DevicePixels => {
                let zoom = match action {
                    Action::Zoom2 => 2.,
                    Action::Zoom4 => 4.,
                    Action::Zoom8 => 8.,
                    Action::Zoom16 => 16.,
                    Action::DevicePixels => 1. / self.density,
                    _ => 1.,
                };
                self.zoom_at(gpui::point(gpui::px(0.), gpui::px(0.)), zoom / self.camera.zoom, cx);
            }
            Action::ScalingBudget => {
                let budgets = [MIB / 4, MIB, 4 * MIB, 16 * MIB, 32 * MIB];
                let index = budgets.iter().position(|budget| *budget == self.policy.max_output_bytes).unwrap_or(3);
                self.policy.max_output_bytes = budgets[(index + 1) % budgets.len()];
            }
            Action::Preview => {
                self.policy.preview_edge = match self.policy.preview_edge {
                    128 => 320,
                    320 => 640,
                    _ => 128,
                }
            }
            Action::Settle => {
                self.policy.settle_delay =
                    std::time::Duration::from_millis(match self.policy.settle_delay.as_millis() {
                        50 => 120,
                        120 => 300,
                        _ => 50,
                    })
            }
            Action::Open => self.open_svg(cx),
            Action::Gallery => self.open_gallery(cx),
            Action::Eager => self.eager = !self.eager,
            Action::Builtin => {
                self.load_request += 1;
                self.loading = false;
                self.load_error = None;
                self.asset = Self::build(self.clipped, self.shadow, cx);
                self.status = None;
                self.rendering_status = None;
                self.camera = Default::default();
                self.copy_camera(cx);
                self.gallery_docs.clear();
            }
            Action::Animate => {
                if self.animate {
                    self.values[3] = self.effective_trim();
                    self.values[4] = self.effective_phase().rem_euclid(80.);
                }
                self.animate = !self.animate;
                self.started = Instant::now();
            }
            Action::Clip | Action::Shadow => {
                if self.asset.as_ref().is_ok_and(|asset| asset.source.is_some()) {
                    return;
                }
                if action == Action::Clip {
                    self.clipped = !self.clipped;
                } else {
                    self.shadow = !self.shadow;
                }
                self.load_request += 1;
                self.loading = false;
                self.asset = Self::build(self.clipped, self.shadow, cx);
                self.camera.scene_size = gpui::size(320., 240.);
                self.status = None;
                self.rendering_status = None;
            }
            Action::Background => self.dark = !self.dark,
            Action::Budget => {
                self.policy.max_output_bytes = if self.policy.max_output_bytes == 16 * MIB {
                    32 * MIB
                } else {
                    16 * MIB
                }
            }
            Action::Jobs | Action::Pool => {
                if action == Action::Jobs {
                    self.jobs = if self.jobs == 4 { 1 } else { self.jobs * 2 };
                } else {
                    self.pool_index = (self.pool_index + 1) % 3;
                }
                self.scheduler = SvgScheduler::new(SchedulerLimits {
                    max_jobs: self.jobs,
                    max_bytes: [16, 32, 64][self.pool_index] * MIB,
                });
            }
            Action::Tiles => {
                self.tiles = match self.tiles {
                    6 => 12,
                    12 => 24,
                    24 => 48,
                    48 => 96,
                    _ => 6,
                }
            }
            Action::Dash => self.dash = (self.dash + 1) % 4,
            Action::Reset => {
                if self.active_tab == "Path Effects" {
                    self.animate = false;
                    for (parameter, slider) in self.sliders.iter().skip(3) {
                        self.values[*parameter as usize] = parameter.config().3;
                        slider.update(cx, |slider, cx| slider.set_value(parameter.config().3, cx));
                    }
                } else {
                    self.camera.zoom = 1.;
                    self.camera.pan = gpui::point(gpui::px(0.), gpui::px(0.));
                    self.policy = RasterPolicy::default();
                    self.copy_camera(cx);
                }
            }
            Action::Benchmark => self.benchmark(cx),
        }
        cx.notify();
    }
    fn benchmark(&mut self, cx: &mut Context<Self>) {
        if self.benchmarking {
            return;
        }
        let renderer = cx.svg_renderer();
        let source = match &self.asset {
            Ok(asset) => asset.bytes.clone(),
            Err(_) => return,
        };
        self.benchmarking = true;
        let policy = self.policy;
        let logical = self.camera.scene_size;
        let zoom = self.values[0];
        let density = self.density;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut rows = Vec::new();
                    let resolution = policy.resolution(logical, zoom, density)?;
                    for index in 0..5 {
                        let start = Instant::now();
                        let document = SvgDocument::parse(&renderer, &source)?;
                        let parse = start.elapsed().as_secs_f64() * 1000.;
                        let raster = document.rasterize(&renderer, resolution.output, policy.max_output_bytes)?;
                        rows.push(format!(
                            "Sample {} · Parse {:.3} ms · Raster {:.3} ms · {}×{} output",
                            index + 1,
                            parse,
                            raster.ms,
                            i32::from(resolution.output.width),
                            i32::from(resolution.output.height)
                        ));
                    }
                    Ok::<_, anyhow::Error>(rows)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.benchmarking = false;
                this.bench_result = Some(result.map_err(|error| error.to_string()));
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::{Action, SvgPage, MIB};
    use gpui::{TestAppContext, px};
    use gpui_luma::controls::button::ButtonEvent;

    #[test]
    fn svg_tabs_render_and_luma_budget_controls_update_scheduler() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (page, cx) = app.add_window_view(|_, cx| SvgPage::new(crate::theme::default_look(), cx));
        for tab in ["Rendering", "Scaling", "Path Effects", "Scheduling & Cache", "Benchmarks"] {
            page.update(cx, |page, cx| page.activate(tab.into(), cx));
            cx.run_until_parked();
        }
        let pool =
            cx.update(|_, cx| page.read(cx).buttons.iter().find(|(id, _)| *id == Action::Pool).unwrap().1.clone());
        pool.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert!(page.asset.is_ok());
            assert_eq!(page.scheduler.as_ref().unwrap().limits().max_bytes, 16 * MIB);
            let paths = page.paths.as_ref().unwrap();
            assert_eq!(paths.len(), 3);
            for path in paths {
                assert!(f32::from(path.length()) > 0.);
                let effect = super::paths::effect(0.68, 3, 11.).unwrap();
                assert!(path.stroke(&effect, px(4.)).is_ok());
            }
        });
        let run =
            cx.update(|_, cx| page.read(cx).buttons.iter().find(|(id, _)| *id == Action::Benchmark).unwrap().1.clone());
        run.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert!(!page.benchmarking);
            assert_eq!(page.bench_result.as_ref().unwrap().as_ref().unwrap().len(), 5);
        });
    }
    #[test]
    fn rendering_and_scaling_have_distinct_controls_and_preserve_scaling_camera() {
        use gpui::{VisualTestContext, div, point, prelude::*, size};
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let window =
            app.open_window(size(px(1600.), px(1800.)), |_, cx| SvgPage::new(crate::theme::default_look(), cx));
        let page = window.root(&mut app).unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), &app);
        let draw = |cx: &mut VisualTestContext| {
            cx.draw(point(px(0.), px(0.)), size(px(1600.), px(1800.)), |_, _| div().size_full().child(page.clone()));
        };
        draw(&mut cx);
        assert!(cx.debug_bounds("svg-rendering-comparison").is_some());
        assert!(cx.debug_bounds("svg-scaling-comparison").is_none());
        assert!(cx.debug_bounds("svg-action-Clip").is_some());
        assert!(cx.debug_bounds("svg-action-Zoom8").is_none());
        page.update(&mut cx, |page, cx| page.activate("Scaling".into(), cx));
        draw(&mut cx);
        assert!(cx.debug_bounds("svg-scaling-comparison").is_some());
        assert!(cx.debug_bounds("svg-rendering-comparison").is_none());
        assert!(cx.debug_bounds("svg-action-Clip").is_none());
        assert!(cx.debug_bounds("svg-action-Zoom8").is_some());
        let zoom =
            cx.update(|_, cx| page.read(cx).buttons.iter().find(|(id, _)| *id == Action::Zoom8).unwrap().1.clone());
        zoom.update(&mut cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        page.update(&mut cx, |page, cx| {
            page.zoom_at(point(px(30.), px(20.)), 1., cx);
            page.camera.pan = point(px(70.), px(-20.));
            page.copy_camera(cx);
            page.activate("Rendering".into(), cx);
        });
        draw(&mut cx);
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert_eq!(page.camera.zoom, 8.);
            assert_eq!(page.camera.pan, point(px(70.), px(-20.)));
            let status = page.rendering_status.as_ref().expect("independent Rendering SVG status");
            let actual = status.resolution.unwrap().requested;
            let logical = page.asset.as_ref().unwrap().logical;
            assert_eq!(
                i32::from(actual.width),
                (logical.width * page.density).ceil() as i32,
                "Rendering must remain at 1× despite the retained Scaling camera"
            );
        });
        page.update(&mut cx, |page, cx| page.activate("Scaling".into(), cx));
        draw(&mut cx);
        cx.update(|_, cx| assert_eq!(page.read(cx).camera.zoom, 8.));
    }

    #[test]
    fn scaling_budget_caps_detail_and_reset_preserves_path_parameters() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| SvgPage::new(crate::theme::default_look(), cx));
        page.update(cx, |page, cx| {
            page.activate("Scaling".into(), cx);
            page.action(Action::Zoom16, cx);
            page.action(Action::ScalingBudget, cx); // 32 MiB
            page.action(Action::ScalingBudget, cx); // 0.25 MiB
            page.action(Action::Preview, cx);
            page.action(Action::Settle, cx);
            let logical = page.asset.as_ref().unwrap().logical;
            let resolution = page.policy.resolution(logical, page.camera.zoom, 2.).unwrap();
            assert!(resolution.limited());
            assert!(xgpui_svg::image_bytes(resolution.output) <= MIB / 4);
            let preview = page.policy.preview(resolution.output);
            assert!(i32::from(preview.width) <= 128 && i32::from(preview.height) <= 128);
            assert_eq!(page.policy.settle_delay.as_millis(), 300);
            page.values[3] = 0.25;
            page.action(Action::Reset, cx);
            assert_eq!(page.camera.zoom, 1.);
            assert_eq!(page.policy, super::RasterPolicy::default());
            assert_eq!(page.values[3], 0.25, "Scaling reset must not reset Path Effects");
            page.action(Action::Zoom4, cx);
            page.activate("Path Effects".into(), cx);
            page.action(Action::Reset, cx);
            assert_eq!(page.values[3], 0.68);
            assert_eq!(page.camera.zoom, 4., "Path reset must not reset the Scaling camera");
        });
        cx.run_until_parked();
    }
}
