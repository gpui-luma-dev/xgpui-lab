use super::state::{ColorFieldEvent, ColorFieldState};
use crate::color_field::{CircleDomain, PolygonDomain, RectDomain, TriangleDomain, SvAtHueModel};
use crate::color_slider::color_spec::Hsv;
use gpui::{
    prelude::*, Context, Entity, FocusHandle, Focusable, ScrollHandle, Render, Window, div, px, point, TestAppContext,
    VisualTestContext, Keystroke, KeyDownEvent, KeyUpEvent, Subscription, MouseMoveEvent, ScrollWheelEvent,
    ScrollDelta,
};
use gpui_luma::focus::LumaFocusScopeExt;
use gpui_luma::interaction::PointerFocusPolicy;
use std::{cell::RefCell, rc::Rc, sync::Arc};

struct Page {
    field: Entity<ColorFieldState>,
    scope: FocusHandle,
    before: FocusHandle,
    after: FocusHandle,
    scroll: ScrollHandle,
    ancestor_keys: usize,
    block_bubble_pointer: bool,
    hue: Option<Entity<gpui_luma::controls::slider::SliderControl>>,
    bubbled_moves: usize,
}
impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("page")
            .w(px(300.0))
            .h(px(300.0))
            .luma_focus_scope(&self.scope)
            .on_key_down(cx.listener(|page, _, _, _| page.ancestor_keys += 1))
            .on_mouse_move(cx.listener(|page, _, _, cx| {
                page.bubbled_moves += 1;
                if page.block_bubble_pointer {
                    cx.stop_propagation();
                }
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|page, _, _, cx| {
                    if page.block_bubble_pointer {
                        cx.stop_propagation();
                    }
                }),
            )
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(div().id("before").track_focus(&self.before).h(px(10.0)))
            .child(div().w(px(200.0)).h(px(200.0)).child(self.field.clone()))
            .child(div().id("after").track_focus(&self.after).h(px(10.0)))
            .when_some(self.hue.clone(), |root, hue| root.child(div().h(px(24.0)).w(px(200.0)).child(hue)))
            .child(div().h(px(1000.0)))
    }
}
fn initial() -> Hsv {
    Hsv { h: 120.0, s: 0.5, v: 0.5, a: 0.7 }
}
fn state() -> ColorFieldState {
    ColorFieldState::saturation_value_rect("field", initial(), 16.0).vector().samples_per_axis(8)
}
fn setup(
    app: &mut TestAppContext,
    state: ColorFieldState,
) -> (Entity<Page>, &mut VisualTestContext, Entity<ColorFieldState>) {
    let (page, cx) = app.add_window_view(|window, cx| {
        window.activate_window();
        gpui_luma::focus::bind_default_focus_keys(cx);
        let before = cx.focus_handle().tab_stop(true);
        before.focus(window, cx);
        Page {
            field: cx.new(|_| state),
            scope: cx.focus_handle(),
            before,
            after: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            ancestor_keys: 0,
            block_bubble_pointer: false,
            hue: None,
            bubbled_moves: 0,
        }
    });
    cx.run_until_parked();
    let field = cx.update(|_, app| page.read(app).field.clone());
    (page, cx, field)
}
fn focus(field: &Entity<ColorFieldState>, cx: &mut VisualTestContext) {
    cx.update(|window, app| field.read(app).focus_handle(app).focus(window, app));
    cx.run_until_parked();
}
fn key(cx: &mut VisualTestContext, stroke: &str, down: bool) {
    let keystroke = Keystroke::parse(stroke).unwrap();
    if down {
        cx.simulate_event(KeyDownEvent { keystroke, is_held: false, prefer_character_input: false });
    } else {
        cx.simulate_event(KeyUpEvent { keystroke });
    }
    cx.run_until_parked();
}
fn events(
    field: &Entity<ColorFieldState>,
    cx: &mut VisualTestContext,
) -> (Rc<RefCell<Vec<ColorFieldEvent>>>, Subscription) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let sub = cx.update(|_, app| {
        app.subscribe(field, {
            let log = log.clone();
            move |_, event: &ColorFieldEvent, _| log.borrow_mut().push(event.clone())
        })
    });
    (log, sub)
}
fn changes(log: &[ColorFieldEvent]) -> usize {
    log.iter().filter(|e| matches!(e, ColorFieldEvent::Change(_))).count()
}
fn releases(log: &[ColorFieldEvent]) -> usize {
    log.iter().filter(|e| matches!(e, ColorFieldEvent::Release(_))).count()
}

