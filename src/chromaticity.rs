//! Shared color-pipeline workspace for the full app and standalone lab.
#[path = "bin/chromaticity-lab/fill.rs"]
mod fill;
#[path = "bin/chromaticity-lab/metadata.rs"]
mod metadata;
#[path = "bin/chromaticity-lab/gpu.rs"]
mod gpu;
#[path = "bin/chromaticity-lab/reveal.rs"]
mod reveal;
#[path = "bin/chromaticity-lab/photo.rs"]
mod photo;
use gpui::{prelude::*, *};
use std::sync::Arc;
use xgpui_color::{GamutMapping, LinearRgb, RgbSpace, XyzD65};
use xgpui_media::{
    ImageJob, MediaImage, MediaLoader,
    pipeline::{self, AlphaMode, ImageColor, ImagePrimaries, TransferFunction},
};

const SPACES: [RgbSpace; 4] = [RgbSpace::Srgb, RgbSpace::DisplayP3, RgbSpace::AdobeRgb1998, RgbSpace::Rec2020];
const NAMES: [&str; 4] = ["sRGB", "Display P3", "Adobe RGB (1998)", "Rec.2020"];
const COLORS: [u32; 4] = [0x65d7ff, 0xff9a62, 0xc4a0ff, 0x91df86];
const PRESETS: [(&str, [f32; 3]); 9] = [
    ("Green", [0., 1., 0.]),
    ("Cyan", [0., 1., 1.]),
    ("Red", [1., 0., 0.]),
    ("Magenta", [1., 0., 1.]),
    ("Mid gray", [0.18; 3]),
    ("Near black", [0.001; 3]),
    ("White", [1.; 3]),
    ("Probe gray", [0.0001; 3]),
    ("Probe blue", [0., 0., 0.0001]),
];

fn coordinates(xyz: [f64; 3], uv: bool) -> Option<[f64; 2]> {
    if xyz.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let scale = xyz.into_iter().map(f64::abs).fold(0., f64::max);
    if scale == 0. {
        return None;
    }
    let [x, y, z] = xyz.map(|v| v / scale);
    let d = if uv { x + 15. * y + 3. * z } else { x + y + z };
    if d == 0. {
        return None;
    }
    let result = if uv { [4. * x / d, 9. * y / d] } else { [x / d, y / d] };
    result.iter().all(|v| v.is_finite()).then_some(result)
}

