use super::*;
use gpui::{AppContext, Subscription, TestAppContext};
use std::{cell::Cell, rc::Rc};

struct Owner {
    field: Entity<ColorFieldState>,
    log: Vec<ColorFieldEvent>,
    _observer: Option<Subscription>,
}

fn hsv() -> Hsv {
    Hsv { h: 210.0, s: 0.7, v: 0.8, a: 1.0 }
}

#[test]
fn named_callbacks_filter_events_preserve_payloads_and_coexist_with_subscribers() {
    let app = TestAppContext::single();
    let observed = Rc::new(Cell::new(0));
    let owner = app.update(|cx| {
        cx.new(|cx| {
            let field = cx
                .new(|_| ColorFieldState::saturation_value_rect("field", hsv(), 16.0))
                .on_value_changed(cx, |owner: &mut Owner, hsv, _| owner.log.push(ColorFieldEvent::Change(hsv)))
                .on_release(cx, |owner, hsv, _| owner.log.push(ColorFieldEvent::Release(hsv)))
                .on_drag_start(cx, |owner, hsv, _| owner.log.push(ColorFieldEvent::DragStart { hsv }))
                .on_drag_end(cx, |owner, hsv, _| owner.log.push(ColorFieldEvent::DragEnd { hsv }))
                .on_hover_changed(cx, |owner, hovered, _| owner.log.push(ColorFieldEvent::HoverChanged { hovered }))
                .on_enabled_changed(cx, |owner, enabled, _| {
                    owner.log.push(ColorFieldEvent::EnabledChanged { enabled })
                });
            let observed = observed.clone();
            let observer = cx.subscribe(&field, move |_: &mut Owner, _, _: &ColorFieldEvent, _| {
                observed.set(observed.get() + 1);
            });
            Owner { field, log: vec![], _observer: Some(observer) }
        })
    });
    let field = app.read(|cx| owner.read(cx).field.clone());
    let events = [
        ColorFieldEvent::DragStart { hsv: hsv() },
        ColorFieldEvent::Change(Hsv { h: 100.0, ..hsv() }),
        ColorFieldEvent::Release(Hsv { v: 0.4, ..hsv() }),
        ColorFieldEvent::DragEnd { hsv: Hsv { s: 0.3, ..hsv() } },
        ColorFieldEvent::HoverChanged { hovered: true },
        ColorFieldEvent::EnabledChanged { enabled: false },
    ];
    for event in &events {
        app.update(|cx| field.update(cx, |_, cx| cx.emit(event.clone())));
    }
    app.read(|cx| assert_eq!(owner.read(cx).log, events));
    assert_eq!(observed.get(), events.len());
}

#[test]
fn callbacks_append_and_can_update_the_field() {
    let app = TestAppContext::single();
    let owner = app.update(|cx| {
        cx.new(|cx| {
            let field = cx
                .new(|_| ColorFieldState::saturation_value_rect("field", hsv(), 16.0))
                .on_event(cx, |owner: &mut Owner, event, _| owner.log.push(event.clone()))
                .on_value_changed(cx, |owner, value, cx| {
                    owner.log.push(ColorFieldEvent::Release(value));
                    owner.field.update(cx, |field, cx| field.set_enabled(false, cx));
                })
                .on_value_changed(cx, |owner, hsv, _| owner.log.push(ColorFieldEvent::DragEnd { hsv }));
            Owner { field, log: vec![], _observer: None }
        })
    });
    let field = app.read(|cx| owner.read(cx).field.clone());
    app.update(|cx| field.update(cx, |_, cx| cx.emit(ColorFieldEvent::Change(hsv()))));
    app.read(|cx| {
        assert_eq!(
            owner.read(cx).log,
            [
                ColorFieldEvent::Change(hsv()),
                ColorFieldEvent::Release(hsv()),
                ColorFieldEvent::DragEnd { hsv: hsv() },
                ColorFieldEvent::EnabledChanged { enabled: false },
            ]
        );
        assert!(field.read(cx).disabled);
    });
}

#[test]
fn callbacks_hold_owner_weakly_and_release_captures_with_field() {
    let app = TestAppContext::single();
    let calls = Rc::new(Cell::new(0));
    let capture = Rc::new(());
    let owner = app.update(|cx| {
        cx.new(|cx| {
            let calls = calls.clone();
            let capture = capture.clone();
            let field = cx.new(|_| ColorFieldState::saturation_value_rect("field", hsv(), 16.0)).on_value_changed(
                cx,
                move |_: &mut Owner, _, _| {
                    let _keep = &capture;
                    calls.set(calls.get() + 1);
                },
            );
            Owner { field, log: vec![], _observer: None }
        })
    });
    let field = app.read(|cx| owner.read(cx).field.clone());
    let weak_owner = owner.downgrade();
    let weak_field = field.downgrade();
    app.update(|_| drop(owner));
    assert!(weak_owner.upgrade().is_none());
    app.update(|cx| field.update(cx, |_, cx| cx.emit(ColorFieldEvent::Change(hsv()))));
    assert_eq!(calls.get(), 0);
    assert_eq!(Rc::strong_count(&capture), 2);
    app.update(|_| drop(field));
    assert!(weak_field.upgrade().is_none());
    assert_eq!(Rc::strong_count(&capture), 1);
}
