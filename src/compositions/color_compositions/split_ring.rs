//! Split ring composition — offset saturation/lightness arcs with inner hue ring.

use std::sync::Arc;

use gpui::{Context, Entity, Render, Subscription, Window, div, hsla, prelude::*, px};
use gpui_luma_color::color_arc::{
    ColorArcBuilder, ColorArcDomainRenderer, ColorArcRenderer, ColorArcTrackContext, RasterArcDelegate,
};
use gpui_luma_color::color_ring::{
    ColorRingBuilder, ColorRingDomainRenderer, ColorRingTrackContext, HueRingDelegate, primary_slider_value,
};
use gpui_luma_color::color_slider::color_spec::Hsv;
use gpui_luma_color::composition::ColorCompositionSync;
use gpui_luma::controls::slider::SliderControl;
use crate::theme::{Look, ColorVizLookExt, TextSize};

use super::super::color_exposition_common::{
    composition_demo_card_width, render_composition_readout_footer, COMPOSITION_PRIMARY_READOUT_GAP,
};

pub struct SplitRingDemo {
    look: Arc<Look>,
    metrics: SplitRingMetrics,
    sync: ColorCompositionSync,
    saturation_arc: Entity<SliderControl>,
    saturation_renderer: Arc<ColorArcDomainRenderer>,
    saturation_context: ColorArcTrackContext,
    lightness_arc: Entity<SliderControl>,
    lightness_renderer: Arc<ColorArcDomainRenderer>,
    lightness_context: ColorArcTrackContext,
    hue_ring: Entity<SliderControl>,
    hue_ring_renderer: Arc<ColorRingDomainRenderer>,
    hue_ring_context: ColorRingTrackContext,
    color: SplitRingColorState,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy)]
struct SplitRingColorState {
    hue_degrees: f32,
    saturation: f32,
    lightness: f32,
}

#[derive(Clone, Copy)]
struct SplitRingMetrics {
    outer_size: f32,
    track_width: f32,
    arc_ring_gap: f32,
    ring_swatch_gap: f32,
    arc_horizontal_offset: f32,
}

impl SplitRingMetrics {
    const ARC_GAP_DEGREES: f32 = 8.0;
    const ARC_ROTATION_DEGREES: f32 = 90.0;

    fn new() -> Self {
        let outer_size: f32 = 238.0;
        let scale = outer_size / 300.0;
        Self {
            outer_size,
            track_width: (20.0 * scale).max(12.0),
            arc_ring_gap: (5.0 * scale).max(3.0),
            ring_swatch_gap: (14.0 * scale).max(8.0),
            arc_horizontal_offset: (5.0 * scale).max(3.0),
        }
    }

    fn arc_sweep_degrees(self) -> f32 {
        180.0 - Self::ARC_GAP_DEGREES
    }

    fn ring_outer_radius(self) -> f32 {
        (self.outer_size * 0.5 - self.track_width - self.arc_ring_gap).max(0.0)
    }

    fn frame_size(self) -> f32 {
        self.outer_size + self.arc_horizontal_offset * 2.0
    }

    /// Visible composition width — drives card width and readout preview.
    fn primary_width(self) -> f32 {
        self.frame_size()
    }

    fn card_width(self) -> f32 {
        composition_demo_card_width(self.primary_width())
    }

    fn ring_size(self) -> f32 {
        self.ring_outer_radius() * 2.0
    }

    fn swatch_size(self) -> f32 {
        (self.ring_outer_radius() - self.track_width - self.ring_swatch_gap).max(0.0) * 2.0
    }
}

impl SplitRingDemo {
    pub fn card_width() -> f32 {
        SplitRingMetrics::new().card_width()
    }

    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let init_color = hsla(18.0 / 360.0, 0.85, 0.49, 1.0);
        let hue_degrees = init_color.h * 360.0;
        let saturation = init_color.s;
        let lightness = init_color.l;
        let hsv_value = Hsv::from_hsla_ext(init_color).v;
        let metrics = SplitRingMetrics::new();

        let saturation_builder = ColorArcBuilder::saturation_with_renderer(
            "controls-doc-split-ring-saturation",
            saturation,
            hue_degrees,
            hsv_value,
            ColorArcRenderer::Raster,
        )
        .range(0.0..1.0)
        .step(0.001)
        .size(px(metrics.outer_size))
        .start_degrees(90.0 + SplitRingMetrics::ARC_ROTATION_DEGREES + SplitRingMetrics::ARC_GAP_DEGREES * 0.5)
        .sweep_degrees(metrics.arc_sweep_degrees())
        .arc_thickness(metrics.track_width)
        .thumb_size(metrics.track_width);
        let saturation_renderer = saturation_builder.domain_renderer();
        let saturation_context = saturation_builder.track_context();
        let saturation_arc = saturation_builder.spawn(cx);

        let lightness_builder = ColorArcBuilder::lightness_with_renderer(
            "controls-doc-split-ring-lightness",
            lightness,
            hue_degrees,
            saturation,
            ColorArcRenderer::Raster,
        )
        .range(0.0..1.0)
        .step(0.001)
        .size(px(metrics.outer_size))
        .start_degrees(270.0 + SplitRingMetrics::ARC_ROTATION_DEGREES + SplitRingMetrics::ARC_GAP_DEGREES * 0.5)
        .sweep_degrees(metrics.arc_sweep_degrees())
        .arc_thickness(metrics.track_width)
        .thumb_size(metrics.track_width);
        let lightness_renderer = lightness_builder.domain_renderer();
        let lightness_context = lightness_builder.track_context();
        let lightness_arc = lightness_builder.spawn(cx);

