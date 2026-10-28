use super::*;
use gpui_luma::infra::ElementExt;
use gpui::{prelude::*, *};
use xgpui_media::pipeline::*;
use anyhow::Context as _;

impl MediaPage {
    pub(super) fn float_view(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let mut root = div()
            .flex()
            .flex_col()
            .gap_3()
            .child("Supplied RGBA16F · generated linear Display P3 pixels")
            .child(format!(
                "Presentation {:?} · Requested native wide {}",
                window.color_status().effective(),
                self.wide
            ));
        if let Some(image) = &self.float_image {
            root = root.child(format!(
                "{}×{} · {:?} · {:?} · {} mip levels · Uploads {} · {:?}",
                image.width(),
                image.height(),
                image.format(),
                image.source_color(),
                image.mip_count(),
                image.upload_count(),
                image.upload_status()
            ));
            if let Ok(adjustments) = self.adjustments(ImagePrimaries::DisplayP3) {
                let wide = self.wide && window.color_status().effective() == WindowColorMode::WideGamutSdr;
                root = root.child(div().flex().flex_wrap().gap_4().children([false, true].map(|right| {
                    let width = 360.;
                    let height = 260.;
                    let zoom = self.values[0];
                    let art_w = width * zoom;
                    let art_h = art_w * 800. / 1280.;
                    let make_view = || {
                        xgpui_media::image_view(image.clone())
                            .adjustments(adjustments)
                            .absolute()
                            .left(px((width - art_w) * 0.5 + self.values[1]))
                            .top(px((height - art_h) * 0.5 + self.values[2]))
                            .w(px(art_w))
                            .h(px(art_h))
                            .rounded(px(self.values[4]))
                            .opacity(self.values[3])
                    };
                    let view = make_view();
                    let mut panel = self.camera_viewport(usize::from(right), width, height, cx);
                    let (view, error) = if right && wide {
                        match view.wide_gamut() {
                            Ok(view) => (Some(view), None),
                            Err(error) => {
                                (Some(make_view()), Some(format!("Wide output unavailable; showing sRGB: {error}")))
                            }
                        }
                    } else {
                        (Some(view), None)
                    };
                    if let Some(view) = view {
                        panel = panel.child(view);
                    }
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(if right && wide && error.is_none() {
                            "Native linear Display P3"
                        } else {
                            "Explicit sRGB preview"
                        })
                        .child(panel)
                        .children(error.map(|error| div().child(error)))
                })));
            }
        } else if let Some(job) = &self.float_job {
            root = root.child(format!("Float source: {:?}", job.status()));
        }
        root.child("Both previews reuse one retained source upload. Wide output falls back to a labeled sRGB preview when window negotiation is unavailable.").into_any_element()
    }

