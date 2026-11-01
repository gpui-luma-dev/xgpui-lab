use std::sync::{Arc, RwLock};

use gpui::{AppContext, Entity, Hsla, SharedString};

use crate::style::Size;
use gpui_luma::controls::slider::{SliderBuilder, SliderControl, SliderValueMapping, new as new_slider};
use gpui_luma::infra::value::{ControlRange, value_from_input};

use super::delegates::{HueRingDelegate, LightnessRingDelegate, SaturationRingDelegate};
use super::domain_renderer::ColorRingDomainRenderer;
use super::hit_target::ColorRingHitTarget;
use super::raster::{ColorRingRenderer, RasterRingDelegate};
use super::template::{ColorRingTemplateConfig, color_ring_template};
use super::track_context::ColorRingTrackContext;
use super::types::ColorRingTrackDelegate;

pub struct ColorRingBuilder {
    slider: SliderBuilder,
    template_config: Arc<RwLock<ColorRingTemplateConfig>>,
    domain_renderer: Arc<ColorRingDomainRenderer>,
    track_context: ColorRingTrackContext,
    initial_value: f32,
}

impl ColorRingBuilder {
    pub fn new(id: impl Into<SharedString>, value: f32, delegate: Arc<dyn ColorRingTrackDelegate>) -> Self {
        let template_config = Arc::new(RwLock::new(ColorRingTemplateConfig::default()));
        let track_context = ColorRingTrackContext::default();
        let domain_renderer = Arc::new(ColorRingDomainRenderer::new(delegate, track_context.clone()));
        let (min_angle, max_angle) = track_context.angle_range();

        Self {
            slider: new_slider(id)
                .angular(min_angle, max_angle)
                .wrapping(true)
                .keyboard_position_step(1.0 / 360.0)
                .domain()
                .domain_track(domain_renderer.clone())
                .radial_hit_target(Arc::new(ColorRingHitTarget::new(track_context.clone())))
                .template(color_ring_template(Arc::clone(&template_config))),
            template_config,
            domain_renderer,
            track_context,
            initial_value: value,
        }
    }

    pub fn hue(id: impl Into<SharedString>, value: f32, saturation: f32, lightness: f32) -> Self {
        Self::hue_with_renderer(id, value, saturation, lightness, ColorRingRenderer::Raster)
    }

    pub fn saturation(id: impl Into<SharedString>, value: f32, hue: f32, hsv_value: f32) -> Self {
        Self::saturation_with_renderer(id, value, hue, hsv_value, ColorRingRenderer::Raster)
    }

    pub fn lightness(id: impl Into<SharedString>, value: f32, hue: f32, saturation: f32) -> Self {
        Self::lightness_with_renderer(id, value, hue, saturation, ColorRingRenderer::Raster)
    }

    pub fn hue_with_renderer(
        id: impl Into<SharedString>,
        value: f32,
        saturation: f32,
        lightness: f32,
        renderer: ColorRingRenderer,
    ) -> Self {
        let delegate: Arc<dyn ColorRingTrackDelegate> = match renderer {
            ColorRingRenderer::Vector => Arc::new(HueRingDelegate { saturation, lightness }),
            ColorRingRenderer::Raster => Arc::new(RasterRingDelegate::hue(saturation, lightness)),
        };
        Self::new(id, value, delegate).range(0.0..360.0).step(1.0)
    }

    pub fn saturation_with_renderer(
        id: impl Into<SharedString>,
        value: f32,
        hue: f32,
        hsv_value: f32,
        renderer: ColorRingRenderer,
    ) -> Self {
        let delegate: Arc<dyn ColorRingTrackDelegate> = match renderer {
            ColorRingRenderer::Vector => Arc::new(SaturationRingDelegate { hue, hsv_value }),
            ColorRingRenderer::Raster => Arc::new(RasterRingDelegate::saturation(hue, hsv_value)),
        };
        Self::new(id, value, delegate)
            .value_map(Arc::new(MirroredSaturationValueMap))
            .range(0.0..1.0)
            .step(0.001)
    }

    pub fn lightness_with_renderer(
        id: impl Into<SharedString>,
        value: f32,
        hue: f32,
        saturation: f32,
        renderer: ColorRingRenderer,
    ) -> Self {
        let delegate: Arc<dyn ColorRingTrackDelegate> = match renderer {
            ColorRingRenderer::Vector => Arc::new(LightnessRingDelegate { hue, saturation }),
            ColorRingRenderer::Raster => Arc::new(RasterRingDelegate::lightness(hue, saturation)),
        };
        Self::new(id, value, delegate)
            .value_map(Arc::new(MirroredLightnessValueMap))
            .range(0.0..1.0)
            .step(0.001)
    }

