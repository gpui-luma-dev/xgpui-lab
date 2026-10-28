use super::*;
use gpui::{prelude::*, AnyElement, Render, Window, MouseButton, MouseMoveEvent, div, img, px};
use xgpui_media::image_view;
#[cfg(target_os = "macos")]
use xgpui_media::video_frame;
use crate::theme::ColorVizLookExt;

impl Render for MediaPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if cx.reduce_motion() {
            self.sweep = false;
            self.animate = false;
        }
        if (self.sweep || self.animate) && !cx.reduce_motion() {
            window.request_animation_frame();
        }
        if let Some(job) = &mut self.float_job {
            if let Some(image) = job.try_take() {
                self.float_image = Some(image);
                self.float_job = None;
            } else if let LoadStatus::Failed(error) = job.status() {
                self.error = Some(error);
                self.float_job = None;
            }
        }
        if self.float_job.is_some() {
            window.request_animation_frame();
        }
        if self.requires_sdr(cx) && window.color_status().effective() != gpui::WindowColorMode::Default {
            return div().p_4().child("Preparing SDR presentation for media blur…").into_any_element();
        }
        let video = self.active_tab == "Video Frames";
        if !video {
            self.ensure_fixtures(cx);
        }
        self.poll();
        if self.job.is_some() || self.waiting > 0 || !self.burst_jobs.is_empty() || !self.burst_images.is_empty() {
            window.request_animation_frame();
        }
        let actions: &[Action] = match self.active_tab.as_ref() {
            "Video Frames" => &[
                Action::Start,
                Action::Restart,
                Action::Format,
                Action::Pause,
                Action::VideoBurst,
                Action::Animate,
                Action::Blur,
                Action::Quality,
                Action::Reset,
            ],
            "Resource Limits" => {
                &[Action::Open, Action::Sample, Action::Reload, Action::Burst, Action::Jobs, Action::Budget]
            }
            "Tiled Canvas" => &[],
            "Float / P3" => &[Action::Animate, Action::Wide, Action::Reset],
            "Linear Adjustments" => {
                &[Action::Open, Action::Sample, Action::Reload, Action::Animate, Action::Filters, Action::Reset]
            }
            "Image Filters" => {
                &[Action::Open, Action::Sample, Action::Reload, Action::Filters, Action::Tint, Action::Reset]
            }
            "Textures & Mipmaps" => &[Action::Open, Action::Sample, Action::Reload, Action::Sweep, Action::Reset],
            _ => &[Action::Open, Action::Sample, Action::Reload, Action::Reset],
        };
        let parameters: &[Parameter] = match self.active_tab.as_ref() {
            "Tiled Canvas" | "Resource Limits" => &[],
            "Video Frames" => &[
                Parameter::Opacity,
                Parameter::Radius,
                Parameter::Exposure,
                Parameter::LinearContrast,
                Parameter::LinearSaturation,
                Parameter::Vignette,
            ],
            "Linear Adjustments" | "Float / P3" => &[
                Parameter::Zoom,
                Parameter::PanX,
                Parameter::PanY,
                Parameter::Opacity,
                Parameter::Radius,
                Parameter::Exposure,
                Parameter::LinearContrast,
                Parameter::LinearSaturation,
                Parameter::Vignette,
            ],
            "Image Filters" => &[
                Parameter::Brightness,
                Parameter::Contrast,
                Parameter::Saturation,
                Parameter::Hue,
                Parameter::Opacity,
                Parameter::Radius,
            ],
            _ => &[Parameter::Zoom, Parameter::PanX, Parameter::PanY, Parameter::Opacity, Parameter::Radius],
        };
        let mut root = div()
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    this.drag = None;
                } else if let Some((anchor, pan)) = this.drag {
                    let delta = event.position - anchor;
                    this.values[1] = pan[0] + f32::from(delta.x);
                    this.values[2] = pan[1] + f32::from(delta.y);
                    this.sync_camera(cx);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.drag = None))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, _| this.drag = None))
            .id("media-lab-scroll")
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .text_color(self.look.chrome().body_text)
            .child(
                div().flex().flex_wrap().gap_2().children(
                    self.buttons
                        .iter()
                        .filter(|(action, _)| {
                            actions.contains(action)
                                && (!matches!(action, Action::Pause | Action::VideoBurst) || self.has_video())
                        })
                        .map(|(_, button)| button.clone()),
                ),
            )
            .child(div().flex().flex_wrap().gap_4().children(
                self.sliders.iter().filter(|(parameter, _)| parameters.contains(parameter)).map(
                    |(parameter, slider)| {
                        div()
                            .w(px(170.))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(format!("{}: {:.2}", parameter.config().0, self.values[*parameter as usize]))
                            .child(slider.clone())
                    },
                ),
            ))
            .children(self.error.as_ref().map(|error| div().child(format!("Media error: {error}"))));
        if video {
            return root.child(self.video_view(window)).into_any_element();
        }
        root = root.child(format!(
            "Image mode: {} · Sweep {} · Adjustment animation {}",
            ["Atlas / mipmap", "Linear sRGB adjustments", "RGBA16F Display P3", "Progressive tiled canvas"][self.mode],
            self.sweep && !cx.reduce_motion(),
            self.animate
        ));
        if self.mode == 3 {
            return root.child(div().h(px(850.)).w_full().children(self.tiled.clone())).into_any_element();
        }
        if self.mode == 2 {
            return root.child(self.float_view(window, cx)).into_any_element();
        }
        if let Some(fixtures) = &self.fixtures {
            let sample = &fixtures.samples[self.selected];
            root = root.child(format!("{} · {}×{} · {}", sample.name, sample.width, sample.height, sample.description));
        } else {
            root = root.child("Generating offline media-lab image fixtures in a background task…");
        }
        if let Some(image) = &self.image {
            root = root.child(format!(
                "Upload {:?} · {} upload(s) · {} mip levels · {:.2} MiB source payload",
                image.upload_status(),
                image.upload_count(),
                image.mip_count(),
                image.payload_bytes() as f64 / MIB as f64
            ));
        } else if let Some(job) = &self.job {
            root = root.child(format!("Image load: {:?}", job.status()));
        }
        if self.active_tab == "Image Filters" {
            root = root.child(format!(
                "Tint preset: {} · Filters affect the media texture only",
                ["Identity", "Warm", "Cool"][self.tint_index]
            ));
        }
        if self.active_tab == "Resource Limits" {
            root = root
                .child(format!(
                    "Decode/upload jobs: {} · Source/staging budget: {} MiB · Per-image reservation: 16 MiB",
                    self.job_limit,
                    [32, 64, 128][self.budget_index]
                ))
                .child(format!(
                    "Burst waiting {} · Decoding {} · Preparing {} · Retired {}",
                    self.waiting,
                    self.burst_jobs.len(),
                    self.burst_images.len(),
                    self.completed
                ));
            if let Ok(loader) = &self.loader {
                root = root
                    .child(format!("Upload admission: {:?}", loader.upload_stats()))
                    .child(format!("Staging pool: {:?}", loader.staging_pool_stats()));
            }
            root=root.child("Completed burst snapshots retire to release their source reservations. READY means host submission, not GPU completion. These budgets do not bound total RSS or VRAM.")
                .child(div().flex().flex_wrap().gap_2().children(self.burst_images.iter().map(|image|image_view(image.clone()).w(px(110.)).h(px(80.)))));
        }
        if self.active_tab == "Textures & Mipmaps"
            && let Some(image) = &self.image
        {
            let sample = &self.fixtures.as_ref().unwrap().samples[self.selected];
            let fit = (360. / sample.width as f32).min(260. / sample.height as f32) * self.values[0];
            let device_ratio = fit * window.scale_factor();
            let lod = (-device_ratio.log2()).max(0.).min(image.mip_count().saturating_sub(1) as f32);
            root=root.child(format!("Sampling probe · {:.4} device pixels / source pixel · estimated LOD {:.2} · {} retained mip levels",device_ratio,lod,image.mip_count()))
                    .child("Wheel/pinch and sweep cross mip transitions. Left uses the GPUI image atlas; right uses the dedicated trilinear media texture. LOD is a footprint estimate, not a forced GPU mip selection.");
            for level in 0..image.mip_count() {
                root = root.child(format!(
                    "Mip {level}: {}×{}",
                    (image.width() >> level).max(1),
                    (image.height() >> level).max(1)
                ));
            }
        }
        if self.active_tab == "Resource Limits" {
            return root
                .children(self.image.as_ref().map(|image| image_view(image.clone()).w(px(220.)).h(px(160.))))
                .into_any_element();
        }
        root.child(self.comparison(cx)).into_any_element()
    }
}

