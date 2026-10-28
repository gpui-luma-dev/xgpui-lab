//! SV triangle composition — hue ring with Photoshop-style saturation/value triangle.

use std::sync::Arc;

use gpui::{Context, Entity, Hsla, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma_color::color_field::model::ColorFieldModelKind;
use gpui_luma_color::color_field::{ColorFieldEvent, ColorFieldModel2D, ColorFieldState, TriangleDomain};
use gpui_luma_color::color_ring::{ColorRingBuilder, primary_slider_value};
use gpui_luma_color::color_slider::color_spec::Hsv;
use gpui_luma_color::composition::ColorCompositionSync;
use gpui_luma::controls::slider::SliderControl;
use crate::theme::{Look, TextSize};

use super::super::color_exposition_common::{
    composition_demo_card_width, render_composition_readout_footer, COMPOSITION_PRIMARY_READOUT_GAP,
};

pub struct SvTriangleDemo {
    look: Arc<Look>,
    metrics: SvTriangleMetrics,
    sync: ColorCompositionSync,
    hsv: Hsv,
    ring: Entity<SliderControl>,
    triangle: Entity<ColorFieldState>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy)]
struct SvTriangleMetrics {
    ring_size: f32,
    ring_thickness: f32,
    thumb_size: f32,
    inner_gap: f32,
}

impl SvTriangleMetrics {
    fn new() -> Self {
        let ring_size: f32 = 238.0;
        let scale = ring_size / 300.0;
        Self {
            ring_size,
            ring_thickness: 20.0 * scale,
            thumb_size: (14.0 * scale).max(12.0),
            inner_gap: (2.0 * scale).max(2.0),
        }
    }

    fn triangle_size(self) -> f32 {
        let inner = (self.ring_size - 2.0 * self.ring_thickness).max(0.0);
        (inner - self.inner_gap).max(40.0)
    }

    fn card_width(self) -> f32 {
        composition_demo_card_width(self.ring_size)
    }
}

impl SvTriangleDemo {
    pub fn card_width() -> f32 {
        SvTriangleMetrics::new().card_width()
    }

    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let initial_hsv = Hsv { h: 317.0, s: 0.83, v: 0.84, a: 1.0 };
        let metrics = SvTriangleMetrics::new();

        let ring = ColorRingBuilder::hue("controls-doc-sv-triangle-ring", initial_hsv.h, 1.0, 0.5)
            .size(px(metrics.ring_size))
            .ring_thickness(metrics.ring_thickness)
            .thumb_size(metrics.thumb_size)
            .spawn(cx);

        let triangle = cx.new(|_| {
            let (white, black, hue) = sv_triangle_vertices();
            ColorFieldState::new(
                "controls-doc-sv-triangle-field",
                initial_hsv,
                Arc::new(TriangleDomain { a: white, b: black, c: hue }),
                Arc::new(PhotoshopSvTriangleModel),
            )
            .thumb_size(metrics.thumb_size)
            .raster_image_prewarmed_square(metrics.triangle_size())
            .rounded(px(0.0))
            .no_border()
            .edge_to_edge()
        });

        let subscriptions = vec![
            cx.subscribe(&ring, |this, _, event, cx| {
                let Some(hue) = primary_slider_value(event) else {
                    return;
                };
                let Some(_sync_guard) = this.sync.begin_guard() else {
                    return;
                };
                this.hsv.h = hue;
                this.sync_controls(cx);
                cx.notify();
            }),
            cx.subscribe(&triangle, |this, _, event: &ColorFieldEvent, cx| {
                let hsv = match event {
                    ColorFieldEvent::Change(hsv) | ColorFieldEvent::Release(hsv) => *hsv,
                    _ => return,
                };
                let Some(_sync_guard) = this.sync.begin_guard() else {
                    return;
                };
                this.hsv.s = hsv.s;
                this.hsv.v = hsv.v;
                this.sync_controls(cx);
                cx.notify();
            }),
        ];

        Self {
            look,
            metrics,
            sync: ColorCompositionSync::new(),
            hsv: initial_hsv,
            ring,
            triangle,
            _subscriptions: subscriptions,
        }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look;
        self.ring.update(cx, |_, cx| cx.notify());
        self.triangle.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn ring(&self) -> Entity<SliderControl> {
        self.ring.clone()
    }

    pub fn triangle(&self) -> Entity<ColorFieldState> {
        self.triangle.clone()
    }

    fn sync_controls(&self, cx: &mut Context<Self>) {
        let hsv = self.hsv;
        self.sync.sync_slider_value(&self.ring, hsv.h, cx);
        self.triangle.update(cx, |field, cx| {
            field.set_hsv_components(hsv.h, hsv.s, hsv.v, cx);
        });
    }
}

impl Render for SvTriangleDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let ring_outer_size = self.metrics.ring_size;
        let triangle_size = self.metrics.triangle_size();
        let triangle_half = triangle_size * 0.5;
        let swatch = self.hsv.to_hsla_ext();
        let look = &self.look;
        let text_size = TextSize::Base;

        div()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(COMPOSITION_PRIMARY_READOUT_GAP))
            .child(
                div()
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
                    ),
            )
            .child(render_composition_readout_footer(look, swatch, text_size, Some(ring_outer_size)))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PhotoshopSvTriangleModel;

impl ColorFieldModel2D for PhotoshopSvTriangleModel {
    fn apply_uv(&self, hsv: &mut Hsv, uv: (f32, f32)) {
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

    fn uv_from_hsv(&self, hsv: &Hsv) -> (f32, f32) {
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

    fn color_at_uv(&self, hsv: &Hsv, uv: (f32, f32)) -> Hsla {
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

        Hsv { h: hsv.h, s: saturation, v: value, a: 1.0 }.to_hsla_ext()
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
