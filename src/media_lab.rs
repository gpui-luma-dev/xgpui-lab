//! Offline media-lab fixtures, bounded image loading and native video presentation.
mod fixtures;
mod video_source;
mod views;
mod advanced;
mod tiled;
pub(crate) mod benchmarks;

use std::{sync::Arc, time::Instant};
use gpui::{AppContext, Context, Entity, SharedString, Point, Pixels, Bounds};
use gpui_luma::{
    controls::{button::Button, slider::Slider},
    prelude::HasPresenter,
};
use gpui_luma_look_radix::{self as radix, Look};
use xgpui_media::{ImageJob, MediaImage, MediaLoader, MediaLimits, UploadLimits, UploadStatus, LoadStatus, ImageFilter};
use fixtures::Fixtures;

const MIB: usize = 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Sample,
    Reload,
    Burst,
    Jobs,
    Budget,
    Reset,
    Filters,
    Tint,
    Start,
    Format,
    Pause,
    VideoBurst,
    Open,
    Sweep,
    Animate,
    Blur,
    Quality,
    Wide,
    Restart,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parameter {
    Zoom,
    PanX,
    PanY,
    Opacity,
    Radius,
    Brightness,
    Contrast,
    Saturation,
    Hue,
    Exposure,
    LinearContrast,
    LinearSaturation,
    Vignette,
}
impl Parameter {
    fn config(self) -> (&'static str, f32, f32, f32) {
        match self {
            Self::Zoom => ("Zoom", 0.1, 4., 1.),
            Self::PanX => ("Pan X", -300., 300., 0.),
            Self::PanY => ("Pan Y", -200., 200., 0.),
            Self::Opacity => ("Opacity", 0., 1., 1.),
            Self::Radius => ("Corners", 0., 64., 16.),
            Self::Brightness => ("Brightness", 0., 2., 1.),
            Self::Contrast => ("Contrast", 0., 2., 1.),
            Self::Saturation => ("Saturation", 0., 2., 1.),
            Self::Exposure => ("Exposure EV", -2., 2., 0.),
            Self::LinearContrast => ("Linear contrast", 0., 2., 1.),
            Self::LinearSaturation => ("Linear saturation", 0., 2., 1.),
            Self::Vignette => ("Vignette", 0., 0.8, 0.),
            Self::Hue => ("Hue rotation", 0., 360., 0.),
        }
    }
}

pub struct MediaPage {
    look: Arc<Look>,
    active_tab: SharedString,
    fixtures: Option<Fixtures>,
    loading: bool,
    error: Option<String>,
    loader: anyhow::Result<MediaLoader>,
    job: Option<ImageJob>,
    image: Option<MediaImage>,
    selected: usize,
    job_limit: usize,
    budget_index: usize,
    waiting: usize,
    burst_jobs: Vec<ImageJob>,
    burst_images: Vec<MediaImage>,
    completed: usize,
    values: [f32; 13],
    drag: Option<(Point<Pixels>, [f32; 2])>,
    bounds: [Bounds<Pixels>; 2],
    sweep: bool,
    animate: bool,
    started: Instant,
    mode: usize,
    tab_values: std::collections::HashMap<SharedString, [f32; 13]>,
    blur_index: usize,
    full_quality: bool,
    wide: bool,
    float_job: Option<ImageJob>,
    float_image: Option<MediaImage>,
    float_loader: anyhow::Result<MediaLoader>,
    tiled: Option<Entity<tiled::TiledDemo>>,
    video_stopped: bool,
    import_request: u64,
    buttons: Vec<(Action, Entity<Button>)>,
    choices: Vec<(Action, crate::lab_controls::Choice)>,
    sliders: Vec<(Parameter, Slider)>,
    #[cfg(target_os = "macos")]
    video: Option<video_source::Demo>,
    #[cfg(target_os = "macos")]
    format: video_source::SourceFormat,
    video_paused: bool,
    tint_index: usize,
    benchmarks: Entity<benchmarks::Benchmarks>,
    cadence: Option<benchmarks::cadence::Run>,
}

