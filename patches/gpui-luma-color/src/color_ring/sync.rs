use std::sync::Arc;

use gpui::{App, Entity};

use gpui_luma::controls::slider::{SliderControl, SliderEvent};

use super::domain_renderer::ColorRingDomainRenderer;
use super::track_context::ColorRingTrackContext;
use super::types::ColorRingTrackDelegate;

pub fn primary_slider_value(event: &SliderEvent) -> Option<f32> {
    match event {
        SliderEvent::Change { value, .. } | SliderEvent::Release { value, .. } => Some(*value),
        _ => None,
    }
}

pub fn update_ring_delegate(
    renderer: &ColorRingDomainRenderer,
    delegate: Arc<dyn ColorRingTrackDelegate>,
    context: ColorRingTrackContext,
) {
    renderer.set_delegate(delegate);
    renderer.set_context(context);
}

pub fn refresh_color_ring(slider: &Entity<SliderControl>, cx: &mut App) {
    slider.update(cx, |slider, cx| {
        slider.sync_domain_thumb_previews();
        cx.notify();
    });
}