    pub fn value_map(mut self, mapping: Arc<dyn SliderValueMapping>) -> Self {
        self.slider = self.slider.value_map(mapping);
        self
    }

    pub fn range(mut self, range: impl Into<ControlRange>) -> Self {
        let range = range.into();
        self.track_context.range = range.start..range.end;
        self.slider = self.slider.range(range);
        self
    }

    pub fn min(mut self, min: f32) -> Self {
        self.track_context.range.start = min;
        self.slider = self.slider.range(self.track_context.range.clone());
        self
    }

    pub fn max(mut self, max: f32) -> Self {
        self.track_context.range.end = max;
        self.slider = self.slider.range(self.track_context.range.clone());
        self
    }

    pub fn step(mut self, step: impl Into<f64>) -> Self {
        self.slider = self.slider.step(step);
        self
    }

    /// Set the arrow-key angular increment (default: 1 degree; Shift uses ten increments).
    /// Pointer value snapping remains controlled by `step`. Invalid angles are ignored.
    pub fn keyboard_step_degrees(mut self, degrees: f32) -> Self {
        self.slider = self.slider.keyboard_position_step(degrees / 360.0);
        self
    }

    pub fn value(mut self, value: impl Into<f64>) -> Self {
        self.initial_value = value_from_input(value);
        self
    }

    pub fn reversed(mut self, reversed: bool) -> Self {
        self.track_context.reversed = reversed;
        self.slider = self.slider.reversed(reversed);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.slider = self.slider.enabled(enabled);
        self
    }

    pub fn size(mut self, size: impl Into<Size>) -> Self {
        self.track_context.size = size.into();
        self
    }

    pub fn ring_thickness(mut self, thickness: f32) -> Self {
        self.track_context.ring_thickness = Some(thickness.max(1.0));
        self
    }

    pub fn ring_thickness_size(mut self, size: impl Into<Size>) -> Self {
        self.track_context.ring_thickness_size = Some(size.into());
        self
    }

    pub fn thumb_size(mut self, size: f32) -> Self {
        self.track_context.thumb_size = Some(size.max(4.0));
        self
    }

    pub fn rotation_degrees(mut self, degrees: f32) -> Self {
        self.track_context.rotation_degrees = degrees;
        self.sync_angular_strategy();
        self
    }

    pub fn allow_inner_target(mut self, allow: bool) -> Self {
        self.track_context.allow_inner_target = allow;
        self
    }

    pub fn ring_inner_border(mut self, enabled: bool) -> Self {
        self.track_context.ring_inner_border = enabled;
        self
    }

    pub fn ring_outer_border(mut self, enabled: bool) -> Self {
        self.track_context.ring_outer_border = enabled;
        self
    }

    pub fn ring_border_color(mut self, color: Hsla) -> Self {
        self.track_context.ring_border_color = color;
        self
    }

    pub fn border_color(self, color: Hsla) -> Self {
        self.ring_border_color(color)
    }

    pub fn domain_renderer(&self) -> Arc<ColorRingDomainRenderer> {
        Arc::clone(&self.domain_renderer)
    }

    pub fn track_context(&self) -> ColorRingTrackContext {
        self.track_context.clone()
    }

    pub fn spawn(mut self, cx: &mut impl AppContext) -> Entity<SliderControl> {
        {
            let mut config = self.template_config.write().expect("color ring template config");
            config.context = self.track_context.clone();
        }
        self.slider.set_radial_hit_target(Arc::new(ColorRingHitTarget::new(self.track_context.clone())));
        self.domain_renderer.set_context(self.track_context);
        self.slider.value(self.initial_value).spawn(cx)
    }

    fn sync_angular_strategy(&mut self) {
        let (min_angle, max_angle) = self.track_context.angle_range();
        self.slider
            .set_strategy(gpui_luma::controls::slider::SliderInputStrategy::Angular { min_angle, max_angle });
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct MirroredSaturationValueMap;

impl SliderValueMapping for MirroredSaturationValueMap {
    fn wraps_value(&self) -> bool {
        false
    }

    fn value_to_position(&self, value: f32, range: ControlRange) -> f32 {
        let pct = range.percentage(value);
        super::common::position_from_mirrored_saturation(pct)
    }

    fn position_to_value(&self, position: f32, range: ControlRange) -> f32 {
        range.value_at(super::common::mirrored_saturation(position))
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct MirroredLightnessValueMap;

impl SliderValueMapping for MirroredLightnessValueMap {
    fn wraps_value(&self) -> bool {
        false
    }

    fn value_to_position(&self, value: f32, range: ControlRange) -> f32 {
        let pct = range.percentage(value);
        super::common::position_from_mirrored_lightness(pct)
    }

    fn position_to_value(&self, position: f32, range: ControlRange) -> f32 {
        range.value_at(super::common::mirrored_lightness(position))
    }
}