#[test]
fn arrows_repeat_shift_and_release_without_drag_events_or_ancestor_keys() {
    let mut app = TestAppContext::single();
    let (page, cx, field) = setup(&mut app, state());
    focus(&field, cx);
    let (log, _sub) = events(&field, cx);
    // Consumers commonly mirror Change back into the field; that must not end a gesture.
    let _mirror = cx.update(|_, app| {
        app.subscribe(&field, |field, event: &ColorFieldEvent, app| {
            if let ColorFieldEvent::Change(hsv) = event {
                field.update(app, |state, cx| state.set_hsv(*hsv, cx));
            }
        })
    });
    key(cx, "right", true);
    cx.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse("right").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    key(cx, "shift-up", true);
    key(cx, "right", false);
    assert_eq!(releases(&log.borrow()), 0);
    key(cx, "up", false);
    assert_eq!(changes(&log.borrow()), 3);
    assert_eq!(releases(&log.borrow()), 1);
    assert!(
        !log.borrow()
            .iter()
            .any(|e| matches!(e, ColorFieldEvent::DragStart { .. } | ColorFieldEvent::DragEnd { .. }))
    );
    cx.update(|_, app| {
        let hsv = field.read(app).hsv;
        assert!((hsv.s - 0.52).abs() < 0.00001);
        assert!((hsv.v - 0.6).abs() < 0.00001);
        assert_eq!((hsv.h, hsv.a), (120.0, 0.7));
        assert_eq!(page.read(app).ancestor_keys, 0);
    });
    key(cx, "cmd-right", true);
    cx.update(|_, app| assert_eq!(page.read(app).ancestor_keys, 1));
}

#[test]
fn tab_blur_disabled_and_tab_override_preserve_value_ownership() {
    let mut app = TestAppContext::single();
    let (page, cx, field) = setup(&mut app, state());
    key(cx, "right", true);
    cx.update(|_, app| assert_eq!(field.read(app).hsv, initial()));
    cx.simulate_keystrokes("tab");
    cx.update(|window, app| assert!(field.read(app).focus_handle(app).is_focused(window)));
    let (log, _sub) = events(&field, cx);
    key(cx, "right", true);
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    assert_eq!(releases(&log.borrow()), 1);
    cx.update(|window, app| assert!(page.read(app).after.is_focused(window)));
    cx.simulate_keystrokes("shift-tab");
    key(cx, "up", true);
    field.update(cx, |state, cx| state.set_enabled(false, cx));
    cx.run_until_parked();
    assert_eq!(releases(&log.borrow()), 2);
    cx.update(|window, app| {
        assert!(!field.read(app).focus_handle(app).is_focused(window));
        page.read(app).before.clone().focus(window, app);
    });
    cx.simulate_keystrokes("tab");
    cx.update(|window, app| assert!(page.read(app).after.is_focused(window)));
    let value = cx.update(|_, app| field.read(app).hsv);
    cx.simulate_click(point(px(20.0), px(40.0)), Default::default());
    key(cx, "right", true);
    cx.update(|_, app| assert_eq!(field.read(app).hsv, value));

    let mut app = TestAppContext::single();
    let (page, cx, _) = setup(&mut app, state().tab_stop(false));
    cx.simulate_keystrokes("tab");
    cx.update(|window, app| assert!(page.read(app).after.is_focused(window)));
}

