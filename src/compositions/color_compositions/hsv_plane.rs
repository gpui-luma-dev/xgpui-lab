//! HSV plane composition — Photoshop-style HSV plane with H, S, and V sliders.

use std::sync::Arc;

use gpui::{Context, Entity, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma_color::color_field::{ColorFieldEvent, ColorFieldState};
use gpui_luma_color::color_slider::color_spec::Hsv;
use gpui_luma_color::color_slider::{ChannelDelegate, ColorSliderBuilder, ColorSliderDomainRenderer, primary_slider_value};
use gpui_luma_color::composition::ColorCompositionSync;
use gpui_luma::controls::slider::SliderControl;
use gpui_luma::theme::ControlSize;
use crate::theme::{Look, TextSize};

use super::super::color_exposition_common::{
    composition_demo_card_width, render_composition_readout_footer, slider_labeled_row_compact_sized,
    COMPOSITION_PRIMARY_READOUT_GAP,
};

pub struct HsvPlaneDemo {
    look: Arc<Look>,
    metrics: HsvPlaneMetrics,
    sync: ColorCompositionSync,
    hsv: Hsv,
    plane: Entity<ColorFieldState>,
    slider_h: Entity<SliderControl>,
    slider_s: Entity<SliderControl>,
    slider_v: Entity<SliderControl>,
    slider_s_domain: Arc<ColorSliderDomainRenderer>,
    slider_v_domain: Arc<ColorSliderDomainRenderer>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy)]
struct HsvPlaneMetrics {
    plane_size: f32,
}

impl HsvPlaneMetrics {
    fn new() -> Self {
        Self { plane_size: 228.0 }
    }

    fn card_width(self) -> f32 {
        composition_demo_card_width(self.plane_size)
    }
}

impl HsvPlaneDemo {
    pub fn card_width() -> f32 {
        HsvPlaneMetrics::new().card_width()
    }

    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let initial_hsv = Hsv { h: 266.0, s: 0.78, v: 0.76, a: 1.0 };
        let metrics = HsvPlaneMetrics::new();

        let plane = cx.new(|_| {
            ColorFieldState::hue_saturation_value("controls-doc-hsv-plane", initial_hsv, 20.0)
                .raster_image_prewarmed_square(metrics.plane_size)
                .rounded(px(0.0))
        });
        let slider_h = ColorSliderBuilder::hue("controls-doc-hsv-plane-h", initial_hsv.h)
            .horizontal()
            .rounded(px(0.0))
            .thumb_small()
            .thumb_square()
            .size(ControlSize::Sm)
            .spawn(cx);
        let slider_s_builder =
            ColorSliderBuilder::channel("controls-doc-hsv-plane-s", initial_hsv.s, initial_hsv, Hsv::SATURATION)
                .expect("HSV saturation delegate should be valid")
                .horizontal()
                .rounded(px(0.0))
                .thumb_small()
                .thumb_square()
                .size(ControlSize::Sm);
        let slider_s_domain = slider_s_builder.domain_renderer();
        let slider_s = slider_s_builder.spawn(cx);
        let slider_v_builder =
            ColorSliderBuilder::channel("controls-doc-hsv-plane-v", initial_hsv.v, initial_hsv, Hsv::VALUE)
                .expect("HSV value delegate should be valid")
                .horizontal()
                .rounded(px(0.0))
                .thumb_small()
                .thumb_square()
                .size(ControlSize::Sm);
        let slider_v_domain = slider_v_builder.domain_renderer();
        let slider_v = slider_v_builder.spawn(cx);

