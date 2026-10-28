//! Exercise the real log inside a scrolling page; no GUI launch.
use super::*;
use gpui::{Entity, IntoElement, ScrollDelta, ScrollHandle, TestAppContext, VisualTestContext, div, point, prelude::*};
use gpui_luma_look_radix::Look;
use crate::compositions::event_log_view::EventLogViewLookExt;

struct Page {
    log: Entity<EventLogView>,
    scroll: ScrollHandle,
    other_focus: FocusHandle,
}
impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("log-page")
            .w(px(300.0))
            .h(px(220.0))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(div().debug_selector(|| "log-bounds".to_owned()).child(self.log.clone()))
            .child(div().track_focus(&self.other_focus).h(px(900.0)))
    }
}
fn setup(app: &mut TestAppContext, populated: bool) -> (Entity<Page>, &mut VisualTestContext) {
    let (page, cx) = app.add_window_view(|window, cx| {
        window.activate_window();
        let log = Arc::new(Look::built_in()).event_log_view("test-log").rows(3).full_width(true).spawn(cx);
        if populated {
            log.update(cx, |log, cx| {
                for i in 0..80 {
                    log.append_line(&format!("Event {i}"), cx);
                }
            });
        }
        let other_focus = cx.focus_handle().tab_stop(true);
        other_focus.focus(window, cx);
        Page { log, scroll: ScrollHandle::new(), other_focus }
    });
    cx.run_until_parked();
    (page, cx)
}
fn wheel(cx: &mut VisualTestContext, delta: ScrollDelta) {
    cx.simulate_event(ScrollWheelEvent { position: point(px(20.0), px(20.0)), delta, ..Default::default() });
    cx.run_until_parked();
}
fn top(log: &Entity<EventLogView>, cx: &mut VisualTestContext) {
    log.update(cx, |log, cx| {
        log.scroll_to_end_pending = false;
        log.scroll.set_vertical_offset(0.0, cx);
    });
    cx.run_until_parked();
}

#[test]
fn unfocused_event_log_passes_wheel_to_page_at_every_position() {
    for populated in [false, true] {
        let mut app = TestAppContext::single();
        let (page, cx) = setup(&mut app, populated);
        let log = cx.update(|_, app| page.read(app).log.clone());
        for fraction in [0.0, 0.5, 1.0] {
            log.update(cx, |log, cx| {
                log.scroll_to_end_pending = false;
                let max = log.scroll.max_vertical_offset().as_f32();
                assert_eq!(max > 0.0, populated);
                log.scroll.set_vertical_offset(max * fraction, cx);
            });
            page.update(cx, |page, cx| {
                page.scroll.set_offset(point(px(0.0), px(0.0)));
                cx.notify();
            });
            cx.run_until_parked();
            let before = cx.update(|_, app| log.read(app).scroll.vertical_offset());
            wheel(cx, ScrollDelta::Pixels(point(px(0.0), px(-25.0))));
            cx.update(|window, app| {
                assert_eq!(log.read(app).scroll.vertical_offset(), before);
                assert_eq!(page.read(app).scroll.offset().y, px(-25.0));
                assert!(page.read(app).other_focus.is_focused(window));
            });
        }
    }
}

#[test]
fn clicking_event_log_enables_wheel_and_blurring_returns_it_to_page() {
    let mut app = TestAppContext::single();
    let (page, cx) = setup(&mut app, true);
    let log = cx.update(|_, app| page.read(app).log.clone());
    top(&log, cx);
    cx.simulate_click(point(px(20.0), px(20.0)), Default::default());
    cx.run_until_parked();
    wheel(cx, ScrollDelta::Pixels(point(px(0.0), px(-25.0))));
    cx.update(|window, app| {
        assert!(log.read(app).focus_handle.is_focused(window));
        assert_eq!(log.read(app).scroll.vertical_offset(), px(25.0));
        assert_eq!(page.read(app).scroll.offset().y, px(0.0));
    });
    wheel(cx, ScrollDelta::Pixels(point(px(0.0), px(-100_000.0))));
    wheel(cx, ScrollDelta::Lines(point(0.0, -2.0)));
    cx.update(|_, app| {
        assert_eq!(log.read(app).scroll.vertical_offset(), log.read(app).scroll.max_vertical_offset());
        assert_eq!(page.read(app).scroll.offset().y, px(0.0));
    });
    cx.update(|window, app| page.read(app).other_focus.clone().focus(window, app));
    cx.run_until_parked();
    wheel(cx, ScrollDelta::Lines(point(0.0, -2.0)));
    cx.update(|_, app| assert!(page.read(app).scroll.offset().y < px(0.0)));
}

