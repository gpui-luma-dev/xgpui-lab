//! Owner-bound callbacks for color fields.
use gpui::{Context, Entity};
use gpui_luma::infra::callbacks::EventCallbacks;

use super::{ColorFieldEvent, ColorFieldState};
use crate::color_slider::color_spec::Hsv;

/// Semantic callbacks on a color-field entity.
///
/// Register after `cx.new`. Handlers hold the owner weakly and live until the
/// field is released. Registrations append and coexist with raw subscribers.
/// Capture entities weakly when they can own this field.
pub trait ColorFieldEventExt: Sized {
    /// Handle every field event, including future event variants.
    fn on_event<M: 'static>(
        self,
        cx: &mut Context<M>,
        callback: impl Fn(&mut M, &ColorFieldEvent, &mut Context<M>) + 'static,
    ) -> Self;

    /// Update the live color preview.
    fn on_value_changed<M: 'static>(
        self,
        cx: &mut Context<M>,
        callback: impl Fn(&mut M, Hsv, &mut Context<M>) + 'static,
    ) -> Self {
        self.on_event(cx, move |owner, event, cx| {
            if let ColorFieldEvent::Change(hsv) = event {
                callback(owner, *hsv, cx);
            }
        })
    }

    /// Commit a completed pointer gesture or changed keyboard adjustment.
    fn on_release<M: 'static>(
        self,
        cx: &mut Context<M>,
        callback: impl Fn(&mut M, Hsv, &mut Context<M>) + 'static,
    ) -> Self {
        self.on_event(cx, move |owner, event, cx| {
            if let ColorFieldEvent::Release(hsv) = event {
                callback(owner, *hsv, cx);
            }
        })
    }

    /// Handle the start of a pointer gesture.
    fn on_drag_start<M: 'static>(
        self,
        cx: &mut Context<M>,
        callback: impl Fn(&mut M, Hsv, &mut Context<M>) + 'static,
    ) -> Self {
        self.on_event(cx, move |owner, event, cx| {
            if let ColorFieldEvent::DragStart { hsv } = event {
                callback(owner, *hsv, cx);
            }
        })
    }

    /// Handle the end of a pointer gesture.
    fn on_drag_end<M: 'static>(
        self,
        cx: &mut Context<M>,
        callback: impl Fn(&mut M, Hsv, &mut Context<M>) + 'static,
    ) -> Self {
        self.on_event(cx, move |owner, event, cx| {
            if let ColorFieldEvent::DragEnd { hsv } = event {
                callback(owner, *hsv, cx);
            }
        })
    }

    /// Handle pointer hover changes.
    fn on_hover_changed<M: 'static>(
        self,
        cx: &mut Context<M>,
        callback: impl Fn(&mut M, bool, &mut Context<M>) + 'static,
    ) -> Self {
        self.on_event(cx, move |owner, event, cx| {
            if let ColorFieldEvent::HoverChanged { hovered } = event {
                callback(owner, *hovered, cx);
            }
        })
    }

    /// Handle enablement changes.
    fn on_enabled_changed<M: 'static>(
        self,
        cx: &mut Context<M>,
        callback: impl Fn(&mut M, bool, &mut Context<M>) + 'static,
    ) -> Self {
        self.on_event(cx, move |owner, event, cx| {
            if let ColorFieldEvent::EnabledChanged { enabled } = event {
                callback(owner, *enabled, cx);
            }
        })
    }
}

impl ColorFieldEventExt for Entity<ColorFieldState> {
    fn on_event<M: 'static>(
        self,
        cx: &mut Context<M>,
        callback: impl Fn(&mut M, &ColorFieldEvent, &mut Context<M>) + 'static,
    ) -> Self {
        EventCallbacks::default().on_event(cx, callback).bind(&self, cx);
        self
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests;