#[test]
fn pointer_focus_override_and_wheel_preserve_mouse_lifecycle() {
    for policy in [PointerFocusPolicy::Focus, PointerFocusPolicy::Preserve] {
        let mut app = TestAppContext::single();
        let (page, cx, field) = setup(&mut app, state().pointer_focus_policy(policy));
        let (log, _sub) = events(&field, cx);
        cx.simulate_event(MouseMoveEvent { position: point(px(150.0), px(60.0)), ..Default::default() });
        cx.run_until_parked();
        cx.update(|window, app| {
            assert!(page.read(app).before.is_focused(window));
            assert_eq!(field.read(app).hsv, initial());
        });
        cx.simulate_click(point(px(150.0), px(60.0)), Default::default());
        cx.run_until_parked();
        cx.update(|window, app| {
            assert_eq!(field.read(app).focus_handle(app).is_focused(window), policy == PointerFocusPolicy::Focus);
            assert!((field.read(app).hsv.s - 0.75).abs() < 0.00001);
        });
        assert_eq!(releases(&log.borrow()), 1);
        assert_eq!(log.borrow().iter().filter(|e| matches!(e, ColorFieldEvent::DragStart { .. })).count(), 1);
        assert_eq!(log.borrow().iter().filter(|e| matches!(e, ColorFieldEvent::DragEnd { .. })).count(), 1);
        let value = cx.update(|_, app| field.read(app).hsv);
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(100.0), px(100.0)),
            delta: ScrollDelta::Pixels(point(px(0.0), px(-30.0))),
            ..Default::default()
        });
        cx.run_until_parked();
        cx.update(|_, app| {
            assert_eq!(field.read(app).hsv, value);
            assert_eq!(page.read(app).scroll.offset().y, px(-30.0));
        });
    }
}

#[test]
fn custom_steps_clamp_to_each_domain_and_boundary_does_not_emit_changes() {
    let domains: Vec<Arc<dyn crate::color_field::FieldDomain2D>> = vec![
        Arc::new(RectDomain),
        Arc::new(CircleDomain),
        Arc::new(TriangleDomain::up()),
        Arc::new(PolygonDomain::new(vec![(0.1, 0.1), (0.9, 0.1), (0.9, 0.9), (0.1, 0.9)])),
    ];
    for domain in domains {
        let mut app = TestAppContext::single();
        let (_, cx, field) = setup(
            &mut app,
            ColorFieldState::new("shape", initial(), domain.clone(), Arc::new(SvAtHueModel))
                .vector()
                .samples_per_axis(8)
                .keyboard_steps(0.25, 1.0),
        );
        focus(&field, cx);
        key(cx, "right", true);
        key(cx, "right", false);
        cx.update(|_, app| assert!((field.read(app).hsv.s - 0.75).abs() < 0.00001));
        for stroke in ["shift-up", "shift-left", "shift-down", "shift-right"] {
            key(cx, stroke, true);
            key(cx, stroke, false);
            cx.update(|_, app| {
                let state = field.read(app);
                let uv = state.model.uv_from_hsv(&state.hsv);
                let clamped = domain.clamp_uv(uv);
                // Polygon hit testing excludes some boundary points; projection still permits them.
                assert!((uv.0 - clamped.0).abs() < 0.00001 && (uv.1 - clamped.1).abs() < 0.00001);
                assert_eq!((state.hsv.h, state.hsv.a), (120.0, 0.7));
            });
        }
    }
    let mut app = TestAppContext::single();
    let (_, cx, field) = setup(&mut app, state().keyboard_steps(f32::NAN, -1.0));
    field.update(cx, |state, cx| state.set_hsv(Hsv { s: 1.0, ..initial() }, cx));
    focus(&field, cx);
    let (log, _sub) = events(&field, cx);
    key(cx, "right", true);
    key(cx, "right", false);
    assert_eq!(changes(&log.borrow()), 0);
    assert_eq!(releases(&log.borrow()), 0);
    key(cx, "left", true);
    key(cx, "left", false);
    cx.update(|_, app| assert!((field.read(app).hsv.s - 0.99).abs() < 0.00001));
}

