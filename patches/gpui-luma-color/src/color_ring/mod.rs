//! Color rings use angular arrow-key steps: 1 degree normally and 10 with Shift.
//! `ColorRingBuilder::keyboard_step_degrees` changes the angular increment;
//! `step` retains its scalar-value snapping role. Arrows preserve the current
//! side of mirrored saturation/lightness rings and wrap through the angular seam.

pub mod builder;
pub(crate) mod common;
pub mod delegates;
pub mod domain_renderer;
pub mod hit_target;
pub mod raster;
pub mod sync;
pub mod template;
pub mod track_context;
pub mod types;
pub mod visual;

#[cfg(all(test, feature = "test-support"))]
mod keyboard_tests;

pub use builder::ColorRingBuilder;
pub use delegates::HueRingDelegate;
pub use domain_renderer::ColorRingDomainRenderer;
pub use hit_target::ColorRingHitTarget;
#[allow(unused_imports)]
pub use delegates::LightnessRingDelegate;
#[allow(unused_imports)]
pub use delegates::SaturationRingDelegate;
#[allow(unused_imports)]
pub use raster::{ColorRingRenderer, RasterRingDelegate};
pub use sync::{primary_slider_value, refresh_color_ring, update_ring_delegate};
pub use template::{ColorRingTemplate, ColorRingTemplateConfig, color_ring_template, default_color_ring_template};
pub use track_context::{ColorRingTrackContext, sizing};
pub use types::ColorRingTrackDelegate;