#[test]
fn event_log_supports_tab_navigation_and_focused_history_keys() {
    let mut app = TestAppContext::single();
    let (page, cx) = setup(&mut app, true);
    let log = cx.update(|_, app| page.read(app).log.clone());
    top(&log, cx);
    // Tab traversal reaches the log's own focus handle, without clicking it.
    cx.update(|window, app| window.focus_next(app));
    cx.run_until_parked();
    cx.update(|window, app| assert!(log.read(app).focus_handle.is_focused(window)));
    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    let line = cx.update(|_, app| log.read(app).scroll.vertical_offset());
    assert!(line > px(0.0));
    cx.simulate_keystrokes("pagedown");
    cx.run_until_parked();
    cx.update(|_, app| assert!(log.read(app).scroll.vertical_offset() > line));
    cx.simulate_keystrokes("home");
    cx.run_until_parked();
    cx.update(|_, app| assert_eq!(log.read(app).scroll.vertical_offset(), px(0.0)));
    cx.simulate_keystrokes("end");
    cx.run_until_parked();
    cx.update(|_, app| {
        assert_eq!(log.read(app).scroll.vertical_offset(), log.read(app).scroll.max_vertical_offset());
        assert_eq!(page.read(app).scroll.offset().y, px(0.0));
    });
    cx.simulate_keystrokes("shift-tab");
    cx.run_until_parked();
    cx.update(|window, app| assert!(!log.read(app).focus_handle.is_focused(window)));
}

#[test]
fn event_log_scrollbar_retains_focus_and_append_respects_history_position() {
    let mut app = TestAppContext::single();
    let (page, cx) = setup(&mut app, true);
    let log = cx.update(|_, app| page.read(app).log.clone());
    let max_before = cx.update(|_, app| log.read(app).scroll.max_vertical_offset());
    log.update(cx, |log, cx| log.append_line("New tail", cx));
    cx.run_until_parked();
    cx.update(|_, app| {
        assert!(log.read(app).scroll.max_vertical_offset() > max_before);
        assert_eq!(log.read(app).scroll.vertical_offset(), log.read(app).scroll.max_vertical_offset());
    });
    top(&log, cx);
    let bounds = cx.debug_bounds("log-bounds").unwrap();
    cx.simulate_click(point(bounds.right() - px(6.0), bounds.bottom() - px(10.0)), Default::default());
    cx.run_until_parked();
    let before = cx.update(|window, app| {
        let log = log.read(app);
        assert!(log.scroll.scrollbar().read(app).focus_handle(app).is_focused(window));
        assert!(log.scroll.vertical_offset() > px(0.0));
        log.scroll.vertical_offset()
    });
    log.update(cx, |log, cx| log.append_line("Keep inspecting history", cx));
    cx.run_until_parked();
    cx.update(|_, app| assert_eq!(log.read(app).scroll.vertical_offset(), before));
    // Scrollbar focus qualifies as focus within this log; wheel over its text works.
    wheel(cx, ScrollDelta::Pixels(point(px(0.0), px(-20.0))));
    cx.update(|_, app| {
        assert!(log.read(app).scroll.vertical_offset() > before);
        assert_eq!(page.read(app).scroll.offset().y, px(0.0));
    });
}
