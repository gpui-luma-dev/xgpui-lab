use super::{ColorRingBuilder, ColorRingRenderer};
use gpui::{
    Context, Entity, Focusable, IntoElement, Render, TestAppContext, VisualTestContext, Window, div, point, px,
    prelude::*,
};
use gpui_luma::controls::slider::{
    SliderControl, SliderEvent, SliderTemplate, SliderRenderModel, SliderTemplateHandlers, ThumbId,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

struct RecordedThumb(Arc<Mutex<f32>>);
impl SliderTemplate for RecordedThumb {
    fn render(
        &self,
        model: &SliderRenderModel<'_>,
        handlers: SliderTemplateHandlers,
        thumb: ThumbId,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> gpui::Stateful<gpui::Div> {
        *self.0.lock().unwrap() = model.thumbs[0].position;
        super::default_color_ring_template().render(model, handlers, thumb, window, cx)
    }
}

struct Page(Entity<SliderControl>);
impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(self.0.clone())
    }
}
fn builder(kind: usize) -> ColorRingBuilder {
    match kind {
        0 => ColorRingBuilder::hue_with_renderer("ring", 180.0, 0.8, 0.5, ColorRingRenderer::Vector),
        1 => ColorRingBuilder::saturation_with_renderer("ring", 0.5, 120.0, 0.8, ColorRingRenderer::Vector),
        _ => ColorRingBuilder::lightness_with_renderer("ring", 0.5, 120.0, 0.8, ColorRingRenderer::Vector),
    }
}
fn setup(app: &mut TestAppContext, builder: ColorRingBuilder) -> (Entity<SliderControl>, &mut VisualTestContext) {
    let (page, cx) = app.add_window_view(|window, cx| {
        window.activate_window();
        gpui_luma::key_handling::bind_default_control_keys(cx);
        Page(builder.spawn(cx))
    });
    cx.run_until_parked();
    let ring = cx.update(|window, app| {
        let ring = page.read(app).0.clone();
        ring.read(app).focus_handle(app).focus(window, app);
        ring
    });
    cx.run_until_parked();
    (ring, cx)
}
fn set_position(ring: &Entity<SliderControl>, position: f32, cx: &mut VisualTestContext) {
    ring.update(cx, |ring, cx| {
        ring.set_thumb_position(ring.primary_thumb_id(), position, cx);
    });
    cx.run_until_parked();
}

#[test]
fn ring_arrows_move_locally_on_both_halves_and_across_seams() {
    for kind in 0..3 {
        for reversed in [false, true] {
            let mut app = TestAppContext::single();
            let (ring, cx) = setup(&mut app, builder(kind).reversed(reversed));
            let displayed = Arc::new(Mutex::new(0.0));
            ring.update(cx, |ring, cx| ring.set_template(Arc::new(RecordedThumb(displayed.clone())), cx));
            let log = Rc::new(RefCell::new(Vec::new()));
            let _sub = cx.update(|_, app| {
                app.subscribe(&ring, {
                    let log = log.clone();
                    move |_, event: &SliderEvent, _| log.borrow_mut().push(event.clone())
                })
            });
            for position in [0.0, 0.001, 0.25, 0.499, 0.5, 0.501, 0.75, 0.999] {
                for (key, degrees) in [
                    ("right", 1.0),
                    ("left", -1.0),
                    ("up", 1.0),
                    ("down", -1.0),
                    ("shift-right", 10.0),
                    ("shift-left", -10.0),
                ] {
                    set_position(&ring, position, cx);
                    log.borrow_mut().clear();
                    cx.simulate_keystrokes(key);
                    cx.run_until_parked();
                    cx.update(|_, app| {
                        let ring = ring.read(app);
                        let actual = ring.thumbs()[0].position;
                        let expected = (position + degrees / 360.0 * if reversed { -1.0 } else { 1.0 }).rem_euclid(1.0);
                        assert!(
                            (actual - expected).abs() < 0.00001,
                            "kind={kind} reversed={reversed}, position={position}, key={key}, actual={actual}"
                        );
                        assert!(
                            (*displayed.lock().unwrap() - expected).abs() < 0.00001,
                            "displayed thumb lagged or traversed the wrap seam"
                        );
                        let events: Vec<_> = log
                            .borrow()
                            .iter()
                            .filter(|event| !matches!(event, SliderEvent::HoverChanged { .. }))
                            .cloned()
                            .collect();
                        assert_eq!(events.len(), 2, "{events:?}");
                        assert!(matches!(events[0], SliderEvent::Change { value, .. } if value == ring.value()));
                        assert!(matches!(events[1], SliderEvent::Release { value, .. } if value == ring.value()));
                    });
                }
            }
        }
    }
}

#[test]
fn mirrored_ring_extrema_remain_valid_channel_values() {
    for kind in [1, 2] {
        let mut app = TestAppContext::single();
        let (ring, cx) = setup(&mut app, builder(kind));
        for position in [0.0, 0.5, 1.0] {
            set_position(&ring, position, cx);
            let expected = if kind == 1 {
                super::common::mirrored_saturation(position)
            } else {
                super::common::mirrored_lightness(position)
            };
            cx.update(|_, app| assert!((ring.read(app).value() - expected).abs() < 0.00001));
        }
        ring.update(cx, |ring, cx| ring.set_value(1.0, cx));
        cx.update(|_, app| assert_eq!(ring.read(app).value(), 1.0));
    }
}

#[test]
fn pointer_to_keyboard_handoff_keeps_side_and_custom_angular_step() {
    let mut app = TestAppContext::single();
    let (ring, cx) = setup(&mut app, builder(1).keyboard_step_degrees(2.0));
    // Default 220px ring, 100px track radius: click its rightmost point.
    cx.simulate_click(point(px(210.0), px(110.0)), Default::default());
    cx.run_until_parked();
    cx.update(|_, app| assert!((ring.read(app).thumbs()[0].position - 0.25).abs() < 0.00001));
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    cx.update(|_, app| assert!((ring.read(app).thumbs()[0].position - (0.25 + 2.0 / 360.0)).abs() < 0.00001));
}
