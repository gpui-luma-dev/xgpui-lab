use std::sync::Arc;

use gpui::{Context, Entity, Hsla, Render, Subscription, div, prelude::*, px};
use gpui_luma_color::composition::ColorCompositionSync;
use gpui_luma_color::color_field::{ColorFieldEvent, ColorFieldModel2D, ColorFieldState, TriangleDomain};
use gpui_luma_color::color_field::model::ColorFieldModelKind;
use gpui_luma_color::color_ring::{ColorRingBuilder, primary_slider_value};
use gpui_luma_color::color_slider::color_spec::Hsv as SdkHsv;
use gpui_luma::controls::slider::{SliderControl, SliderEvent};

use super::color::{hsla_to_sdk_hsv, sdk_hsv_to_hsla};

pub struct SvTrianglePicker {
    metrics: SvTrianglePickerMetrics,
    sync: ColorCompositionSync,
    hsv: SdkHsv,
    ring: Entity<SliderControl>,
    triangle: Entity<ColorFieldState>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy)]
struct SvTrianglePickerMetrics {
    ring_size: f32,
    ring_thickness: f32,
    thumb_size: f32,
    inner_gap: f32,
}

impl SvTrianglePickerMetrics {
    fn new() -> Self {
        let ring_size: f32 = 174.0;
        let scale = ring_size / 220.0;
        Self {
            ring_size,
            ring_thickness: 20.0 * scale,
            thumb_size: (12.0 * scale).max(10.0),
            inner_gap: (2.0 * scale).max(2.0),
        }
    }

    fn triangle_size(self) -> f32 {
        let inner = (self.ring_size - 2.0 * self.ring_thickness).max(0.0);
        (inner - self.inner_gap).max(36.0)
    }
}

impl SvTrianglePicker {
    pub fn new(initial: Hsla, cx: &mut Context<Self>) -> Self {
        let initial_hsv = hsla_to_sdk_hsv(initial);
        let metrics = SvTrianglePickerMetrics::new();

        let ring = ColorRingBuilder::hue("color-viz-sv-picker-ring", initial_hsv.h, 1.0, 0.5)
            .size(px(metrics.ring_size))
            .ring_thickness(metrics.ring_thickness)
            .thumb_size(metrics.thumb_size)
            .spawn(cx);

        let triangle = cx.new(|_| {
            let (white, black, hue) = sv_triangle_vertices();
            ColorFieldState::new(
                "color-viz-sv-picker-triangle",
                initial_hsv,
                Arc::new(TriangleDomain { a: white, b: black, c: hue }),
                Arc::new(PhotoshopSvTriangleModel),
            )
            .thumb_size(metrics.thumb_size)
            .raster_image()
            .rounded(px(0.0))
            .no_border()
            .edge_to_edge()
        });

        let mut picker = Self {
            metrics,
            sync: ColorCompositionSync::new(),
            hsv: initial_hsv,
            ring: ring.clone(),
            triangle: triangle.clone(),
            _subscriptions: Vec::new(),
        };
        picker.wire_internal(cx, ring, triangle);
        picker
    }

    #[allow(dead_code)]
    pub fn ring(&self) -> Entity<SliderControl> {
        self.ring.clone()
    }

    #[allow(dead_code)]
    pub fn triangle(&self) -> Entity<ColorFieldState> {
        self.triangle.clone()
    }

    pub fn color(&self) -> Hsla {
        sdk_hsv_to_hsla(self.hsv)
    }

    pub fn set_color(&mut self, color: Hsla, cx: &mut Context<Self>) {
        let Some(_sync_guard) = self.sync.begin_guard() else {
            return;
        };
        self.hsv = hsla_to_sdk_hsv(color);
        self.sync_controls(cx);
        cx.notify();
    }

    fn wire_internal(
        &mut self,
        cx: &mut Context<Self>,
        ring: Entity<SliderControl>,
        triangle: Entity<ColorFieldState>,
    ) {
        self._subscriptions.push(cx.subscribe(&ring, |this, _, event: &SliderEvent, cx| {
            let Some(hue) = primary_slider_value(event) else {
                return;
            };
            let Some(_sync_guard) = this.sync.begin_guard() else {
                return;
            };
            this.hsv.h = hue;
            this.sync_controls(cx);
            cx.notify();
        }));
        self._subscriptions.push(cx.subscribe(&triangle, |this, _, event: &ColorFieldEvent, cx| {
            let hsv = match event {
                ColorFieldEvent::Change(hsv) | ColorFieldEvent::Release(hsv) => *hsv,
                _ => return,
            };
            let Some(_sync_guard) = this.sync.begin_guard() else {
                return;
            };
            this.hsv.s = hsv.s;
            this.hsv.v = hsv.v;
            cx.notify();
        }));
    }

