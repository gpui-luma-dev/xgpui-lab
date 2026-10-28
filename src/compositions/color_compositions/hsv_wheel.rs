//! HSV wheel composition — hue ring with centered saturation/value square.

use std::f32::consts::SQRT_2;
use std::sync::Arc;

use gpui::{Context, Entity, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma_color::color_field::{ColorFieldEvent, ColorFieldState};
use gpui_luma_color::color_ring::{ColorRingBuilder, primary_slider_value};
use gpui_luma_color::color_slider::color_spec::Hsv;
use gpui_luma_color::composition::ColorCompositionSync;
use gpui_luma::controls::slider::SliderControl;
use crate::theme::{Look, TextSize};

use super::super::color_exposition_common::{
    composition_demo_card_width, render_composition_readout_footer, COMPOSITION_PRIMARY_READOUT_GAP,
};

pub struct HsvWheelDemo {
    look: Arc<Look>,
    metrics: HsvWheelMetrics,
    sync: ColorCompositionSync,
    color_ring: Entity<SliderControl>,
    plane_sv: Entity<ColorFieldState>,
    hsv: Hsv,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy)]
struct HsvWheelMetrics {
    ring_outer_size: f32,
    ring_thickness: f32,
    ring_thumb_size: f32,
    inner_gap: f32,
}

impl HsvWheelMetrics {
    fn new() -> Self {
        let ring_outer_size: f32 = 238.0;
        let scale = ring_outer_size / 300.0;
        Self {
            ring_outer_size,
            ring_thickness: 20.0 * scale,
            ring_thumb_size: (16.0 * scale).max(12.0),
            inner_gap: (8.0 * scale).max(6.0),
        }
    }

    fn plane_size(self) -> f32 {
        let ring_inner_diameter = (self.ring_outer_size - 2.0 * self.ring_thickness).max(0.0);
        (ring_inner_diameter / SQRT_2 - self.inner_gap).max(40.0)
    }

    fn card_width(self) -> f32 {
        composition_demo_card_width(self.ring_outer_size)
    }
}

impl HsvWheelDemo {
    pub fn card_width() -> f32 {
        HsvWheelMetrics::new().card_width()
    }

    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let hsv = Hsv { h: 220.0, s: 0.88, v: 0.6, a: 1.0 };
        let metrics = HsvWheelMetrics::new();

        let color_ring = ColorRingBuilder::hue("controls-doc-hsv-wheel-ring", hsv.h, 1.0, 0.5)
            .size(px(metrics.ring_outer_size))
            .ring_thickness(metrics.ring_thickness)
            .thumb_size(metrics.ring_thumb_size)
            .spawn(cx);
        let plane_sv = cx.new(|_| {
            ColorFieldState::saturation_value("controls-doc-hsv-wheel-plane", hsv, metrics.ring_thumb_size)
                .rounded(px(0.0))
                .no_border()
                .edge_to_edge()
        });

        let subscriptions = vec![
            cx.subscribe(&color_ring, |this, _, event, cx| {
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
            cx.subscribe(&plane_sv, |this, _, event: &ColorFieldEvent, cx| {
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
            color_ring,
            plane_sv,
            hsv,
            _subscriptions: subscriptions,
        }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look;
        self.color_ring.update(cx, |_, cx| cx.notify());
        self.plane_sv.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn color_ring(&self) -> Entity<SliderControl> {
        self.color_ring.clone()
    }

    pub fn plane_sv(&self) -> Entity<ColorFieldState> {
        self.plane_sv.clone()
    }

    fn sync_controls(&self, cx: &mut Context<Self>) {
        let hsv = self.hsv;
        self.sync.sync_slider_value(&self.color_ring, hsv.h, cx);
        self.plane_sv.update(cx, |plane, cx| {
            plane.set_hsv_components(hsv.h, hsv.s, hsv.v, cx);
        });
    }
}

impl Render for HsvWheelDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let ring_outer_size = self.metrics.ring_outer_size;
        let plane_size = self.metrics.plane_size();
        let plane_half = plane_size / 2.0;
        let color = self.hsv.to_hsla_ext();
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
                    .child(self.color_ring.clone())
                    .child(
                        div()
                            .absolute()
                            .top_1_2()
                            .left_1_2()
                            .mt(px(-plane_half))
                            .ml(px(-plane_half))
                            .size(px(plane_size))
                            .child(self.plane_sv.clone()),
                    ),
            )
            .child(render_composition_readout_footer(look, color, text_size, Some(ring_outer_size)))
    }
}
