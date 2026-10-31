use std::sync::{Arc, RwLock};

use gpui::{AbsoluteLength, AppContext, CornersRefinement, Entity, SharedString};

use gpui_luma::controls::slider::{SliderBuilder, SliderControl, SliderThumbSize, new as new_slider};
use gpui_luma::infra::value::{ControlRange, value_from_input};
use gpui_luma::theme::ControlSize;

use super::color_spec::{ColorSpecification, slider_step_for_channel};
use super::color_thumb::ThumbShape;
use super::delegates::{AlphaDelegate, ChannelDelegate, GradientDelegate, HueDelegate};
use super::domain_renderer::ColorSliderDomainRenderer;
use super::types::{Axis, ColorInterpolation, ColorSliderDelegate, ThumbPosition, ThumbSize};
use super::template::{ColorSliderTemplateConfig, color_slider_template};
use super::track_context::ColorSliderTrackContext;
use gpui_luma::controls::slider::SliderThumbPolicy;

/// Builder for a unified [`Slider`] entity configured as a color spectrum slider.
pub struct ColorSliderBuilder {
    slider: SliderBuilder,
    template_config: Arc<RwLock<ColorSliderTemplateConfig>>,
    domain_renderer: Arc<ColorSliderDomainRenderer>,
    track_context: ColorSliderTrackContext,
    initial_value: f32,
}

impl ColorSliderBuilder {
    pub fn new(id: impl Into<SharedString>, value: f32, delegate: Arc<dyn ColorSliderDelegate>) -> Self {
        let template_config = Arc::new(RwLock::new(ColorSliderTemplateConfig::default()));
        let track_context = ColorSliderTrackContext {
            range: 0.0..1.0,
            reversed: false,
            axis: Axis::Horizontal,
            interpolation: ColorInterpolation::default(),
            corner_radii: CornersRefinement::default(),
            theme_is_dark: false,
        };
        let domain_renderer =
            Arc::new(ColorSliderDomainRenderer::new(delegate, track_context.clone(), Arc::clone(&template_config)));

        Self {
            slider: new_slider(id)
                .domain()
                .domain_track(domain_renderer.clone())
                .template(color_slider_template(Arc::clone(&template_config))),
            template_config,
            domain_renderer,
            track_context,
            initial_value: value,
        }
    }

    pub fn hue(id: impl Into<SharedString>, value: f32) -> Self {
        Self::new(id, value, Arc::new(HueDelegate)).range(0.0..360.0).step(1.0)
    }

    pub fn gradient(
        id: impl Into<SharedString>,
        value: f32,
        colors: Vec<gpui_luma::color::ColorValue>,
    ) -> anyhow::Result<Self> {
        Ok(Self::new(id, value, Arc::new(GradientDelegate::from_colors(colors)?)).range(0.0..1.0).step(0.01))
    }

    pub fn multi_stop(mut self) -> Self {
        self.slider = self.slider.multi_stop();
        self
    }

    /// Exactly two fixed gradient stops (no click-to-add or delete).
    pub fn dual_stop(mut self) -> Self {
        self.slider = self.slider.thumb_policy(SliderThumbPolicy {
            min_count: 2,
            max_count: 2,
            min_distance: 0.02,
            allow_insert: false,
            allow_remove: false,
            allow_overlap: false,
        });
        self
    }

    pub fn thumb_policy(mut self, policy: SliderThumbPolicy) -> Self {
        self.slider = self.slider.thumb_policy(policy);
        self
    }

    pub fn thumb_values(mut self, values: impl IntoIterator<Item = (impl Into<f64>, Option<gpui::Hsla>)>) -> Self {
        self.slider = self.slider.thumb_values(values);
        self
    }

    pub fn alpha<S: ColorSpecification>(id: impl Into<SharedString>, value: f32, spec: S) -> Self {
        Self::new(id, value, Arc::new(AlphaDelegate { spec })).range(0.0..1.0).step(0.01)
    }