        let subscriptions = vec![
            cx.subscribe(&plane, |this, _, event: &ColorFieldEvent, cx| {
                let hsv = match event {
                    ColorFieldEvent::Change(hsv) | ColorFieldEvent::Release(hsv) => *hsv,
                    _ => return,
                };
                let Some(_sync_guard) = this.sync.begin_guard() else {
                    return;
                };
                this.hsv.h = hsv.h;
                this.hsv.s = hsv.s;
                this.hsv.v = hsv.v;
                this.sync_controls(cx);
                cx.notify();
            }),
            cx.subscribe(&slider_h, |this, _, event, cx| {
                if let Some(value) = primary_slider_value(event) {
                    let Some(_sync_guard) = this.sync.begin_guard() else {
                        return;
                    };
                    this.hsv.h = value;
                    this.sync_controls(cx);
                    cx.notify();
                }
            }),
            cx.subscribe(&slider_s, |this, _, event, cx| {
                if let Some(value) = primary_slider_value(event) {
                    let Some(_sync_guard) = this.sync.begin_guard() else {
                        return;
                    };
                    this.hsv.s = value;
                    this.sync_controls(cx);
                    cx.notify();
                }
            }),
            cx.subscribe(&slider_v, |this, _, event, cx| {
                if let Some(value) = primary_slider_value(event) {
                    let Some(_sync_guard) = this.sync.begin_guard() else {
                        return;
                    };
                    this.hsv.v = value;
                    this.sync_controls(cx);
                    cx.notify();
                }
            }),
        ];

        Self {
            look,
            metrics,
            sync: ColorCompositionSync::new(),
            hsv: initial_hsv,
            plane,
            slider_h,
            slider_s,
            slider_v,
            slider_s_domain,
            slider_v_domain,
            _subscriptions: subscriptions,
        }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look;
        self.plane.update(cx, |_, cx| cx.notify());
        self.slider_h.update(cx, |_, cx| cx.notify());
        self.slider_s.update(cx, |_, cx| cx.notify());
        self.slider_v.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn plane(&self) -> Entity<ColorFieldState> {
        self.plane.clone()
    }

    pub fn slider_h(&self) -> Entity<SliderControl> {
        self.slider_h.clone()
    }

    pub fn slider_s(&self) -> Entity<SliderControl> {
        self.slider_s.clone()
    }

    pub fn slider_v(&self) -> Entity<SliderControl> {
        self.slider_v.clone()
    }

    fn sync_controls(&self, cx: &mut Context<Self>) {
        let hsv = self.hsv;
        self.plane.update(cx, |plane, cx| {
            plane.set_hsv_components(hsv.h, hsv.s, hsv.v, cx);
        });
        self.sync.sync_color_slider(
            &self.slider_s,
            &self.slider_s_domain,
            Arc::new(
                ChannelDelegate::new(hsv, Hsv::SATURATION.into()).expect("HSV saturation delegate should be valid"),
            ),
            self.slider_s_domain.context(),
            hsv.s,
            cx,
        );
        self.sync.sync_color_slider(
            &self.slider_v,
            &self.slider_v_domain,
            Arc::new(ChannelDelegate::new(hsv, Hsv::VALUE.into()).expect("HSV value delegate should be valid")),
            self.slider_v_domain.context(),
            hsv.v,
            cx,
        );
        self.sync.sync_slider_value(&self.slider_h, hsv.h, cx);
    }
}

impl Render for HsvPlaneDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let hsla = self.hsv.to_hsla_ext();
        let plane_size = self.metrics.plane_size;
        let look = &self.look;
        let text_size = TextSize::Base;

        div()
            .w(px(plane_size))
            .max_w_full()
            .flex()
            .flex_col()
            .gap(px(COMPOSITION_PRIMARY_READOUT_GAP))
            .child(div().w(px(plane_size)).h(px(plane_size)).overflow_hidden().child(self.plane.clone()))
            .child(slider_labeled_row_compact_sized(look, "H", self.slider_h.clone(), text_size))
            .child(slider_labeled_row_compact_sized(look, "S", self.slider_s.clone(), text_size))
            .child(slider_labeled_row_compact_sized(look, "V", self.slider_v.clone(), text_size))
            .child(render_composition_readout_footer(look, hsla, text_size, Some(plane_size)))
    }
}