        let hue_ring_builder = ColorRingBuilder::hue("controls-doc-split-ring-hue", hue_degrees, saturation, lightness)
            .size(px(metrics.ring_size()))
            .ring_thickness(metrics.track_width)
            .thumb_size(metrics.track_width);
        let hue_ring_renderer = hue_ring_builder.domain_renderer();
        let hue_ring_context = hue_ring_builder.track_context();
        let hue_ring = hue_ring_builder.spawn(cx);

        let subscriptions = vec![
            cx.subscribe(&hue_ring, |this, _, event, cx| {
                let Some(hue) = primary_slider_value(event) else {
                    return;
                };
                let Some(_sync_guard) = this.sync.begin_guard() else {
                    return;
                };
                this.color.hue_degrees = hue;
                this.sync_controls(cx);
                cx.notify();
            }),
            cx.subscribe(&saturation_arc, |this, _, event, cx| {
                let Some(saturation) = primary_slider_value(event) else {
                    return;
                };
                let Some(_sync_guard) = this.sync.begin_guard() else {
                    return;
                };
                this.color.saturation = saturation.clamp(0.0, 1.0);
                this.sync_controls(cx);
                cx.notify();
            }),
            cx.subscribe(&lightness_arc, |this, _, event, cx| {
                let Some(lightness) = primary_slider_value(event) else {
                    return;
                };
                let Some(_sync_guard) = this.sync.begin_guard() else {
                    return;
                };
                this.color.lightness = lightness.clamp(0.0, 1.0);
                this.sync_controls(cx);
                cx.notify();
            }),
        ];

        let this = Self {
            look,
            metrics,
            sync: ColorCompositionSync::new(),
            saturation_arc,
            saturation_renderer,
            saturation_context,
            lightness_arc,
            lightness_renderer,
            lightness_context,
            hue_ring,
            hue_ring_renderer,
            hue_ring_context,
            color: SplitRingColorState { hue_degrees, saturation, lightness },
            _subscriptions: subscriptions,
        };
        this.sync_controls(cx);
        this
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look;
        self.saturation_arc.update(cx, |_, cx| cx.notify());
        self.lightness_arc.update(cx, |_, cx| cx.notify());
        self.hue_ring.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn hue_ring(&self) -> Entity<SliderControl> {
        self.hue_ring.clone()
    }

    pub fn saturation_arc(&self) -> Entity<SliderControl> {
        self.saturation_arc.clone()
    }

    pub fn lightness_arc(&self) -> Entity<SliderControl> {
        self.lightness_arc.clone()
    }

    fn sync_controls(&self, cx: &mut Context<Self>) {
        let hue_degrees = self.color.hue_degrees;
        let saturation = self.color.saturation;
        let lightness = self.color.lightness;
        let hsv_value = Hsv::from_hsla_ext(hsla((hue_degrees / 360.0).rem_euclid(1.0), saturation, lightness, 1.0)).v;

        self.sync.sync_color_ring(
            &self.hue_ring,
            &self.hue_ring_renderer,
            Arc::new(HueRingDelegate { saturation, lightness }),
            self.hue_ring_context.clone(),
            hue_degrees,
            cx,
        );
        self.sync.sync_color_arc(
            &self.saturation_arc,
            &self.saturation_renderer,
            Arc::new(RasterArcDelegate::saturation(hue_degrees, hsv_value)),
            self.saturation_context.clone(),
            saturation,
            cx,
        );
        self.sync.sync_color_arc(
            &self.lightness_arc,
            &self.lightness_renderer,
            Arc::new(RasterArcDelegate::lightness(hue_degrees, saturation)),
            self.lightness_context.clone(),
            lightness,
            cx,
        );
    }
}

impl Render for SplitRingDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let swatch_color =
            hsla((self.color.hue_degrees / 360.0).rem_euclid(1.0), self.color.saturation, self.color.lightness, 1.0);
        let ring_size = self.metrics.ring_size();
        let swatch_size = self.metrics.swatch_size();
        let primary_width = self.metrics.primary_width();
        let group_width = self.metrics.frame_size();
        let arc_offset = self.metrics.arc_horizontal_offset;
        let look = &self.look;
        let text_size = TextSize::Base;

        let group_left = (primary_width - group_width) * 0.5;
        let group_top = (primary_width - self.metrics.outer_size) * 0.5;
        let ring_offset = (self.metrics.outer_size - ring_size) * 0.5;
        let swatch_offset = (self.metrics.outer_size - swatch_size) * 0.5;

        div()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(COMPOSITION_PRIMARY_READOUT_GAP))
            .child(
                div()
                    .relative()
                    .size(px(primary_width))
                    .child(
                        div()
                            .absolute()
                            .left(px(group_left + arc_offset + ring_offset))
                            .top(px(group_top + ring_offset))
                            .size(px(ring_size))
                            .child(self.hue_ring.clone()),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(group_left))
                            .top(px(group_top))
                            .size(px(self.metrics.outer_size))
                            .child(self.saturation_arc.clone()),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(group_left + arc_offset * 2.0))
                            .top(px(group_top))
                            .size(px(self.metrics.outer_size))
                            .child(self.lightness_arc.clone()),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(group_left + arc_offset + swatch_offset))
                            .top(px(group_top + swatch_offset))
                            .size(px(swatch_size))
                            .rounded_full()
                            .bg(swatch_color)
                            .border_1()
                            .border_color(look.chrome().border),
                    ),
            )
            .child(render_composition_readout_footer(look, swatch_color, text_size, Some(primary_width)))
    }
}
