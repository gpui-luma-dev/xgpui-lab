use super::state::{ColorFieldEvent, ColorFieldState};
use gpui::{App, Context, FocusHandle, Focusable, KeyDownEvent, KeyUpEvent, Window};
use gpui_luma::interaction::PointerFocusPolicy;

impl Focusable for ColorFieldState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus_handle
            .get_or_init(|| cx.focus_handle())
            .clone()
            .tab_stop(self.tab_stop && !self.disabled)
    }
}

impl ColorFieldState {
    /// Include this field in Tab traversal (enabled by default). Pointer focus is independent.
    pub fn tab_stop(mut self, enabled: bool) -> Self {
        self.tab_stop = enabled;
        self
    }

    /// Choose whether pointer interaction focuses the field. Defaults to Focus.
    pub fn pointer_focus_policy(mut self, policy: PointerFocusPolicy) -> Self {
        self.pointer_focus = policy;
        self
    }

    /// Normal and Shift+Arrow steps as fractions of the field (defaults: 0.01 and 0.1).
    /// Each must be finite and in (0, 1]; an invalid argument retains its previous value.
    pub fn keyboard_steps(mut self, normal: f32, large: f32) -> Self {
        if normal.is_finite() && normal > 0.0 && normal <= 1.0 {
            self.keyboard_step = normal;
        }
        if large.is_finite() && large > 0.0 && large <= 1.0 {
            self.keyboard_large_step = large;
        }
        self
    }

    pub(super) fn prepare_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) -> FocusHandle {
        let focus = self.focus_handle(cx);
        if self.blur_subscription.is_none() {
            self.blur_subscription = Some(cx.on_blur(&focus, window, |state, _, cx| {
                state.finish_keyboard_adjustment(cx);
            }));
        }
        if self.disabled {
            self.finish_keyboard_adjustment(cx);
            if focus.is_focused(window) {
                window.blur(cx);
            }
        }
        focus
    }

    pub(super) fn finish_keyboard_adjustment(&mut self, cx: &mut Context<Self>) {
        self.held_arrows = 0;
        if std::mem::take(&mut self.keyboard_changed) {
            cx.emit(ColorFieldEvent::Release(self.hsv));
        }
    }

    pub(super) fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if self.disabled
            || !self.focus_handle(cx).is_focused(window)
            || modifiers.control
            || modifiers.platform
            || modifiers.alt
            || modifiers.function
        {
            return;
        }
        let Some((bit, dx, dy)) = arrow(&event.keystroke.key) else {
            return;
        };
        window.prevent_default();
        cx.stop_propagation();
        // Pointer gestures retain their existing lifecycle and exclusively own value changes.
        if self.is_interaction_active() {
            return;
        }
        self.held_arrows |= bit;
        let step = if modifiers.shift {
            self.keyboard_large_step
        } else {
            self.keyboard_step
        };
        let current = self.domain.clamp_uv(self.model.uv_from_hsv(&self.hsv));
        let target = self.domain.clamp_uv((current.0 + dx * step, current.1 + dy * step));
        if (target.0 - current.0).abs() <= f32::EPSILON && (target.1 - current.1).abs() <= f32::EPSILON {
            return;
        }
        let mut next = self.hsv;
        self.model.apply_uv(&mut next, target);
        if !next.h.is_finite() || !next.s.is_finite() || !next.v.is_finite() || !next.a.is_finite() {
            return;
        }
        next.h = next.h.clamp(0.0, 360.0);
        next.s = next.s.clamp(0.0, 1.0);
        next.v = next.v.clamp(0.0, 1.0);
        next.a = next.a.clamp(0.0, 1.0);
        if next != self.hsv {
            self.hsv = next;
            self.keyboard_changed = true;
            cx.emit(ColorFieldEvent::Change(self.hsv));
            cx.notify();
        }
    }

    pub(super) fn handle_key_up(&mut self, event: &KeyUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some((bit, _, _)) = arrow(&event.keystroke.key) else {
            return;
        };
        if self.held_arrows & bit == 0 {
            return;
        }
        self.held_arrows &= !bit;
        if self.held_arrows == 0 {
            self.finish_keyboard_adjustment(cx);
        }
        window.prevent_default();
        cx.stop_propagation();
    }
}

fn arrow(key: &str) -> Option<(u8, f32, f32)> {
    match key {
        "left" => Some((1, -1.0, 0.0)),
        "right" => Some((2, 1.0, 0.0)),
        "up" => Some((4, 0.0, -1.0)),
        "down" => Some((8, 0.0, 1.0)),
        _ => None,
    }
}
