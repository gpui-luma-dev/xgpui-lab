use std::sync::Arc;

use gpui::{App, Entity};

use gpui_luma::controls::slider::{SliderControl, SliderEvent};

use super::domain_renderer::ColorSliderDomainRenderer;
use super::types::ColorSliderDelegate;
use super::track_context::ColorSliderTrackContext;

pub fn primary_slider_value(event: &SliderEvent) -> Option<f32> {
    match event {
        SliderEvent::Change { value, .. } | SliderEvent::Release { value, .. } => Some(*value),
        _ => None,
    }
}

pub fn update_domain_delegate(
    renderer: &ColorSliderDomainRenderer,
    delegate: Arc<dyn ColorSliderDelegate>,
    context: ColorSliderTrackContext,
) {
    renderer.set_delegate(delegate);
    renderer.set_context(context);
}

pub fn refresh_color_slider(slider: &Entity<SliderControl>, cx: &mut App) {
    slider.update(cx, |slider, cx| {
        slider.sync_domain_thumb_previews();
        cx.notify();
    });
}