    pub(super) fn camera_viewport(
        &self,
        index: usize,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let owner = cx.entity();
        div()
            .id(format!("media-image-viewport-{index}"))
            .relative()
            .w(px(width))
            .h(px(height))
            .flex_shrink_0()
            .overflow_hidden()
            .cursor_crosshair()
            .child(super::advanced::checker())
            .on_prepaint(move |bounds, _, cx| {
                owner.update(cx, |this, _| this.bounds[index] = bounds);
            })
            .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(24.)).y);
                this.zoom_at(event.position - this.bounds[index].center(), (delta * 0.002).clamp(-1., 1.).exp(), cx);
                cx.stop_propagation();
            }))
            .on_pinch(cx.listener(move |this, event: &PinchEvent, _, cx| {
                this.zoom_at(event.position - this.bounds[index].center(), (1. + event.delta).max(0.01), cx);
                cx.stop_propagation();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.drag = Some((event.position, [this.values[1], this.values[2]]));
                    this.sweep = false;
                    cx.stop_propagation();
                }),
            )
    }
    pub(super) fn sync_camera(&self, cx: &mut Context<Self>) {
        for (parameter, slider) in self.sliders.iter().take(3) {
            slider.update(cx, |slider, cx| slider.set_value(self.values[*parameter as usize], cx));
        }
    }
    pub(super) fn zoom_at(&mut self, anchor: Point<Pixels>, factor: f32, cx: &mut Context<Self>) {
        let old = self.values[0];
        let next = (old * factor).clamp(0.1, 4.);
        let pan = point(px(self.values[1]), px(self.values[2]));
        let pan = anchor - (anchor - pan) * (next / old);
        self.values[0] = next;
        self.values[1] = pan.x.into();
        self.values[2] = pan.y.into();
        self.sweep = false;
        self.drag = None;
        self.sync_camera(cx);
        cx.notify();
    }
    pub(super) fn adjustments(&self, primaries: ImagePrimaries) -> anyhow::Result<ImageAdjustments> {
        let ev = self.values[9] as f64
            + if self.animate {
                (self.started.elapsed().as_secs_f64() * 1.5).sin() * 0.8
            } else {
                0.
            };
        ImageAdjustments::new(
            ColorMatrix::identity(primaries)
                .exposure(ev)?
                .contrast(self.values[10] as f64, 0.18)?
                .saturation(self.values[11] as f64)?,
            Some(Vignette::new(self.values[12] as f64, 0.35, 0.65, 1.)?),
        )
    }
    #[cfg(target_os = "macos")]
    pub(super) fn blur(&self) -> anyhow::Result<ImageBlur> {
        Ok(ImageBlur::new(px([0., 4., 8., 16.][self.blur_index]))?.with_quality(if self.full_quality {
            LayerBlurQuality::FullResolution
        } else {
            LayerBlurQuality::Adaptive
        }))
    }
    pub(super) fn load_float(&mut self) {
        if self.float_image.is_some() || self.float_job.is_some() {
            return;
        }
        if let Ok(loader) = &self.float_loader {
            match loader.load_rgba16f(1280, 800, pixels(1280, 800), ImageColor::LINEAR_P3) {
                Ok(job) => self.float_job = Some(job),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
    pub(super) fn open_image(&mut self, cx: &mut Context<Self>) {
        self.import_request += 1;
        let request = self.import_request;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open PNG, JPEG or WebP".into()),
        });
        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<Option<Fixtures>> = async {
                let selected = picker.await.context("File picker closed")??;
                let Some(path) = selected.and_then(|paths| paths.into_iter().next()) else {
                    return Ok(None);
                };
                cx.background_spawn(async move { Fixtures::new(Some(path)) }).await.map(Some)
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                if request != this.import_request {
                    return;
                }
                match result {
                    Ok(Some(fixtures)) => {
                        this.selected = fixtures.samples.len() - 1;
                        this.fixtures = Some(fixtures);
                        this.reload();
                    }
                    Ok(None) => (),
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn pixels(width: u32, height: u32) -> Vec<[f32; 4]> {
    (0..width * height)
        .map(|i| {
            let u = (i % width) as f32 / (width - 1) as f32;
            let v = (i / width) as f32 / (height - 1) as f32;
            let alpha = ((0.49 - (u - 0.5).abs().max((v - 0.5).abs())) * 60.).clamp(0., 1.);
            let rgb = if v < 0.33 {
                [1. - u, u, 0.02]
            } else if v < 0.66 {
                [u * 0.12, u * 0.12, u * 0.12]
            } else {
                [0.05, 1. - u, u]
            };
            [rgb[0], rgb[1], rgb[2], alpha]
        })
        .collect()
}

pub(crate) fn checker() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, (), window, _| {
            let tile = 24.;
            for y in 0..(f32::from(bounds.size.height) / tile).ceil() as i32 {
                for x in 0..(f32::from(bounds.size.width) / tile).ceil() as i32 {
                    let origin = bounds.origin + point(px(x as f32 * tile), px(y as f32 * tile));
                    let rect = Bounds::new(
                        origin,
                        size(px(tile).min(bounds.right() - origin.x), px(tile).min(bounds.bottom() - origin.y)),
                    );
                    window.paint_quad(fill(rect, rgb(if (x + y) % 2 == 0 { 0x20282b } else { 0x252e31 })));
                }
            }
        },
    )
    .absolute()
    .size_full()
}
