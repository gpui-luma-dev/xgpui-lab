use super::*;
use gpui_luma::infra::ElementExt;
use gpui::{Pixels, Point, SvgRenderer, PathPromptOptions, px};
use anyhow::{Context as _, ensure};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

fn read_svg(path: &Path) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take((32 * MIB + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 32 * MIB, "SVG exceeds the 32 MiB input limit");
    Ok(bytes)
}
impl SvgPage {
    pub(super) fn from_bytes(renderer: &SvgRenderer, bytes: Vec<u8>, source: Option<PathBuf>) -> anyhow::Result<Asset> {
        let start = Instant::now();
        let document = SvgDocument::parse(renderer, &bytes)?;
        let parse_ms = start.elapsed().as_secs_f64() * 1000.;
        let logical = document.fit_size(320.)?;
        let resolution = RasterPolicy::default().resolution(logical, 1., 1.)?;
        let raster = document.rasterize(renderer, resolution.output, 16 * MIB)?;
        Ok(Asset { document, fixed: raster.image, parse_ms, raster_ms: raster.ms, logical, bytes, source })
    }
    pub(super) fn copy_camera(&mut self, cx: &mut Context<Self>) {
        self.values[0] = self.camera.zoom;
        self.values[1] = self.camera.pan.x.into();
        self.values[2] = self.camera.pan.y.into();
        for (parameter, slider) in self.sliders.iter().take(3) {
            slider.update(cx, |slider, cx| slider.set_value(self.values[*parameter as usize], cx));
        }
    }
    pub(super) fn zoom_at(&mut self, anchor: Point<Pixels>, factor: f32, cx: &mut Context<Self>) {
        self.camera.zoom_at(anchor, factor);
        self.drag_position = None;
        self.copy_camera(cx);
        cx.notify();
    }
    pub fn suspend(&mut self) {
        self.drag_position = None;
        self.animate = false;
    }
    pub(super) fn effective_trim(&self) -> f32 {
        if self.animate && self.motion_allowed {
            0.15 + 0.85 * (self.started.elapsed().as_secs_f32() * 0.6).sin().mul_add(0.5, 0.5)
        } else {
            self.values[3]
        }
    }
    pub(super) fn effective_phase(&self) -> f32 {
        self.values[4]
            + if self.animate && self.motion_allowed {
                self.started.elapsed().as_secs_f32() * 24.
            } else {
                0.
            }
    }
    pub(super) fn viewport(&self, index: usize, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        use gpui::prelude::*;
        let owner = cx.entity();
        gpui::div()
            .id(format!("svg-viewport-{index}"))
            .w(px(320.))
            .h(px(260.))
            .flex_shrink_0()
            .overflow_hidden()
            .cursor_crosshair()
            .on_prepaint(move |bounds, _, cx| owner.update(cx, |this, _| this.panel_bounds[index] = bounds))
            .on_scroll_wheel(cx.listener(move |this, event: &gpui::ScrollWheelEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(24.)).y);
                this.zoom_at(
                    event.position - this.panel_bounds[index].center(),
                    (delta * 0.002).clamp(-2., 2.).exp(),
                    cx,
                );
                cx.stop_propagation();
            }))
            .on_pinch(cx.listener(move |this, event: &gpui::PinchEvent, _, cx| {
                this.zoom_at(event.position - this.panel_bounds[index].center(), (1. + event.delta).max(0.01), cx);
                cx.stop_propagation();
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.drag_position = Some(event.position);
                    cx.stop_propagation();
                }),
            )
    }
    pub(super) fn open_svg(&mut self, cx: &mut Context<Self>) {
        self.load_request += 1;
        let request = self.load_request;
        self.loading = true;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open SVG artwork".into()),
        });
        let renderer = cx.svg_renderer();
        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<Option<Asset>> = async {
                let selected = picker.await.context("File picker closed")??;
                let Some(path) = selected.and_then(|paths| paths.into_iter().next()) else {
                    return Ok(None);
                };
                cx.background_spawn(async move { Self::from_bytes(&renderer, read_svg(&path)?, Some(path)) })
                    .await
                    .map(Some)
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                if request != this.load_request {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(Some(asset)) => {
                        this.camera = Default::default();
                        this.camera.scene_size = asset.logical;
                        this.asset = Ok(asset);
                        this.status = None;
                        this.rendering_status = None;
                        this.copy_camera(cx);
                        this.load_error = None;
                    }
                    Ok(None) => (),
                    Err(error) => this.load_error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn open_gallery(&mut self, cx: &mut Context<Self>) {
        self.load_request += 1;
        let request = self.load_request;
        self.loading = true;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open SVG gallery folder".into()),
        });
        let renderer = cx.svg_renderer();
        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<Option<Vec<(String, SvgDocument)>>> = async {
                let selected = picker.await.context("File picker closed")??;
                let Some(directory) = selected.and_then(|paths| paths.into_iter().next()) else {
                    return Ok(None);
                };
                cx.background_spawn(async move {
                    let mut paths = std::fs::read_dir(directory)?
                        .filter_map(Result::ok)
                        .map(|entry| entry.path())
                        .filter(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("svg")))
                        .collect::<Vec<_>>();
                    paths.sort();
                    ensure!(!paths.is_empty(), "No SVG artwork found in the selected folder");
                    let mut docs = Vec::new();
                    for path in paths {
                        let document = SvgDocument::parse(&renderer, &read_svg(&path)?)?;
                        docs.push((path.file_name().unwrap_or_default().to_string_lossy().into_owned(), document));
                    }
                    Ok::<_, anyhow::Error>(docs)
                })
                .await
                .map(Some)
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                if request != this.load_request {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(Some(docs)) => {
                        this.gallery_docs = docs;
                        this.load_error = None;
                    }
                    Ok(None) => (),
                    Err(error) => this.load_error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
