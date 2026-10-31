use std::sync::{Arc, RwLock};

use gpui::{AppContext, Entity, Hsla, SharedString};

use gpui_luma::controls::slider::{SliderBuilder, SliderControl, SliderInputStrategy, new as new_slider};
use gpui_luma::infra::value::{ControlRange, value_from_input};
use crate::style::Size;

use super::delegates::{HueArcDelegate, LightnessArcDelegate, SaturationArcDelegate};
use super::domain_renderer::ColorArcDomainRenderer;
use super::hit_target::ColorArcHitTarget;
use super::raster::{ColorArcRenderer, RasterArcDelegate};
use super::template::{ColorArcTemplateConfig, color_arc_template};
use super::track_context::{ColorArcTrackContext, arc_angle_range};
use super::types::ColorArcDelegate;

/// Builder for a unified [`Slider`] entity configured as a color arc dial.
pub struct ColorArcBuilder {
    slider: SliderBuilder,
    template_config: Arc<RwLock<ColorArcTemplateConfig>>,
    domain_renderer: Arc<ColorArcDomainRenderer>,
    track_context: ColorArcTrackContext,
    initial_value: f32,
}

impl ColorArcBuilder {
    pub fn new(id: impl Into<SharedString>, value: f32, delegate: Arc<dyn ColorArcDelegate>) -> Self {
        let template_config = Arc::new(RwLock::new(ColorArcTemplateConfig::default()));
        let track_context = ColorArcTrackContext::default();
        let domain_renderer = Arc::new(ColorArcDomainRenderer::new(delegate, track_context.clone()));
        let (min_angle, max_angle) = arc_angle_range(track_context.start_degrees, track_context.sweep_degrees);

        Self {
            slider: new_slider(id)
                .angular(min_angle, max_angle)
                .domain()
                .domain_track(domain_renderer.clone())
                .radial_hit_target(Arc::new(ColorArcHitTarget::new(track_context.clone(), None)))
                .template(color_arc_template(Arc::clone(&template_config))),
            template_config,
            domain_renderer,
            track_context,
            initial_value: value,
        }
    }

    pub fn hue(id: impl Into<SharedString>, value: f32, saturation: f32, lightness: f32) -> Self {
        Self::hue_with_renderer(id, value, saturation, lightness, ColorArcRenderer::Raster)
    }

    pub fn saturation(id: impl Into<SharedString>, value: f32, hue: f32, hsv_value: f32) -> Self {
        Self::saturation_with_renderer(id, value, hue, hsv_value, ColorArcRenderer::Raster)
    }

    pub fn lightness(id: impl Into<SharedString>, value: f32, hue: f32, saturation: f32) -> Self {
        Self::lightness_with_renderer(id, value, hue, saturation, ColorArcRenderer::Raster)
    }

    pub fn hue_with_renderer(
        id: impl Into<SharedString>,
        value: f32,
        saturation: f32,
        lightness: f32,
        renderer: ColorArcRenderer,
    ) -> Self {
        let delegate: Arc<dyn ColorArcDelegate> = match renderer {
            ColorArcRenderer::Vector => Arc::new(HueArcDelegate { saturation, lightness }),
            ColorArcRenderer::Raster => Arc::new(RasterArcDelegate::hue(saturation, lightness)),
        };
        Self::new(id, value, delegate).range(0.0..360.0).step(1.0)
    }

    pub fn saturation_with_renderer(
        id: impl Into<SharedString>,
        value: f32,
        hue: f32,
        hsv_value: f32,
        renderer: ColorArcRenderer,
    ) -> Self {
        let delegate: Arc<dyn ColorArcDelegate> = match renderer {
            ColorArcRenderer::Vector => Arc::new(SaturationArcDelegate { hue, hsv_value }),
            ColorArcRenderer::Raster => Arc::new(RasterArcDelegate::saturation(hue, hsv_value)),
        };
        Self::new(id, value, delegate).range(0.0..1.0).step(0.001)
    }

    pub fn lightness_with_renderer(
        id: impl Into<SharedString>,
        value: f32,
        hue: f32,
        saturation: f32,
        renderer: ColorArcRenderer,
    ) -> Self {
        let delegate: Arc<dyn ColorArcDelegate> = match renderer {
            ColorArcRenderer::Vector => Arc::new(LightnessArcDelegate { hue, saturation }),
            ColorArcRenderer::Raster => Arc::new(RasterArcDelegate::lightness(hue, saturation)),
        };
        Self::new(id, value, delegate).range(0.0..1.0).step(0.001)
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

    pub fn arc_thickness(mut self, thickness: f32) -> Self {
        self.track_context.arc_thickness = Some(thickness.max(1.0));
        self
    }

    pub fn arc_thickness_size(mut self, size: impl Into<Size>) -> Self {
        self.track_context.arc_thickness_size = Some(size.into());
        self
    }

    pub fn thumb_size(self, size: f32) -> Self {
        self.template_config.write().expect("color arc template config").thumb_size = Some(size.max(4.0));
        self
    }

    pub fn start_degrees(mut self, degrees: f32) -> Self {
        self.track_context.start_degrees = degrees;
        self.sync_angular_strategy();
        self
    }

    pub fn sweep_degrees(mut self, degrees: f32) -> Self {
        self.track_context.sweep_degrees = degrees.max(0.0);
        self.sync_angular_strategy();
        self
    }

    pub fn border_color(mut self, color: Hsla) -> Self {
        self.track_context.border_color = color;
        self
    }

    pub fn domain_renderer(&self) -> Arc<ColorArcDomainRenderer> {
        Arc::clone(&self.domain_renderer)
    }

    pub fn track_context(&self) -> ColorArcTrackContext {
        self.track_context.clone()
    }

    pub fn spawn(mut self, cx: &mut impl AppContext) -> Entity<SliderControl> {
        let thumb_size = {
            let mut config = self.template_config.write().expect("color arc template config");
            config.context = self.track_context.clone();
            config.thumb_size
        };
        self.slider
            .set_radial_hit_target(Arc::new(ColorArcHitTarget::new(self.track_context.clone(), thumb_size)));
        self.domain_renderer.set_context(self.track_context);
        self.slider.value(self.initial_value).spawn(cx)
    }

    fn sync_angular_strategy(&mut self) {
        let (min_angle, max_angle) =
            arc_angle_range(self.track_context.start_degrees, self.track_context.sweep_degrees);
        self.slider.set_strategy(SliderInputStrategy::Angular { min_angle, max_angle });
    }
}