impl MediaPage {
    fn comparison(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut root = div().flex().flex_wrap().gap_4();
        let Some(fixtures) = &self.fixtures else {
            return root.into_any_element();
        };
        let sample = &fixtures.samples[self.selected];
        let width = 360.;
        let height = 260.;
        let zoom = self.values[0]
            * if self.sweep && !cx.reduce_motion() {
                1. + 0.1 * (self.started.elapsed().as_secs_f32() * 0.9).sin()
            } else {
                1.
            };
        let fit = (width / sample.width as f32).min(height / sample.height as f32) * zoom;
        let w = sample.width as f32 * fit;
        let h = sample.height as f32 * fit;
        let x = (width - w) * 0.5 + self.values[1];
        let y = (height - h) * 0.5 + self.values[2];
        let placements: &[bool] = if self.active_tab == "Images" {
            &[true]
        } else {
            &[false, true]
        };
        for &native in placements {
            let mut viewport = self.camera_viewport(usize::from(native), width, height, cx);
            if native || self.mode == 1 {
                if let Some(image) = &self.image {
                    let filter = if self.active_tab == "Image Filters" {
                        self.filter()
                    } else {
                        ImageFilter::new()
                    };
                    let view = image_view(image.clone());
                    let view = if native && self.mode == 1 {
                        match self.adjustments(xgpui_media::pipeline::ImagePrimaries::Srgb) {
                            Ok(adjustments) => view.adjustments(adjustments),
                            Err(_) => view,
                        }
                    } else {
                        view.filter(filter)
                    };
                    viewport = viewport.child(
                        view.absolute()
                            .left(px(x))
                            .top(px(y))
                            .w(px(w))
                            .h(px(h))
                            .rounded(px(self.values[4]))
                            .opacity(self.values[3]),
                    );
                }
            } else {
                viewport = viewport.child(
                    img(sample.path.clone())
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(w))
                        .h(px(h))
                        .rounded(px(self.values[4]))
                        .opacity(self.values[3]),
                );
            }
            root = root.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(if self.active_tab == "Images" {
                        "Image inspection · retained decoded source"
                    } else if native && self.mode == 1 {
                        "Linear adjustments · exposure / contrast / saturation / vignette"
                    } else if native {
                        "Dedicated mipmapped media texture"
                    } else if self.mode == 1 {
                        "Unadjusted retained media source"
                    } else {
                        "GPUI image atlas baseline"
                    })
                    .child(viewport),
            );
        }
        root.into_any_element()
    }

    fn video_view(&mut self, window: &mut Window) -> AnyElement {
        use xgpui_media::pipeline::ImagePrimaries;
        let mut root = div()
            .flex()
            .flex_col()
            .gap_3()
            .child("Generated native video · adjusted preview and original reference");
        #[cfg(target_os = "macos")]
        {
            if self.video.is_none() && !self.video_stopped {
                match super::video_source::Demo::new(self.format) {
                    Ok(video) => self.video = Some(video),
                    Err(error) => {
                        self.error = Some(error.to_string());
                        self.video_stopped = true;
                    }
                }
            }
            root = root.child(format!(
                "Format: {} · Paused {} · Stopped {} · Blur {} px · {} quality",
                self.format.label(),
                self.video_paused,
                self.video_stopped,
                [0, 4, 8, 16][self.blur_index],
                if self.full_quality { "full" } else { "adaptive" }
            ));
            if let Some(video) = &self.video {
                let frame = video.stream.snapshot();
                let stats = video.stream.stats();
                if stats.playing && !stats.closed {
                    window.request_animation_frame();
                }
                root = root
                    .child(format!(
                        "Mailbox: {:?} · Timestamp {:.3} s",
                        stats,
                        frame.as_ref().map_or(0., |frame| frame.timestamp().as_secs_f64())
                    ))
                    .children(video.error().map(|error| div().child(format!("Source error: {error}"))));
                let width = (1020. / window.scale_factor()
                    - 2. * (16. + 8. / window.scale_factor()) * gpui::MAX_LAYER_GROUPS as f32)
                    .clamp(80., 640.)
                    .min(((f32::from(window.viewport_size().width) - 160.) / 2.).max(80.));
                if let (Ok(adjustments), Ok(blur)) = (self.adjustments(ImagePrimaries::Srgb), self.blur()) {
                    root = root.child(div().flex().flex_wrap().items_start().gap_4().children([true, false].map(
                        |adjusted| {
                            let mut viewport = div()
                                .relative()
                                .w(px(width))
                                .h(px(width * 9. / 16.))
                                .flex_shrink_0()
                                .overflow_hidden()
                                .rounded(px(self.values[4]))
                                .child(super::advanced::checker());
                            if let Some(frame) = &frame {
                                let view = video_frame(frame.clone());
                                let view = if adjusted {
                                    view.adjustments(adjustments).blur(blur.clone())
                                } else {
                                    view
                                };
                                viewport = viewport
                                    .child(view.size_full().rounded(px(self.values[4])).opacity(self.values[3]));
                            }
                            div()
                                .w(px(width))
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(if adjusted {
                                    "Adjusted preview"
                                } else {
                                    "Original reference"
                                })
                                .child(viewport)
                                .child(if adjusted {
                                    "Linear exposure, contrast, saturation, vignette and bounded blur"
                                } else {
                                    "Same immutable frame and timestamp, without adjustments"
                                })
                        },
                    )));
                }
            }
            root = root.children(self.error.as_ref().map(|error| div().child(format!("Video error: {error}"))));
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = window;
            root = root.child("The source lab's generated IOSurface/NV12 source requires native macOS.");
        }
        root.child("Synthetic BGRA/NV12 presentation, not a decoder or A/V clock. Stop retains the last frame; Restart replaces the source.").into_any_element()
    }
}
