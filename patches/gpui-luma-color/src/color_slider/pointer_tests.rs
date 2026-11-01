use super::{ColorSliderBuilder, default_color_slider_template};
use gpui::{
    Bounds, Context, Entity, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Render, TestAppContext, VisualTestContext, Window, div, point, prelude::*, px,
};
use gpui_luma::controls::slider::{
    SliderControl, SliderEvent, SliderRenderModel, SliderTemplate, SliderTemplateHandlers, ThumbId,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

struct RecordedTrack(Arc<Mutex<Bounds<Pixels>>>);

impl SliderTemplate for RecordedTrack {
    fn render(
        &self,
        model: &SliderRenderModel<'_>,
        mut handlers: SliderTemplateHandlers,
        thumb: ThumbId,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> gpui::Stateful<gpui::Div> {
        let recorded = self.0.clone();
        let original = handlers.track_bounds;
        handlers.track_bounds = Box::new(move |bounds, window, cx| {
            *recorded.lock().unwrap() = *bounds;
            original(bounds, window, cx);
        });
        default_color_slider_template().render(model, handlers, thumb, window, cx)
    }
}

struct Page(Entity<SliderControl>);
impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p(px(40.0)).child(div().w(px(300.0)).h(px(300.0)).child(self.0.clone()))
    }
}

fn setup(
    app: &mut TestAppContext,
    builder: ColorSliderBuilder,
) -> (Entity<SliderControl>, Bounds<Pixels>, &mut VisualTestContext) {
    let bounds = Arc::new(Mutex::new(Bounds::default()));
    let (page, cx) = app.add_window_view(|window, cx| {
        window.activate_window();
        let slider = builder.spawn(cx);
        slider.update(cx, |slider, cx| slider.set_template(Arc::new(RecordedTrack(bounds.clone())), cx));
        Page(slider)
    });
    cx.run_until_parked();
    let slider = cx.update(|_, app| page.read(app).0.clone());
    let bounds = *bounds.lock().unwrap();
    assert!(bounds.size.width > px(0.0));
    (slider, bounds, cx)
}

fn position(bounds: Bounds<Pixels>, fraction: f32) -> Point<Pixels> {
    point(bounds.left() + bounds.size.width * fraction, bounds.center().y)
}

fn press(cx: &mut VisualTestContext, position: Point<Pixels>) {
    cx.simulate_event(MouseDownEvent { position, button: MouseButton::Left, click_count: 1, ..Default::default() });
    cx.run_until_parked();
}

fn drag(cx: &mut VisualTestContext, position: Point<Pixels>) {
    cx.simulate_event(MouseMoveEvent { position, pressed_button: Some(MouseButton::Left), ..Default::default() });
    cx.run_until_parked();
}

#[test]
fn track_press_drags_immediately_and_releases_outside() {
    for reversed in [false, true] {
        let mut app = TestAppContext::single();
        let builder = ColorSliderBuilder::hue("slider", 36.0).reversed(reversed);
        let (slider, bounds, cx) = setup(&mut app, builder);
        let log = Rc::new(RefCell::new(Vec::new()));
        let _sub = cx.update(|_, app| {
            app.subscribe(&slider, {
                let log = log.clone();
                move |_, event: &SliderEvent, _| log.borrow_mut().push(event.clone())
            })
        });
        let assert_value = |cx: &mut VisualTestContext, fraction: f32| {
            let expected = 360.0 * if reversed { 1.0 - fraction } else { fraction };
            cx.update(|_, app| {
                assert!(
                    (slider.read(app).value() - expected).abs() < 1.1,
                    "reversed={reversed}, fraction={fraction}, actual={}",
                    slider.read(app).value()
                )
            });
        };
        press(cx, position(bounds, 0.4));
        assert_value(cx, 0.4);
        drag(cx, position(bounds, 0.7));
        assert_value(cx, 0.7);

        let mut outside = position(bounds, 1.2);
        outside.y += px(80.0);
        drag(cx, outside);
        assert_value(cx, 1.0);
        cx.simulate_event(MouseUpEvent { position: outside, button: MouseButton::Left, ..Default::default() });
        cx.run_until_parked();
        cx.simulate_event(MouseMoveEvent { position: position(bounds, 0.2), ..Default::default() });
        cx.run_until_parked();
        assert_value(cx, 1.0);
        let events = log.borrow();
        assert_eq!(events.iter().filter(|event| matches!(event, SliderEvent::DragStart { .. })).count(), 1);
        assert_eq!(events.iter().filter(|event| matches!(event, SliderEvent::Release { .. })).count(), 1);
        assert_eq!(events.iter().filter(|event| matches!(event, SliderEvent::DragEnd { .. })).count(), 1);
    }
}

#[test]
fn track_press_inserts_one_stop_and_drags_that_same_stop() {
    let mut app = TestAppContext::single();
    let builder = ColorSliderBuilder::gradient(
        "slider",
        0.0,
        vec![gpui::red(), gpui::blue()].into_iter().map(gpui_luma::color::gpui_bridge::from_hsla).collect(),
    )
    .expect("valid demo gradient colors")
    .multi_stop()
    .thumb_values([(0.0, None), (1.0, None)]);
    let (slider, bounds, cx) = setup(&mut app, builder);
    press(cx, position(bounds, 0.4));
    let inserted = cx.update(|_, app| {
        assert_eq!(slider.read(app).thumbs().len(), 3);
        slider.read(app).active_thumb_id().unwrap()
    });
    for fraction in [0.6, 0.7, 0.8] {
        drag(cx, position(bounds, fraction));
        cx.update(|_, app| {
            let slider = slider.read(app);
            assert_eq!(slider.thumbs().len(), 3);
            assert_eq!(slider.active_thumb_id(), Some(inserted));
            assert!((slider.thumb_value(inserted).unwrap() - fraction).abs() < 0.011);
            assert_eq!(
                slider.thumbs().iter().filter(|thumb| thumb.position == 0.0 || thumb.position == 1.0).count(),
                2
            );
        });
    }
}

#[test]
fn disabled_slider_ignores_track_press_and_drag() {
    let mut app = TestAppContext::single();
    let (slider, bounds, cx) = setup(&mut app, ColorSliderBuilder::hue("slider", 36.0).enabled(false));
    press(cx, position(bounds, 0.4));
    drag(cx, position(bounds, 0.7));
    cx.update(|_, app| assert_eq!(slider.read(app).value(), 36.0));
}

#[test]
fn existing_thumb_still_drags_on_first_press() {
    let mut app = TestAppContext::single();
    let (slider, bounds, cx) = setup(&mut app, ColorSliderBuilder::hue("slider", 180.0));
    // The thumb is centered on the 20px-high slider.
    press(cx, point(bounds.center().x, px(50.0)));
    drag(cx, position(bounds, 0.6));
    drag(cx, position(bounds, 0.7));
    cx.update(|_, app| assert!((slider.read(app).value() - 252.0).abs() < 1.1));
}
