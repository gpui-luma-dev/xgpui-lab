use std::sync::Arc;

use gpui::{App, Context, FocusHandle, Focusable, KeyDownEvent, Render, ScrollWheelEvent, Subscription, Window, px};

use gpui_luma::controls::scroll_container::ScrollContainer;
use gpui_luma::controls::scrollbar::ScrollbarEvent;
use gpui_luma::theme::{StandardBoxScale, observe_theme_revision};

use super::model::{EventLogViewBuilder, EventLogViewModel, EventLogViewRenderModel};
use super::template::{EventLogViewTemplate, EventLogViewTemplateHandlers, default_event_log_template};
use super::theme::{EventLogLook, EventLogTheme};

pub struct EventLogView {
    model: EventLogViewModel,
    template: Arc<dyn EventLogViewTemplate>,
    text: String,
    scroll: ScrollContainer,
    scroll_to_end_pending: bool,
    focus_handle: FocusHandle,
    focus_subscriptions: Vec<Subscription>,
    _subscriptions: Vec<Subscription>,
}

impl EventLogView {
    pub fn from_builder(builder: EventLogViewBuilder, cx: &mut Context<Self>) -> Self {
        let theme = Arc::clone(&builder.model.theme);
        let template = default_event_log_template(theme);
        let scroll = ScrollContainer::new(
            format!("{}-scroll", builder.model.id),
            Arc::clone(&builder.model.scrollbar_template),
            cx,
        );
        let scrollbar = scroll.scrollbar();
        let subscriptions = vec![cx.subscribe(&scrollbar, |this, _, event: &ScrollbarEvent, cx| {
            let ScrollbarEvent::Change { value } = event else {
                return;
            };
            this.scroll.set_vertical_offset(*value, cx);
            this.scroll_to_end_pending = false;
        })];

        observe_theme_revision(cx, |_, cx| cx.notify()).detach();

        Self {
            model: builder.model,
            template,
            text: String::new(),
            scroll,
            scroll_to_end_pending: false,
            focus_handle: cx.focus_handle().tab_stop(true),
            focus_subscriptions: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn append_line(&mut self, line: &str, cx: &mut Context<Self>) {
        if line.is_empty() {
            return;
        }

        let follow_tail = self.text.is_empty()
            || self.scroll_to_end_pending
            || self.scroll.vertical_offset() >= self.scroll.max_vertical_offset() - px(0.5);
        self.text.push_str(line);
        if !line.ends_with('\n') {
            self.text.push('\n');
        }
        self.scroll_to_end_pending = follow_tail;
        cx.notify();
    }

    pub fn set_theme(&mut self, theme: Arc<dyn EventLogTheme>, cx: &mut Context<Self>) {
        self.model.theme = Arc::clone(&theme);
        self.template = default_event_log_template(theme);
        cx.notify();
    }

    pub fn set_font_family(&mut self, font_family: impl Into<String>, cx: &mut Context<Self>) {
        self.model.font_family = Some(font_family.into());
        cx.notify();
    }

    fn resolved_look(&self, scale: &StandardBoxScale, focused: bool) -> EventLogLook {
        let mut look = self.model.theme.resolve_look(self.model.rows, scale, focused);
        if let Some(font_family) = &self.model.font_family {
            look.font_family = font_family.clone();
        }
        look
    }

    fn scroll_to_end_if_needed(&mut self, cx: &mut Context<Self>) {
        if !self.scroll_to_end_pending {
            return;
        }

        let max_scroll = self.scroll.max_vertical_offset().as_f32();
        let current = self.scroll.vertical_offset().as_f32();
        if max_scroll <= 0.5 {
            if self.text.is_empty() {
                self.scroll_to_end_pending = false;
            }
            return;
        }

        if (current - max_scroll).abs() <= 0.5 {
            self.scroll_to_end_pending = false;
            return;
        }

        self.scroll.set_vertical_offset(max_scroll, cx);
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_handle.contains_focused(window, cx) {
            return;
        }
        let key = &event.keystroke;
        if key.key == "tab" && !key.modifiers.control && !key.modifiers.platform && !key.modifiers.alt {
            if key.modifiers.shift {
                window.focus_prev(cx);
            } else {
                window.focus_next(cx);
            }
        } else {
            // A focused scrollbar retains its own keyboard commands.
            if !self.focus_handle.is_focused(window)
                || key.modifiers.control
                || key.modifiers.platform
                || key.modifiers.alt
            {
                return;
            }
            let step = self.scroll_wheel_step(window, cx);
            let page = step * self.model.rows as f32;
            let current = self.scroll.vertical_offset().as_f32();
            let target = match key.key.as_str() {
                "up" => current - step,
                "down" => current + step,
                "pageup" => current - page,
                "pagedown" => current + page,
                "home" => 0.0,
                "end" => self.scroll.max_vertical_offset().as_f32(),
                _ => return,
            };
            self.scroll_to_end_pending = false;
            self.scroll.set_vertical_offset(target.clamp(0.0, self.scroll.max_vertical_offset().as_f32()), cx);
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    fn scroll_wheel_step(&self, window: &Window, _cx: &mut Context<Self>) -> f32 {
        let scale_factor = window.scale_factor();
        let scale =
            StandardBoxScale::compute(gpui_luma::theme::ControlSize::Md, &self.model.theme.metrics(), scale_factor);
        self.resolved_look(&scale, false).typography.line_height
    }

    fn handle_scroll_wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_handle.contains_focused(window, cx) || self.scroll.max_vertical_offset().as_f32() <= 0.5 {
            return;
        }

        let step = px(self.scroll_wheel_step(window, cx));
        let delta_y = event.delta.pixel_delta(step).y.as_f32();
        if !delta_y.is_finite() || delta_y.abs() <= f32::EPSILON {
            return;
        }

        self.scroll_to_end_pending = false;
        self.scroll.scroll_vertical_by(px(-delta_y), cx);
        cx.stop_propagation();
    }
}

impl Focusable for EventLogView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for EventLogView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        if self.focus_subscriptions.is_empty() {
            self.focus_subscriptions.push(cx.on_focus_in(&self.focus_handle, window, |_, _, cx| cx.notify()));
            self.focus_subscriptions
                .push(cx.on_focus_out(&self.focus_handle, window, |_, _, _, cx| cx.notify()));
        }
        let focused = self.focus_handle.contains_focused(window, cx);
        let scale_factor = window.scale_factor();
        let scale =
            StandardBoxScale::compute(gpui_luma::theme::ControlSize::Md, &self.model.theme.metrics(), scale_factor);
        let look = self.resolved_look(&scale, focused);

        self.scroll.sync_scrollbar(cx);
        // Appends change the measured range during layout; use the new end.
        let entity = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = entity.update(cx, |log, cx| log.scroll_to_end_if_needed(cx));
        });

        let render_model = EventLogViewRenderModel {
            id: &self.model.id,
            text: &self.text,
            placeholder: &self.model.placeholder,
            full_width: self.model.full_width,
            focus_handle: &self.focus_handle,
            look,
        };

        let handlers = EventLogViewTemplateHandlers {
            scroll_wheel: Box::new(cx.listener(Self::handle_scroll_wheel)),
            mouse_down: Box::new(cx.listener(|log, _, window, cx| log.focus_handle.focus(window, cx))),
            key_down: Box::new(cx.listener(Self::handle_key_down)),
        };

        self.template.render(&render_model, &self.scroll, handlers, window, cx)
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests;