fn half(bits: u16) -> f64 {
    let sign = if bits & 0x8000 == 0 { 1. } else { -1. };
    let exponent = (bits >> 10) & 31;
    let fraction = f64::from(bits & 1023);
    if exponent == 31 {
        return if fraction == 0. { sign * f64::INFINITY } else { f64::NAN };
    }
    sign * if exponent == 0 {
        fraction * 2f64.powi(-24)
    } else {
        (1. + fraction / 1024.) * 2f64.powi(i32::from(exponent) - 15)
    }
}
fn primaries(space: RgbSpace) -> ImagePrimaries {
    match space {
        RgbSpace::Srgb => ImagePrimaries::Srgb,
        RgbSpace::DisplayP3 => ImagePrimaries::DisplayP3,
        RgbSpace::AdobeRgb1998 => ImagePrimaries::AdobeRgb1998,
        RgbSpace::Rec2020 => ImagePrimaries::Rec2020,
    }
}
#[derive(Clone)]
struct Inspection {
    linear: [f64; 3],
    xyz: [f64; 3],
    stored: [f64; 4],
    bits: [u16; 4],
    composed: [f64; 3],
    error: f64,
    mapped: bool,
}
pub(crate) struct Lab {
    photo_open: bool,
    photo_selection: usize,
    photo: Option<photo::Photo>,
    reveal_open: bool,
    standalone_navigation: bool,
    reveal_selection: usize,
    reveal: Option<reveal::Reveal>,
    metadata: metadata::Mode,
    source: usize,
    preset: usize,
    transfer: usize,
    rec: bool,
    reduce: bool,
    alpha: f32,
    uv: bool,
    effects: bool,
    rgb: [f32; 3],
    jobs: Vec<ImageJob>,
    images: Vec<MediaImage>,
    inspection: Option<Inspection>,
    error: Option<String>,
    wide: bool,
    fill_route: usize,
    fill_output: usize,
    fill_luminance: usize,
    hatch: bool,
    fill_worker: Option<fill::Job>,
    fill_job: Option<ImageJob>,
    fill_image: Option<MediaImage>,
    fill_status: String,
    gpu_bounds: Arc<std::sync::Mutex<Option<Bounds<ScaledPixels>>>>,
    gpu_report: String,
}
impl Lab {
    fn initial() -> Self {
        let mut s = Self {
            photo_open: false,
            photo_selection: 1,
            photo: None,
            reveal_open: false,
            standalone_navigation: true,
            reveal_selection: 1,
            reveal: None,
            metadata: metadata::Mode::Named,
            source: 2,
            preset: 0,
            transfer: 1,
            rec: true,
            reduce: false,
            alpha: 0.5,
            uv: false,
            effects: false,
            rgb: [0., 1., 0.],
            jobs: vec![],
            images: vec![],
            inspection: None,
            error: None,
            wide: false,
            fill_route: 1,
            fill_output: 1,
            fill_luminance: 1,
            hatch: true,
            fill_worker: None,
            fill_job: None,
            fill_image: None,
            fill_status: String::new(),
            gpu_bounds: Arc::new(std::sync::Mutex::new(None)),
            gpu_report: "GPU readback: not checked. Show the full native sample, then click Check GPU.".into(),
        };
        s.reload();
        s.reload_fill();
        s
    }
    pub(crate) fn new(window: &mut Window, standalone: bool) -> Self {
        let mut s = Self::initial();
        s.standalone_navigation = standalone;
        if standalone {
            if std::env::args().any(|a| a == "--icc-parametric" || a == "--icc-sampled") {
                s.metadata = if std::env::args().any(|a| a == "--icc-sampled") {
                    metadata::Mode::Sampled
                } else {
                    metadata::Mode::Parametric
                };
                s.preset = 7;
                s.rgb = PRESETS[7].1;
                s.reload();
            }
            if std::env::args().any(|a| a == "--native-fill") {
                s.fill_route = 2;
            }
            if std::env::args().any(|a| a == "--uv") {
                s.uv = true;
                s.reload_fill();
            }
            if std::env::args().any(|a| a == "--photo") {
                s.set_photo();
            }
            s.effects = cfg!(target_os = "macos") && std::env::args().any(|a| a == "--effects");
            match window.set_color_mode(WindowColorMode::WideGamutSdr) {
                Ok(status) => s.wide = status.effective() == WindowColorMode::WideGamutSdr,
                Err(e) => s.error = Some(format!("Wide unavailable: {e:#}")),
            };
        } else {
            s.wide = window.color_status().effective() == WindowColorMode::WideGamutSdr;
        }
        if let Err(error) = metadata::fixtures() {
            s.error = Some(format!("ICC fixtures: {error:#}"));
        }
        s
    }
    fn color(&self) -> ImageColor {
        ImageColor {
            primaries: primaries(SPACES[self.source]),
            transfer: [TransferFunction::Srgb, TransferFunction::AdobeRgb1998, TransferFunction::Linear][self.transfer],
            alpha: AlphaMode::Straight,
        }
    }
    fn reload(&mut self) {
        self.jobs.clear();
        self.images.clear();
        self.error = None;
        let result = (|| -> anyhow::Result<()> {
            let rgb = self.rgb;
            let rgba = [rgb[0], rgb[1], rgb[2], self.alpha];
            let color = self.metadata.color(self.color())?;
            let mapping = if self.reduce {
                GamutMapping::ReduceChroma
            } else {
                GamutMapping::Clip
            };
            let source = metadata::inspect(&color, rgba)?;
            let target = if self.rec {
                RgbSpace::Rec2020
            } else {
                RgbSpace::DisplayP3
            };
            let converted = XyzD65::new(source.xyz)?.to_linear(target)?.components();
            let mapped = converted.iter().any(|v| *v < -1e-12 || *v > 1. + 1e-12);
            let data = pipeline::pixels_rgba16f_data(1, 1, vec![rgba], color.clone(), primaries(target), mapping)?;
            let bits =
                std::array::from_fn(|i| u16::from_le_bytes([data.levels()[0][2 * i], data.levels()[0][2 * i + 1]]));
            let stored = bits.map(half);
            let straight = if stored[3] > 0. {
                stored[..3].iter().map(|v| v / stored[3]).collect::<Vec<_>>()
            } else {
                vec![0.; 3]
            };
            let restored = LinearRgb::new(target, [straight[0], straight[1], straight[2]])?;
            let original = source.xyz;
            let actual = restored.to_xyz()?.components();
            let error = original.into_iter().zip(actual).map(|(a, b)| (a - b).abs()).fold(0., f64::max);
            let composed = restored.convert(RgbSpace::DisplayP3)?.components();
            self.inspection =
                Some(Inspection { linear: source.decoded, xyz: original, stored, bits, composed, error, mapped });
            let loader = MediaLoader::default();
            // Native and preview share identical pixels; separate jobs also exercise retention/upload.
            for _ in 0..2 {
                self.jobs.push(loader.load_pixel_buffer(
                    64,
                    32,
                    vec![rgba; 64 * 32],
                    color.clone(),
                    primaries(target),
                    mapping,
                )?);
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.error = Some(format!("{e:#}"));
            self.inspection = None;
        }
    }
    fn reload_fill(&mut self) {
        self.fill_worker = None;
        self.fill_job = None;
        self.fill_image = None;
        self.fill_status = "Generating fill…".into();
        self.fill_worker = Some(fill::Job::start(fill::Settings {
            uv: self.uv,
            luminance: fill::LUMINANCE[self.fill_luminance],
            output: SPACES[self.fill_output],
            mapping: if self.reduce {
                GamutMapping::ReduceChroma
            } else {
                GamutMapping::Clip
            },
            hatch: self.hatch,
        }));
    }
    pub(crate) fn set_example(&mut self, reveal: bool) {
        self.photo_open = false;
        self.reveal_open = reveal;
        if reveal && self.reveal.is_none() {
            match reveal::Reveal::new() {
                Ok(reveal) => self.reveal = Some(reveal),
                Err(error) => self.error = Some(format!("Reveal demo: {error:#}")),
            }
        }
    }
    pub(crate) fn set_photo(&mut self) {
        self.photo_open = true;
        self.reveal_open = false;
        if self.photo.is_none() {
            self.photo = Some(photo::Photo::new());
        }
    }
    fn window_mode_controls(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let wide = window.color_status().requested() == WindowColorMode::WideGamutSdr;
        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(self.button(format!("{} Standard (sRGB)", if wide { "○" } else { "●" }), 22, cx))
            .child(self.button(format!("{} Wide gamut", if wide { "●" } else { "○" }), 23, cx))
            .into_any_element()
    }
    fn button(&self, label: String, action: usize, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id(ElementId::Name(format!("control-{action}").into()))
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(0x263448))
            .text_color(rgb(0xe2eaf3))
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |s, _, window, cx| {
                if action != 17 {
                    s.gpu_report = "GPU readback: controls changed; check again.".into();
                }
                match action {
                    0 => {
                        if s.metadata == metadata::Mode::Named {
                            s.source = (s.source + 1) % 4;
                        }
                        s.metadata = metadata::Mode::Named;
                    }
                    1 => {
                        s.preset = (s.preset + 1) % PRESETS.len();
                        s.rgb = PRESETS[s.preset].1;
                    }
                    2 => {
                        if s.metadata == metadata::Mode::Named {
                            s.transfer = (s.transfer + 1) % 3;
                        }
                        s.metadata = metadata::Mode::Named;
                    }
                    3 => s.rec = !s.rec,
                    4 => s.reduce = !s.reduce,
                    5 => {
                        s.alpha = if s.alpha == 1. {
                            0.5
                        } else if s.alpha == 0.5 {
                            0.
                        } else {
                            1.
                        }
                    }
                    6 => s.uv = !s.uv,
                    7 => {
                        if cfg!(target_os = "macos") {
                            s.effects = !s.effects;
                        } else {
                            s.error = Some("Nested wide captures are currently validated on Metal only".into());
                        }
                    }
                    8 | 22 | 23 => {
                        let requested = if action == 22
                            || (action == 8 && window.color_status().requested() == WindowColorMode::WideGamutSdr)
                        {
                            WindowColorMode::Default
                        } else {
                            WindowColorMode::WideGamutSdr
                        };
                        match window.set_color_mode(requested) {
                            Ok(status) => {
                                s.wide = status.effective() == WindowColorMode::WideGamutSdr;
                                s.error = None;
                            }
                            Err(e) => s.error = Some(format!("Wide presentation: {e:#}")),
                        };
                        cx.notify();
                        return;
                    }
                    9..=11 => {
                        let c = action - 9;
                        let steps = [0., 0.0001, 0.001, 0.18, 0.5, 1.];
                        let index = steps.iter().position(|v| *v == s.rgb[c]).unwrap_or(0);
                        s.rgb[c] = steps[(index + 1) % steps.len()];
                    }
                    12 => s.fill_route = (s.fill_route + 1) % 3,
                    13 => s.fill_luminance = (s.fill_luminance + 1) % fill::LUMINANCE.len(),
                    14 => s.fill_output = (s.fill_output + 1) % SPACES.len(),
                    15 => s.hatch = !s.hatch,
                    16 => s.metadata = s.metadata.next(),
                    17 => {
                        let result = (|| -> anyhow::Result<String> {
                            anyhow::ensure!(s.wide, "Enable the wide-gamut window first (Metal only)");
                            anyhow::ensure!(
                                s.images.first().is_some_and(|i| i.upload_status() == xgpui_media::UploadStatus::Ready),
                                "Wait for native sample upload"
                            );
                            let inspection = s.inspection.as_ref().ok_or_else(|| anyhow::anyhow!("No valid pixel"))?;
                            let bounds = s
                                .gpu_bounds
                                .lock()
                                .unwrap()
                                .ok_or_else(|| anyhow::anyhow!("Native sample has not painted"))?;
                            let frame = window.render_to_linear_p3()?;
                            gpu::verify(&frame, bounds, inspection.stored, s.rec, s.effects)
                        })();
                        s.gpu_report = result.unwrap_or_else(|e| format!("GPU check: {e:#}"));
                        cx.notify();
                        return;
                    }
                    18 => s.set_example(!s.reveal_open),
                    19..=21 => s.reveal_selection = action - 19,
                    24..=26 => s.photo_selection = action - 24,
                    27 => s.set_example(false),
                    28 => {
                        if let Some(photo) = &mut s.photo {
                            photo.check(window);
                        }
                    }
                    _ => {}
                }
                if action < 6 || action == 16 || (9..=11).contains(&action) {
                    s.reload();
                }
                if [4, 6, 13, 14, 15].contains(&action) {
                    s.reload_fill();
                }
                cx.notify();
            }))
            .into_any_element()
    }
    fn diagram(&self) -> impl IntoElement {
        let uv = self.uv;
        let fill = self.fill_image.clone().filter(|_| self.fill_route != 0);
        let native = self.fill_route == 2 && self.wide;
        let selected = self.inspection.as_ref().and_then(|i| coordinates(i.xyz, uv));
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let inset = 24.;
                let width = f32::from(bounds.size.width) - 2. * inset;
                let height = f32::from(bounds.size.height) - 2. * inset;
                let xmax = if uv { 0.65 } else { 0.8 };
                let ymax = if uv { 0.65 } else { 0.9 };
                let scale = (width / xmax).min(height / ymax);
                let width = scale * xmax;
                let height = scale * ymax;
                let horizontal_inset = (f32::from(bounds.size.width) - width) / 2.;
                let pos = |p: [f64; 2]| {
                    point(
                        bounds.left() + px(horizontal_inset + p[0] as f32 / xmax * width),
                        bounds.bottom() - px(inset + p[1] as f32 / ymax * height),
                    )
                };
                if let Some(image) = &fill {
                    let bounds = Bounds::new(pos([0., f64::from(ymax)]), size(px(width), px(height)));
                    wake_for_upload(image, bounds, window);
                    let geometry = CustomGeometry::RoundedRect { bounds, corner_radii: Default::default() };
                    if native {
                        if let Ok(source) = image.wide(None) {
                            window.paint_custom(geometry, Arc::new(source));
                        }
                    } else {
                        window.paint_custom(geometry, Arc::new(image.clone()));
                    }
                }
                for n in 0..=8 {
                    let t = n as f64 / 8.;
                    for segment in [
                        [pos([t * f64::from(xmax), 0.]), pos([t * f64::from(xmax), f64::from(ymax)])],
                        [pos([0., t * f64::from(ymax)]), pos([f64::from(xmax), t * f64::from(ymax)])],
                    ] {
                        let mut p = PathBuilder::stroke(px(1.));
                        p.move_to(segment[0]);
                        p.line_to(segment[1]);
                        if let Ok(p) = p.build() {
                            window.paint_path(p, rgb(0x263448));
                        }
                    }
                }
                let mut locus = PathBuilder::stroke(px(1.5));
                for (index, line) in include_str!("../assets/colorimetry/CIE_cc_1931_2deg.csv").lines().enumerate() {
                    let v: Vec<f64> = line.split(',').map(|s| s.parse::<f64>().expect("validated CIE datum")).collect();
                    let xy = coordinates([v[1], v[2], v[3]], uv).unwrap();
                    if index == 0 {
                        locus.move_to(pos(xy));
                    } else {
                        locus.line_to(pos(xy));
                    }
                }
                locus.close();
                if let Ok(p) = locus.build() {
                    window.paint_path(p, rgb(0x8b97aa));
                }
                for (i, space) in SPACES.into_iter().enumerate() {
                    let mut path = PathBuilder::stroke(px(2.));
                    for c in 0..3 {
                        let rgb = std::array::from_fn(|k| if k == c { 1. } else { 0. });
                        let xy = coordinates(LinearRgb::new(space, rgb).unwrap().to_xyz().unwrap().components(), uv)
                            .unwrap();
                        if c == 0 {
                            path.move_to(pos(xy));
                        } else {
                            path.line_to(pos(xy));
                        }
                    }
                    path.close();
                    if let Ok(p) = path.build() {
                        window.paint_path(p, rgb(COLORS[i]));
                    }
                }
                for (location, color) in [
                    (coordinates([0.3127 / 0.3290, 1., (1. - 0.3127 - 0.3290) / 0.3290], uv), 0xffffff),
                    (selected, 0xffdc78),
                ] {
                    if let Some(p) = location {
                        let p = pos(p);
                        let mut path = PathBuilder::stroke(px(2.));
                        path.move_to(p - point(px(6.), px(0.)));
                        path.line_to(p + point(px(6.), px(0.)));
                        path.move_to(p - point(px(0.), px(6.)));
                        path.line_to(p + point(px(0.), px(6.)));
                        if let Ok(path) = path.build() {
                            window.paint_path(path, rgb(color));
                        }
                    }
                }
            },
        )
        .w_full()
        .h(px(680.))
        .flex_shrink_0()
    }
    fn patch(&self, image: Option<MediaImage>, native: bool, scale: f32) -> AnyElement {
        let wide = self.wide && native;
        let gpu_bounds = self.gpu_bounds.clone();
        // Reserve the core capture region's worst-case four-level halo:
        // 16 logical + 8 physical pixels per side per nesting level.
        let panel_width = ((1000. - 8. * (16. * scale + 8.)) / scale).clamp(24., 320.);
        let content = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                if native {
                    *gpu_bounds.lock().unwrap() = Some(bounds.scale(window.scale_factor()));
                }
                if let Some(image) = &image {
                    wake_for_upload(image, bounds, window);
                    if wide {
                        if let Ok(image) = image.wide(None) {
                            window.paint_custom(
                                CustomGeometry::RoundedRect { bounds, corner_radii: Default::default() },
                                Arc::new(image),
                            );
                        }
                    } else {
                        window.paint_custom(
                            CustomGeometry::RoundedRect { bounds, corner_radii: Default::default() },
                            Arc::new(image.clone()),
                        );
                    }
                }
            },
        )
        .w_full()
        .h(px(90.));
        let sample = xgpui_shader::painted().w(px(panel_width)).h(px(120.)).p_3().child(content);
        let sample = if self.effects {
            sample.layer_effect(xgpui_shader::LayerEffect::opacity(0.65).unwrap().with_blur(px(8.)).unwrap())
        } else {
            sample
        };
        let sample = if self.effects {
            xgpui_shader::painted()
                .w(px(panel_width))
                .layer_effect(xgpui_shader::LayerEffect::opacity(0.8).unwrap())
                .child(sample)
                .into_any_element()
        } else {
            sample.into_any_element()
        };
        div()
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(if native {
                if self.wide {
                    "Native wide • extended composition"
                } else {
                    "Native wide unavailable • sRGB preview"
                }
            } else {
                "Clipped sRGB preview"
            })
            .child(div().bg(rgb(0xffffff)).child(sample))
            .into_any_element()
    }
}
impl Render for Lab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.wide = window.color_status().effective() == WindowColorMode::WideGamutSdr;
        if self.photo_open {
            return self.render_photo(window, cx).into_any_element();
        }
        if self.reveal_open {
            return self.render_reveal(window, cx).into_any_element();
        }
        if let Some(worker) = &self.fill_worker {
            match worker.poll() {
                Ok(Some(result)) => {
                    self.fill_status = format!(
                        "{:.1}% of spectral-locus cells require mapping at this Y",
                        100. * result.mapped as f64 / result.visible.max(1) as f64
                    );
                    self.fill_job = Some(result.image);
                    self.fill_worker = None;
                }
                Ok(None) => {}
                Err(error) => {
                    self.fill_status = format!("Fill failed: {error:#}");
                    self.fill_worker = None;
                }
            }
        }
        if let Some(job) = &mut self.fill_job {
            if let Some(image) = job.try_take() {
                self.fill_image = Some(image);
            }
            if let xgpui_media::LoadStatus::Failed(error) = job.status() {
                self.fill_status = format!("Fill failed: {error}");
            }
        }
        if self.fill_worker.is_some()
            || self.fill_job.as_ref().is_some_and(|j| j.status() == xgpui_media::LoadStatus::Loading)
        {
            window.request_animation_frame();
        }
        if let Some(image) = &self.fill_image
            && let xgpui_media::UploadStatus::Failed(error) = image.upload_status()
        {
            self.fill_status = format!("Fill upload failed: {error}");
        }
        for job in &mut self.jobs {
            if let Some(image) = job.try_take() {
                self.images.push(image);
            }
            if let xgpui_media::LoadStatus::Failed(e) = job.status() {
                self.error = Some(e);
            }
        }
        if self.jobs.iter().any(|j| j.status() == xgpui_media::LoadStatus::Loading) {
            window.request_animation_frame();
        }
        for image in &self.images {
            if let xgpui_media::UploadStatus::Failed(e) = image.upload_status() {
                self.error = Some(e);
            }
        }
        let i = self.inspection.clone();
        let info = if let Some(i) = i {
            format!(
                "Decoded source RGB  {:.10?}\nXYZ D65             {:.10?}\nCoordinates         {:.10?}\nStored RGBA16F      {:.10?}\nFP16 bits           {:04x?}\nPredicted linear P3 {:.7?}\nMax XYZ error       {:.9e}\nIngestion mapping   {}",
                i.linear,
                i.xyz,
                coordinates(i.xyz, self.uv),
                i.stored,
                i.bits,
                i.composed,
                i.error,
                if i.mapped {
                    "source outside storage gamut — mapping applied"
                } else {
                    "inside storage gamut"
                }
            )
        } else {
            "No valid pixel".into()
        };
        div().id("chromaticity-scroll").overflow_y_scroll().size_full().bg(rgb(0x101823)).text_color(rgb(0xe2eaf3)).p_6().flex().flex_col().gap_3()
            .when(self.standalone_navigation, |body| body.child(self.button("Try the hidden wide-gamut symbol →".into(),18,cx)))
            .child(div().text_2xl().child("Chromaticity & Color Gamut Lab"))
            .child("Source → transfer decode → explicit storage mapping → FP16 → extended composition → output")
            .child(div().flex().flex_wrap().gap_2().child(self.button(format!("Source: {}",if self.metadata==metadata::Mode::Named {NAMES[self.source]} else {"ICC colorants"}),0,cx)).child(self.button(format!("Patch: {}",if self.rgb==PRESETS[self.preset].1 {PRESETS[self.preset].0} else {"Custom"}),1,cx)).child(self.button(if self.metadata==metadata::Mode::Named {format!("Transfer: {:?}",self.color().transfer)} else {"Transfer: ICC curves".into()},2,cx)).child(self.button(format!("Storage: {}",if self.rec {"Rec.2020"} else {"P3"}),3,cx)).child(self.button(format!("Map: {}",if self.reduce {"reduce chroma"} else {"clip"}),4,cx)).child(self.button(format!("Alpha: {}",self.alpha),5,cx)).child(self.button(if self.uv {"CIE u′v′"} else {"CIE xy"}.into(),6,cx)).child(self.button(format!("Nested captures + blur: {}",self.effects),7,cx)).child(self.window_mode_controls(window,cx)))
            .child(div().flex().flex_wrap().gap_2().child(self.button(format!("Metadata: {}",self.metadata.label()),16,cx)).child(self.button(format!("Presentation: {}",["off","sRGB preview","native wide"][self.fill_route]),12,cx)).child(self.button(format!("Fill Y: {}",fill::LUMINANCE[self.fill_luminance]),13,cx)).child(self.button(format!("Simulated gamut: {}",NAMES[self.fill_output]),14,cx)).child(self.button(format!("Hatch mapped colors: {}",self.hatch),15,cx)))
            .child(div().text_sm().child(format!("{} • {}. Hatching marks colors outside the selected RGB bounds at fixed relative Y, not measured monitor coverage. Fill Y is before mapping and independent of patch brightness. sRGB preview additionally clips to sRGB.",self.fill_status,if self.fill_route==2 && !self.wide {"native unavailable — showing sRGB preview"} else {"output mapping is explicit"})))
            .child(div().text_sm().child("Wide gamut describes display capability; P3 and Adobe RGB are specific color spaces. Simulated gamut limits the fill; presentation selects sRGB clipping or native wide output. The display profile and panel determine visible colors."))
            .child(div().text_sm().child("Click controls to cycle. RGB steps: 0, 0.0001, 0.001, 0.18, 0.5, 1. Source/Transfer return ICC patches to named metadata. ICC metadata includes synthetic Adobe-primary probes and published Compact v4 sRGB, P3, Adobe-compatible profiles and sampled sRGB2014. Serialized curves and colorants are preserved. Probe presets are intentionally dark; choose Mid gray for visible differences."))
            .child(div().flex().gap_2().children((0..3).map(|c|self.button(format!("{}: {}",["R","G","B"][c],self.rgb[c]),9+c,cx))))
            .child(div().text_sm().child(format!("{} • active: {:?} • display reports wide gamut: {:?} • fallback: {:?}",if cfg!(target_os="macos") {"Metal"} else if cfg!(target_os="windows") {"D3D11"} else {"Native backend"},window.color_status().effective(),window.color_status().display_wide_gamut(),window.color_status().fallback())))
            .child(div().flex().items_start().flex_shrink_0().gap_5().child(div().flex_1().min_w_0().child(self.diagram()).child(div().flex().gap_3().children(NAMES.into_iter().enumerate().map(|(i,n)|div().text_color(rgb(COLORS[i])).child(n)))).child("White cross: D65 • gold cross: selected source. Axes start at zero; xy max 0.8 × 0.9, u′v′ max 0.65 × 0.65. CIE 1931 2° spectral locus + purple boundary (CIE 2019, CC BY-SA 4.0); no monitor measurement." )).child(div().w(px(360.)).flex_shrink_0().flex().flex_col().gap_4().child(self.patch(self.images.first().cloned(),true,window.scale_factor())).child(self.patch(self.images.get(1).cloned(),false,window.scale_factor())).child(self.button("Check GPU (Metal FP16)".into(),17,cx)).child(div().text_sm().child(self.gpu_report.clone()))))
            .child(div().flex_shrink_0().flex().flex_col().gap_2().child(div().text_lg().child("Decode tracking")).child(div().text_sm().child(info)))
            .child(self.error.clone().unwrap_or_else(||"P3 red lies slightly outside Rec.2020. Keep P3 storage to retain all of P3. Plotted gamuts do not imply monitor coverage. FP16 values are inspected from actual CPU-packed bytes; composition values above are predictions; Check GPU measures native sample center pixels.".into()))
            .into_any_element()
    }
}
// Poll only visible uploads. An offscreen or disabled fill may remain Idle;
// keeping the whole window animating for that state would waste CPU indefinitely.
fn wake_for_upload(image: &MediaImage, bounds: Bounds<Pixels>, window: &mut Window) {
    if !bounds.intersect(&window.content_mask().bounds).is_empty()
        && !matches!(image.upload_status(), xgpui_media::UploadStatus::Ready | xgpui_media::UploadStatus::Failed(_))
    {
        window.request_animation_frame();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;
    #[test]
    fn d65_coordinates_and_black_singularity() {
        let xyz = [0.3127 / 0.3290, 1., (1. - 0.3127 - 0.3290) / 0.3290];
        let xy = coordinates(xyz, false).unwrap();
        assert!((xy[0] - 0.3127).abs() < 8. * f64::EPSILON);
        assert!((xy[1] - 0.3290).abs() < 8. * f64::EPSILON);
        let uv = coordinates(xyz, true).unwrap();
        assert!((uv[0] - 0.1978300066428368).abs() < 8. * f64::EPSILON);
        assert!((uv[1] - 0.468319994938791).abs() < 8. * f64::EPSILON);
        assert_eq!(coordinates([0.; 3], false), None);
    }
    #[test]
    fn chromaticity_is_scale_invariant_and_rejects_nonfinite() {
        let white = [0.9504559270516716, 1., 1.0890577507598784];
        for uv in [false, true] {
            let reference = coordinates(white, uv).unwrap();
            for scale in [1e-300, 1e-30, 1., 1e30, 1e300] {
                let value = coordinates(white.map(|v| v * scale), uv).unwrap();
                for c in 0..2 {
                    assert!((value[c] - reference[c]).abs() <= 8. * f64::EPSILON);
                }
            }
        }
        assert!(coordinates([f64::NAN, 1., 1.], false).is_none());
    }
    #[test]
    fn cie_dataset_matches_published_validation_rows() {
        let rows: Vec<_> = include_str!("../assets/colorimetry/CIE_cc_1931_2deg.csv").lines().collect();
        assert_eq!(rows.len(), 471);
        assert_eq!(rows[89], "449,0.15763,0.01684,0.82553");
        assert_eq!(rows[190], "550,0.30160,0.69231,0.00609");
    }
    #[test]
    fn initial_adobe_pipeline_retains_negative_p3() {
        let lab = Lab::initial();
        let i = lab.inspection.unwrap();
        assert!(!i.mapped);
        assert!(i.composed[0] < -0.1);
        assert!(i.bits[3] == 0x3800);
    }
    #[test]
    fn example_switching_retains_both_examples_settings() {
        let mut lab = Lab::initial();
        lab.source = 1;
        lab.alpha = 1.;
        lab.set_example(true);
        assert!(lab.reveal_open && lab.reveal.is_some());
        lab.reveal_selection = 2;
        lab.set_example(false);
        assert!(!lab.reveal_open);
        assert_eq!(lab.source, 1);
        assert_eq!(lab.alpha, 1.);
        lab.set_example(true);
        assert_eq!(lab.reveal_selection, 2);
        assert!(lab.reveal.is_some());
    }
    #[test]
    fn binary16_subnormals_and_sign() {
        assert_eq!(half(1), 2f64.powi(-24));
        assert_eq!(half(0x8001), -2f64.powi(-24));
        assert_eq!(half(0x3c00), 1.);
    }
}
