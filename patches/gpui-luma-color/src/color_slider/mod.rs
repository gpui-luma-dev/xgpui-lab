pub mod builder;
pub mod color_spec;
pub mod color_thumb;
pub mod delegates;
pub mod domain_renderer;
mod radius;
mod oklch_spec;
pub mod sync;
pub mod template;
pub mod track_context;
pub mod types;
pub mod visual;

#[cfg(all(test, feature = "test-support"))]
mod pointer_tests;

#[allow(unused_imports)]
pub use color_spec::{ColorSpecification, Hsl, RgbaSpec};
pub use oklch_spec::Oklch;
pub use color_thumb::ThumbShape;
pub use builder::ColorSliderBuilder;
pub use delegates::{AlphaDelegate, ChannelDelegate, GradientDelegate, GradientStop, HueDelegate};
pub use domain_renderer::ColorSliderDomainRenderer;
pub use sync::{primary_slider_value, refresh_color_slider, update_domain_delegate};
pub use template::{ColorSliderTemplate, ColorSliderTemplateConfig, color_slider_template, default_color_slider_template};
pub use track_context::ColorSliderTrackContext;
pub use types::{Axis, ColorInterpolation, ColorSliderDelegate, ThumbPosition, ThumbSize, sizing};
pub use gpui_luma::controls::slider::{SliderControl, SliderEvent, SliderThumbSize};