impl MediaPage {
    fn init_choices(&mut self, cx: &mut Context<Self>) {
        self.choices = [Action::Blur, Action::Sample, Action::Jobs, Action::Budget, Action::Tint, Action::Format]
            .into_iter()
            .map(|action| {
                let (label, options, selected) = self.choice_options(action);
                let choice = crate::lab_controls::Choice::new(
                    format!("media_lab-{action:?}"),
                    label,
                    &self.look,
                    options,
                    selected,
                    cx,
                    move |this: &mut Self, index, cx| this.select_choice(action, index, cx),
                );
                (action, choice)
            })
            .collect();
    }
    fn sync_choices(&self, cx: &mut gpui::App) {
        for (action, choice) in &self.choices {
            let (_, options, selected) = self.choice_options(*action);
            choice.sync(options, selected, cx);
        }
    }
    fn choice_options(&self, action: Action) -> (&'static str, Vec<String>, usize) {
        match action {
            Action::Blur => {
                ("Blur radius", crate::lab_controls::labels(&["0 px", "4 px", "8 px", "16 px"]), self.blur_index)
            }
            Action::Sample => (
                "Sample",
                self.fixtures
                    .as_ref()
                    .map(|f| f.samples.iter().map(|s| s.name.clone()).collect())
                    .unwrap_or_default(),
                self.selected,
            ),
            Action::Jobs => (
                "Job limit",
                crate::lab_controls::labels(&["1", "2", "4"]),
                [1, 2, 4].iter().position(|&n| n == self.job_limit).unwrap_or(1),
            ),
            Action::Budget => {
                ("Byte budget", crate::lab_controls::labels(&["32 MiB", "64 MiB", "128 MiB"]), self.budget_index)
            }
            Action::Tint => ("Tint", crate::lab_controls::labels(&["Identity", "Warm", "Cool"]), self.tint_index),
            Action::Format => (
                "Video format",
                crate::lab_controls::labels(&[
                    "BGRA8",
                    "NV12 full · BT.601",
                    "NV12 video · BT.601",
                    "NV12 full · BT.709",
                    "NV12 video · BT.709",
                ]),
                {
                    #[cfg(target_os = "macos")]
                    {
                        video_source::SourceFormat::ALL.iter().position(|&f| f == self.format).unwrap_or(0)
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        0
                    }
                },
            ),
            _ => unreachable!("action has no selector"),
        }
    }
    fn select_choice(&mut self, action: Action, index: usize, cx: &mut Context<Self>) {
        match action {
            Action::Blur => {
                self.blur_index = index;
            }
            Action::Sample => {
                self.selected = index;
                self.reload();
            }
            Action::Jobs => {
                self.job_limit = [1, 2, 4][index];
                self.reset_loader();
            }
            Action::Budget => {
                self.budget_index = index;
                self.reset_loader();
            }
            Action::Tint => {
                self.tint_index = index;
            }
            Action::Format => {
                #[cfg(target_os = "macos")]
                {
                    let running = self.video.is_some();
                    self.stop_video();
                    self.format = video_source::SourceFormat::ALL[index];
                    if running {
                        match video_source::Demo::new(self.format) {
                            Ok(video) => self.video = Some(video),
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    }
                }
            }
            _ => unreachable!("action has no selector"),
        }

        cx.notify();
    }

    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let buttons = [
            (Action::Open, "Open image…"),
            (Action::Sweep, "Sweep zoom"),
            (Action::Animate, "Animate adjustments"),
            (Action::Quality, "Toggle blur quality"),
            (Action::Wide, "Toggle wide output"),
            (Action::Restart, "Restart video"),
            (Action::Reload, "Reload image"),
            (Action::Burst, "Burst ×12"),
            (Action::Reset, "Reset view"),
            (Action::Filters, "Reset filters"),
            (Action::Start, "Stop source"),
            (Action::Pause, "Pause / Resume"),
            (Action::VideoBurst, "Burst 120 frames"),
        ]
        .map(|(action, label)| {
            (
                action,
                radix::Button::new(format!("media-{action:?}"))
                    .look(look.as_ref())
                    .outline()
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
            Parameter::Opacity,
            Parameter::Radius,
            Parameter::Brightness,
            Parameter::Contrast,
            Parameter::Saturation,
            Parameter::Hue,
            Parameter::Exposure,
            Parameter::LinearContrast,
            Parameter::LinearSaturation,
            Parameter::Vignette,
        ]
        .map(|parameter| {
            let (_, min, max, value) = parameter.config();
            (
                parameter,
                radix::Slider::new(format!("media-{parameter:?}"))
                    .look(look.as_ref())
                    .size(radix::SliderSize::One)
                    .range(min..max)
                    .value(value as f64)
                    .step(
                        if matches!(parameter, Parameter::PanX | Parameter::PanY | Parameter::Radius | Parameter::Hue) {
                            1.
                        } else {
                            0.01
                        },
                    )
                    .on_value_changed(cx, move |this: &mut Self, _, value, cx| {
                        this.values[parameter as usize] = value;
                        cx.notify();
                    })
                    .spawn(cx),
            )
        })
        .to_vec();
        let benchmarks = cx.new(|cx| benchmarks::Benchmarks::new(look.clone(), cx));
        let mut page = Self {
            look,
            active_tab: "Images".into(),
            fixtures: None,
            loading: false,
            error: None,
            loader: Self::loader(2, 64 * MIB),
            job: None,
            image: None,
            selected: 0,
            job_limit: 2,
            budget_index: 1,
            waiting: 0,
            burst_jobs: vec![],
            burst_images: vec![],
            completed: 0,
            values: [1., 0., 0., 1., 16., 1., 1., 1., 0., 0., 1., 1., 0.],
            drag: None,
            bounds: [Bounds::default(); 2],
            sweep: false,
            animate: false,
            started: Instant::now(),
            mode: 0,
            tab_values: Default::default(),
            blur_index: 0,
            full_quality: false,
            wide: true,
            float_job: None,
            float_image: None,
            float_loader: Self::loader(2, 64 * MIB),
            tiled: None,
            video_stopped: false,
            import_request: 0,
            buttons,
            choices: Vec::new(),
            sliders,
            #[cfg(target_os = "macos")]
            video: None,
            #[cfg(target_os = "macos")]
            format: Default::default(),
            video_paused: false,
            tint_index: 0,
            benchmarks,
            cadence: None,
        };
        page.init_choices(cx);
        page
    }
    fn reset_loader(&mut self) {
        self.job = None;
        self.image = None;
        self.burst_jobs.clear();
        self.burst_images.clear();
        self.waiting = 0;
        self.completed = 0;
        self.loader = Self::loader(self.job_limit, [32, 64, 128][self.budget_index] * MIB);
        self.reload();
    }
    fn loader(jobs: usize, bytes: usize) -> anyhow::Result<MediaLoader> {
        MediaLoader::with_upload_limits(
            MediaLimits {
                max_jobs: jobs,
                max_resident_bytes: bytes,
                max_image_bytes: 16 * MIB,
                max_encoded_bytes: 32 * MIB,
            },
            UploadLimits { max_jobs: jobs, max_staging_bytes: bytes },
        )
    }
    pub fn activate(&mut self, tab: SharedString, cx: &mut Context<Self>) {
        self.drag = None;
        if tab != "Video Frames" {
            self.stop_video();
            self.video_stopped = false;
        } else if self.active_tab != tab {
            self.video_stopped = false;
        }
        if self.active_tab != tab {
            self.tab_values.insert(self.active_tab.clone(), self.values);
            self.values =
                self.tab_values.get(&tab).copied().unwrap_or([1., 0., 0., 1., 16., 1., 1., 1., 0., 0., 1., 1., 0.]);
            self.sweep = false;
            self.animate = false;
            for (parameter, slider) in &self.sliders {
                slider.update(cx, |slider, cx| slider.set_value(self.values[*parameter as usize], cx));
            }
        }
        self.mode = match tab.as_ref() {
            "Linear Adjustments" => 1,
            "Float / P3" => 2,
            "Tiled Canvas" => 3,
            _ => 0,
        };
        if self.mode == 2 {
            self.load_float();
        }
        if self.mode == 3 && self.tiled.is_none() {
            let look = self.look.clone();
            self.tiled = Some(cx.new(|cx| tiled::TiledDemo::new(look, cx)));
        }
        self.active_tab = tab;
        cx.notify();
    }
    pub fn requires_sdr(&self, cx: &gpui::App) -> bool {
        (self.active_tab == "Video Frames" && self.blur_index > 0)
            || (self.active_tab == "Tiled Canvas"
                && self.tiled.as_ref().is_some_and(|tile| tile.read(cx).requires_sdr()))
    }
    pub fn suspend(&mut self) {
        self.stop_video();
        self.video_stopped = false;
        self.drag = None;
        self.sweep = false;
        self.animate = false;
        self.waiting = 0;
        self.burst_jobs.clear();
        self.burst_images.clear();
    }
    fn has_video(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.video.is_some()
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
    pub fn stop_video(&mut self) {
        #[cfg(target_os = "macos")]
        {
            self.video = None;
        }
        self.video_paused = false;
    }
    fn ensure_fixtures(&mut self, cx: &mut Context<Self>) {
        if self.fixtures.is_some() || self.loading || self.error.is_some() {
            return;
        }
        self.loading = true;
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async { Fixtures::new(None) }).await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(fixtures) => {
                        this.fixtures = Some(fixtures);
                        this.reload();
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn reload(&mut self) {
        self.job = None;
        self.image = None;
        self.error = None;
        if let (Some(fixtures), Ok(loader)) = (&self.fixtures, &self.loader) {
            match loader.load(fixtures.samples[self.selected].path.clone()) {
                Ok(job) => self.job = Some(job),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
    fn poll(&mut self) {
        if let Some(job) = &mut self.job {
            if let Some(image) = job.try_take() {
                self.image = Some(image);
                self.job = None;
            } else if let LoadStatus::Failed(error) = job.status() {
                self.error = Some(error);
                self.job = None;
            }
        }
        let mut pending = Vec::new();
        for mut job in self.burst_jobs.drain(..) {
            if let Some(image) = job.try_take() {
                self.burst_images.push(image);
            } else if let LoadStatus::Failed(error) = job.status() {
                self.error = Some(error);
                self.completed += 1;
            } else {
                pending.push(job);
            }
        }
        self.burst_jobs = pending;
        self.burst_images.retain(|image| match image.upload_status() {
            UploadStatus::Ready => {
                self.completed += 1;
                false
            }
            UploadStatus::Failed(error) => {
                self.error = Some(error);
                self.completed += 1;
                false
            }
            _ => true,
        });
        if let (Some(fixtures), Ok(loader)) = (&self.fixtures, &self.loader) {
            while self.waiting > 0 {
                match loader.load(fixtures.samples[self.selected].path.clone()) {
                    Ok(job) => {
                        self.burst_jobs.push(job);
                        self.waiting -= 1;
                    }
                    Err(_) => break,
                }
            }
        }
    }
    fn action(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::Blur | Action::Sample | Action::Jobs | Action::Budget | Action::Tint | Action::Format => {
                unreachable!("selector setting")
            }
            Action::Open => self.open_image(cx),
            Action::Sweep => {
                self.sweep = !self.sweep;
                self.started = Instant::now();
            }
            Action::Animate => {
                self.animate = !self.animate;
                self.started = Instant::now();
            }
            Action::Quality => self.full_quality = !self.full_quality,
            Action::Wide => self.wide = !self.wide,
            Action::Restart => {
                self.stop_video();
                self.video_stopped = false;
            }
            Action::Reload => {
                self.error = None;
                self.reload();
            }
            Action::Burst => self.waiting += 12,
            Action::Reset | Action::Filters => {
                if action == Action::Reset {
                    self.sweep = false;
                    self.animate = false;
                    self.blur_index = 0;
                    self.full_quality = false;
                }
                if action == Action::Filters
                    || matches!(self.active_tab.as_ref(), "Video Frames" | "Linear Adjustments" | "Float / P3")
                {
                    self.tint_index = 0;
                }
                for (parameter, slider) in &self.sliders {
                    if action == Action::Reset
                        || matches!(
                            parameter,
                            Parameter::Brightness
                                | Parameter::Contrast
                                | Parameter::Saturation
                                | Parameter::Hue
                                | Parameter::Exposure
                                | Parameter::LinearContrast
                                | Parameter::LinearSaturation
                                | Parameter::Vignette
                        )
                    {
                        self.values[*parameter as usize] = parameter.config().3;
                        slider.update(cx, |slider, cx| slider.set_value(parameter.config().3, cx));
                    }
                }
            }
            Action::Start => {
                #[cfg(target_os = "macos")]
                {
                    if self.video.is_some() {
                        if let Some(video) = &self.video {
                            video.stop();
                        }
                        self.video_stopped = true;
                    } else {
                        match video_source::Demo::new(self.format) {
                            Ok(video) => {
                                self.video = Some(video);
                                self.video_paused = false;
                                self.error = None;
                            }
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    }
                }
            }
            Action::Pause => {
                self.video_paused = !self.video_paused;
                #[cfg(target_os = "macos")]
                if let Some(video) = &self.video {
                    video.stream.set_playing(!self.video_paused);
                }
            }
            Action::VideoBurst =>
            {
                #[cfg(target_os = "macos")]
                if let Some(video) = &self.video {
                    video.burst();
                }
            }
        }
        cx.notify();
    }
    fn filter(&self) -> ImageFilter {
        ImageFilter::new()
            .brightness(self.values[5])
            .contrast(self.values[6])
            .saturation(self.values[7])
            .hue_rotate(self.values[8])
            .tint([[0., 0., 0., 0.], [1., 0.4, 0.1, 0.3], [0.1, 0.6, 1., 0.3]][self.tint_index])
    }
}

#[cfg(test)]
mod tests {
    use super::{Fixtures, MediaPage, MIB};
    use std::time::{Duration, Instant};

    #[test]
    fn selectors_pick_nonadjacent_values_and_follow_filter_reset() {
        use super::Action;
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| MediaPage::new(crate::theme::default_look(), cx));
        page.update(cx, |page, cx| {
            page.fixtures = Some(Fixtures::new(None).unwrap());
            page.sync_choices(cx);
        });
        for (action, index) in
            [(Action::Sample, 2), (Action::Jobs, 2), (Action::Budget, 2), (Action::Tint, 2), (Action::Blur, 3)]
        {
            let choice =
                cx.update(|_, cx| page.read(cx).choices.iter().find(|(key, _)| *key == action).unwrap().1.clone());
            cx.update(|_, cx| choice.select(index, cx));
        }
        page.update(cx, |page, cx| {
            assert_eq!(page.selected, 2);
            assert_eq!(page.job_limit, 4);
            assert_eq!(page.budget_index, 2);
            assert_eq!(page.tint_index, 2);
            assert_eq!(page.blur_index, 3);
            page.action(Action::Filters, cx);
            page.sync_choices(cx);
            let tint = &page.choices.iter().find(|(key, _)| *key == Action::Tint).unwrap().1;
            assert_eq!(page.tint_index, 0);
            assert_eq!(tint.selector.read(cx).selected_id().unwrap().as_ref(), "0");
        });
    }

    #[test]
    fn source_lab_fixtures_decode_with_mips_and_release_loader_reservations() {
        let fixtures = Fixtures::new(None).expect("offline lab fixtures");
        assert_eq!(fixtures.samples.len(), 3);
        let loader = MediaPage::loader(1, 16 * MIB).expect("bounded loader");
        let mut job = loader.load(fixtures.samples[0].path.clone()).expect("admit first image");
        assert!(loader.load(fixtures.samples[1].path.clone()).is_err());
        let start = Instant::now();
        let image = loop {
            if let Some(image) = job.try_take() {
                break image;
            }
            assert!(start.elapsed() < Duration::from_secs(15), "image decode timed out: {:?}", job.status());
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(image.width(), fixtures.samples[0].width);
        assert!(image.mip_count() > 1);
        assert_eq!(image.upload_count(), 0, "CPU decode must not imply GPU upload");
        assert!(loader.load(fixtures.samples[1].path.clone()).is_err());
        drop(image);
        drop(job);
        assert!(loader.load(fixtures.samples[1].path.clone()).is_ok(), "retired snapshots release source admission");
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn luma_media_controls_update_filters_and_release_native_video() {
        use gpui::TestAppContext;
        use gpui_luma::controls::button::ButtonEvent;
        use super::{Action, Parameter};
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (page, cx) = app.add_window_view(|_, cx| MediaPage::new(crate::theme::default_look(), cx));
        page.update(cx, |page, cx| page.activate("Video Frames".into(), cx));
        cx.run_until_parked();
        let (saturation, reset) = cx.update(|_, cx| {
            let page = page.read(cx);
            (
                page.sliders.iter().find(|(id, _)| *id == Parameter::Saturation).unwrap().1.clone(),
                page.buttons.iter().find(|(id, _)| *id == Action::Filters).unwrap().1.clone(),
            )
        });
        saturation.update(cx, |slider, cx| {
            slider.set_thumb_position(slider.primary_thumb_id(), 0., cx);
        });
        cx.run_until_parked();
        cx.update(|_, cx| assert_eq!(page.read(cx).values[7], 0.));
        reset.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        page.update(cx, |page, _| {
            assert_eq!(page.values[7], 1.);
            page.stop_video();
        });
    }
    #[cfg(feature = "test-support")]
    #[test]
    fn media_camera_keeps_pointer_anchor_and_suspension_clears_motion() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| MediaPage::new(crate::theme::default_look(), cx));
        page.update(cx, |page, cx| {
            page.values[1] = 20.;
            page.values[2] = -10.;
            let anchor = gpui::point(gpui::px(80.), gpui::px(30.));
            page.zoom_at(anchor, 2., cx);
            assert_eq!(page.values[0], 2.);
            assert_eq!(page.values[1], -40.);
            assert_eq!(page.values[2], -50.);
            page.sweep = true;
            page.animate = true;
            page.video_stopped = true;
            page.suspend();
            assert!(!page.sweep && !page.animate && !page.video_stopped);
            assert!(page.drag.is_none());
        });
    }
    #[cfg(feature = "test-support")]
    #[test]
    fn specialized_tabs_restore_independent_camera_and_filter_values() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| MediaPage::new(crate::theme::default_look(), cx));
        page.update(cx, |page, cx| {
            page.values[0] = 2.;
            page.activate("Image Filters".into(), cx);
            assert_eq!(page.values[0], 1.);
            page.values[7] = 0.2;
            page.activate("Linear Adjustments".into(), cx);
            assert_eq!(page.mode, 1);
            assert_eq!(page.values[7], 1.);
            page.activate("Image Filters".into(), cx);
            assert_eq!(page.mode, 0);
            assert_eq!(page.values[7], 0.2);
            page.activate("Images".into(), cx);
            assert_eq!(page.values[0], 2.);
        });
    }
}