#[test]
fn wheel_model_moves_spatially_and_preserves_value_and_alpha() {
    let mut app = TestAppContext::single();
    let (_, cx, field) = setup(
        &mut app,
        ColorFieldState::hue_saturation_wheel("wheel", initial(), 16.0).vector().samples_per_axis(8),
    );
    focus(&field, cx);
    let before = cx.update(|_, app| {
        let state = field.read(app);
        state.model.uv_from_hsv(&state.hsv)
    });
    key(cx, "right", true);
    key(cx, "right", false);
    cx.update(|_, app| {
        let state = field.read(app);
        let after = state.model.uv_from_hsv(&state.hsv);
        assert!((after.0 - before.0 - 0.01).abs() < 0.00001);
        assert!((after.1 - before.1).abs() < 0.00001);
        assert_eq!((state.hsv.v, state.hsv.a), (initial().v, initial().a));
    });
}

#[test]
fn pointer_drag_tracks_outside_with_bubble_blocker_until_left_release() {
    use gpui::{MouseButton, MouseDownEvent, MouseUpEvent};
    let mut app = TestAppContext::single();
    let (page, cx, field) = setup(&mut app, state());
    page.update(cx, |page, cx| {
        page.block_bubble_pointer = true;
        cx.notify();
    });
    cx.run_until_parked();
    let (log, _subscription) = events(&field, cx);
    cx.simulate_event(MouseDownEvent {
        position: point(px(100.0), px(100.0)),
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
    let outside = point(px(250.0), px(50.0));
    cx.simulate_event(MouseMoveEvent {
        position: outside,
        pressed_button: Some(MouseButton::Left),
        ..Default::default()
    });
    cx.run_until_parked();
    cx.update(|_, app| {
        let state = field.read(app);
        assert!(state.is_interaction_active());
        assert_eq!(state.hsv.s, 1.0);
    });
    cx.simulate_event(MouseUpEvent { position: outside, button: MouseButton::Right, ..Default::default() });
    cx.run_until_parked();
    cx.update(|_, app| assert!(field.read(app).is_interaction_active()));
    cx.simulate_event(MouseUpEvent { position: outside, button: MouseButton::Left, ..Default::default() });
    cx.run_until_parked();
    let released = cx.update(|_, app| {
        assert!(!field.read(app).is_interaction_active());
        field.read(app).hsv
    });
    assert_eq!(releases(&log.borrow()), 1);
    cx.simulate_event(MouseMoveEvent { position: point(px(20.0), px(150.0)), ..Default::default() });
    cx.run_until_parked();
    cx.update(|_, app| assert_eq!(field.read(app).hsv, released));
}

#[test]
fn field_drag_over_hue_slider_owns_moves_and_release() {
    use gpui::{MouseButton, MouseDownEvent, MouseUpEvent};
    use crate::color_slider::ColorSliderBuilder;
    let mut app = TestAppContext::single();
    let (page, cx, field) = setup(&mut app, state());
    let hue = page.update(cx, |page, cx| {
        let hue = ColorSliderBuilder::hue("neighbor-hue", 120.0).spawn(cx);
        page.hue = Some(hue.clone());
        cx.notify();
        hue
    });
    cx.run_until_parked();
    cx.simulate_event(MouseDownEvent {
        position: point(px(100.0), px(100.0)),
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
    let before = cx.update(|_, app| page.read(app).bubbled_moves);
    for x in [30.0, 100.0, 180.0] {
        cx.simulate_event(MouseMoveEvent {
            position: point(px(x), px(230.0)),
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        });
        cx.run_until_parked();
        cx.update(|_, app| {
            assert!(field.read(app).is_interaction_active());
            assert_eq!(field.read(app).hsv.v, 0.0);
            assert_eq!(hue.read(app).value(), 120.0);
            assert_eq!(page.read(app).bubbled_moves, before);
        });
    }
    cx.simulate_event(MouseUpEvent {
        position: point(px(180.0), px(230.0)),
        button: MouseButton::Left,
        ..Default::default()
    });
    cx.run_until_parked();
    cx.update(|_, app| assert!(!field.read(app).is_interaction_active()));
    // A fresh press can now operate the neighboring hue slider normally.
    cx.simulate_event(MouseDownEvent {
        position: point(px(170.0), px(230.0)),
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.simulate_event(MouseUpEvent {
        position: point(px(170.0), px(230.0)),
        button: MouseButton::Left,
        ..Default::default()
    });
    cx.run_until_parked();
    cx.update(|_, app| assert_ne!(hue.read(app).value(), 120.0));
}
