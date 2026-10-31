use std::sync::Arc;

use gpui::{App, Entity};

use gpui_luma::controls::slider::{SliderControl, SliderEvent};

use super::domain_renderer::ColorArcDomainRenderer;
use super::track_context::ColorArcTrackContext;
use super::types::ColorArcDelegate;

pub fn primary_slider_value(event: &SliderEvent) -> Option<f32> {
    match event {
        SliderEvent::Change { value, .. } | SliderEvent::Release { value, .. } => Some(*value),
        _ => None,
    }
}

pub fn update_arc_delegate(
    renderer: &ColorArcDomainRenderer,
    delegate: Arc<dyn ColorArcDelegate>,
    context: ColorArcTrackContext,
) {
    renderer.set_delegate(delegate);
    renderer.set_context(context);
}

pub fn refresh_color_arc(slider: &Entity<SliderControl>, cx: &mut App) {
    slider.update(cx, |slider, cx| {
        slider.sync_domain_thumb_previews();
        cx.notify();
    });
}