    pub fn channel<S: ColorSpecification>(
        id: impl Into<SharedString>,
        value: f32,
        spec: S,
        channel_name: impl Into<SharedString>,
    ) -> Result<Self, String> {
        let channel_name = channel_name.into();

        let delegate = ChannelDelegate::new(spec, channel_name.clone())?;

        let channel = spec
            .channels()
            .iter()
            .find(|c| c.name == channel_name.as_ref())
            .ok_or_else(|| format!("Channel '{}' not found", channel_name))?;

        let step = slider_step_for_channel(channel);

        Ok(Self::new(id, value, Arc::new(delegate)).range(channel.min..channel.max).step(step))
    }

    pub fn saturation<S: ColorSpecification>(
        id: impl Into<SharedString>,
        value: f32,
        spec: S,
        channel_name: impl Into<SharedString>,
    ) -> Result<Self, String> {
        Self::channel(id, value, spec, channel_name)
    }

    pub fn lightness<S: ColorSpecification>(
        id: impl Into<SharedString>,
        value: f32,
        spec: S,
        channel_name: impl Into<SharedString>,
    ) -> Result<Self, String> {
        Self::channel(id, value, spec, channel_name)
    }

    pub fn horizontal(mut self) -> Self {
        self.track_context.axis = Axis::Horizontal;
        self.slider = self.slider.horizontal();
        self
    }

    pub fn vertical(mut self) -> Self {
        self.track_context.axis = Axis::Vertical;
        self.slider = self.slider.vertical();
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

    pub fn size(mut self, size: ControlSize) -> Self {
        self.template_config.write().expect("color slider template config").size = size;
        self.slider = self.slider.size(size);
        self
    }

    pub fn thumb_size(mut self, size: SliderThumbSize) -> Self {
        let mapped = match size {
            SliderThumbSize::Sm => ThumbSize::Small,
            SliderThumbSize::Md => ThumbSize::Medium,
            SliderThumbSize::Lg => ThumbSize::Large,
        };
        self.template_config.write().expect("color slider template config").thumb.size = mapped;
        self.slider = self.slider.thumb_size(size);
        self
    }

    pub fn thumb_xsmall(self) -> Self {
        self.template_config.write().expect("color slider template config").thumb.size = ThumbSize::XSmall;
        self
    }

    pub fn thumb_small(self) -> Self {
        self.thumb_size(SliderThumbSize::Sm)
    }

    pub fn thumb_medium(self) -> Self {
        self.thumb_size(SliderThumbSize::Md)
    }

    pub fn thumb_large(self) -> Self {
        self.thumb_size(SliderThumbSize::Lg)
    }

    pub fn edge_to_edge(self) -> Self {
        self.template_config.write().expect("color slider template config").thumb.position = ThumbPosition::EdgeToEdge;
        self
    }

    pub fn thumb_square(self) -> Self {
        self.template_config.write().expect("color slider template config").thumb.shape = ThumbShape::Square;
        self
    }

    pub fn thumb_bar(self) -> Self {
        self.template_config.write().expect("color slider template config").thumb.shape = ThumbShape::Bar;
        self
    }

    pub fn interpolation(mut self, interpolation: ColorInterpolation) -> Self {
        self.track_context.interpolation = interpolation;
        self
    }

    pub fn theme_is_dark(mut self, theme_is_dark: bool) -> Self {
        self.track_context.theme_is_dark = theme_is_dark;
        self
    }

    pub fn corner_radius(mut self, radius: AbsoluteLength) -> Self {
        {
            let mut config = self.template_config.write().expect("color slider template config");
            config.corner_radii.top_left = Some(radius);
            config.corner_radii.top_right = Some(radius);
            config.corner_radii.bottom_left = Some(radius);
            config.corner_radii.bottom_right = Some(radius);
            self.track_context.corner_radii = config.corner_radii.clone();
        }
        self.slider = self.slider.corner_radius(radius);
        self
    }

    pub fn rounded(self, radius: gpui::Pixels) -> Self {
        self.corner_radius(radius.into())
    }

    pub fn domain_renderer(&self) -> Arc<ColorSliderDomainRenderer> {
        Arc::clone(&self.domain_renderer)
    }

    pub fn track_context(&self) -> ColorSliderTrackContext {
        self.track_context.clone()
    }

    pub fn spawn(self, cx: &mut impl AppContext) -> Entity<SliderControl> {
        self.domain_renderer.set_context(self.track_context);
        self.slider.value(self.initial_value).spawn(cx)
    }
}