    fn sync_controls(&self, cx: &mut Context<Self>) {
        self.sync.sync_slider_value(&self.ring, self.hsv.h, cx);
        self.triangle.update(cx, |field, cx| {
            field.set_hsv_components(self.hsv.h, self.hsv.s, self.hsv.v, cx);
        });
    }
}

impl Render for SvTrianglePicker {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let ring_outer_size = self.metrics.ring_size;
        let triangle_size = self.metrics.triangle_size();
        let triangle_half = triangle_size * 0.5;

        div()
            .id("color-viz-sv-picker")
            .size(px(ring_outer_size))
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .child(self.ring.clone())
            .child(
                div()
                    .absolute()
                    .left_1_2()
                    .top_1_2()
                    .ml(px(-triangle_half))
                    .mt(px(-triangle_half))
                    .size(px(triangle_size))
                    .child(self.triangle.clone()),
            )
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct PhotoshopSvTriangleModel;

impl ColorFieldModel2D for PhotoshopSvTriangleModel {
    fn apply_uv(&self, hsv: &mut SdkHsv, uv: (f32, f32)) {
        let x = uv.0.clamp(0.0, 1.0);
        let y = uv.1.clamp(0.0, 1.0);
        let (white, black, hue) = sv_triangle_vertices();
        let (_, black_weight, hue_weight) = barycentric((x, y), white, black, hue);
        let value = (1.0 - black_weight.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        let saturation = if value <= f32::EPSILON {
            0.0
        } else {
            (hue_weight.clamp(0.0, 1.0) / value).clamp(0.0, 1.0)
        };

        hsv.s = saturation;
        hsv.v = value;
    }

    fn uv_from_hsv(&self, hsv: &SdkHsv) -> (f32, f32) {
        let saturation = hsv.s.clamp(0.0, 1.0);
        let value = hsv.v.clamp(0.0, 1.0);
        let hue_weight = (saturation * value).clamp(0.0, 1.0);
        let black_weight = (1.0 - value).clamp(0.0, 1.0);
        let white_weight = (value - hue_weight).clamp(0.0, 1.0);
        let (white, black, hue) = sv_triangle_vertices();

        (
            (white.0 * white_weight + black.0 * black_weight + hue.0 * hue_weight).clamp(0.0, 1.0),
            (white.1 * white_weight + black.1 * black_weight + hue.1 * hue_weight).clamp(0.0, 1.0),
        )
    }

    fn color_at_uv(&self, hsv: &SdkHsv, uv: (f32, f32)) -> Hsla {
        let x = uv.0.clamp(0.0, 1.0);
        let y = uv.1.clamp(0.0, 1.0);
        let (white, black, hue) = sv_triangle_vertices();
        let (_, black_weight, hue_weight) = barycentric((x, y), white, black, hue);
        let value = (1.0 - black_weight.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        let saturation = if value <= f32::EPSILON {
            0.0
        } else {
            (hue_weight.clamp(0.0, 1.0) / value).clamp(0.0, 1.0)
        };

        sdk_hsv_to_hsla(SdkHsv { h: hsv.h, s: saturation, v: value, a: 1.0 })
    }

    fn kind(&self) -> ColorFieldModelKind {
        ColorFieldModelKind::SvAtHue
    }
}

fn sv_triangle_vertices() -> ((f32, f32), (f32, f32), (f32, f32)) {
    ((0.25, 0.066_987_3), (0.25, 0.933_012_7), (1.0, 0.5))
}

fn barycentric(point: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> (f32, f32, f32) {
    let denom = (b.1 - c.1) * (a.0 - c.0) + (c.0 - b.0) * (a.1 - c.1);
    if denom.abs() <= f32::EPSILON {
        return (0.0, 0.0, 0.0);
    }

    let wa = ((b.1 - c.1) * (point.0 - c.0) + (c.0 - b.0) * (point.1 - c.1)) / denom;
    let wb = ((c.1 - a.1) * (point.0 - c.0) + (a.0 - c.0) * (point.1 - c.1)) / denom;
    let wc = 1.0 - wa - wb;
    (wa, wb, wc)
}
