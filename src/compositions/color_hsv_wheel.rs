//! HSV wheel composition exposition — hue ring with centered saturation/value square.

use std::sync::Arc;

use gpui::{Context, Entity, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma_color::color_field::ColorFieldEvent;
use gpui_luma::controls::slider::SliderEvent;
use gpui_luma_look_radix::Look;

use super::color_compositions::hsv_wheel::HsvWheelDemo;
use super::color_exposition_common::{format_color_field_event, format_slider_event, render_demo_section, render_demo_card};
use super::event_stream::ControlEventStream;
use super::template::render_composition_exposition;

pub struct ColorHsvWheelControlExposition {
    look: Arc<Look>,
    state: Entity<HsvWheelDemo>,
    event_stream: Entity<ControlEventStream>,
    _subscriptions: Vec<Subscription>,
}

impl ColorHsvWheelControlExposition {
    pub fn new(cx: &mut Context<Self>, look: Arc<Look>) -> Self {
        gpui_luma::theme::observe_theme_revision(cx, |this, cx| this.sync_look(this.look.clone(), cx)).detach();

        let state = cx.new(|cx| HsvWheelDemo::new(look.clone(), cx));

        let event_stream = cx.new(|cx| {
            ControlEventStream::new(
                cx,
                look.clone(),
                "controls-color-hsv-wheel-event-log",
                "Edit the HSV wheel; SliderEvent and ColorFieldEvent variants appear below.",
            )
        });

        let subscriptions = wire_hsv_wheel_events(&state, &event_stream, cx);

        Self { look, state, event_stream, _subscriptions: subscriptions }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look.clone();
        self.state.update(cx, |demo, cx| demo.sync_look(look, cx));
        self.event_stream.update(cx, |stream, cx| stream.sync_look(self.look.clone(), cx));
        cx.notify();
    }
}

impl Render for ColorHsvWheelControlExposition {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let look = &self.look;

        let preview = div()
            .w_full()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(28.0))
            .child(render_demo_section(
                look,
                "HSV Wheel",
                "Hue ring with a centered saturation/value square.",
                render_demo_card(look, HsvWheelDemo::card_width(), self.state.clone()),
            ))
            .child(self.event_stream.clone());

        render_composition_exposition("color-hsv-wheel", preview.into_any_element())
    }
}

fn wire_hsv_wheel_events(
    state: &Entity<HsvWheelDemo>,
    event_stream: &Entity<ControlEventStream>,
    cx: &mut Context<ColorHsvWheelControlExposition>,
) -> Vec<Subscription> {
    let color_ring = state.read(cx).color_ring();
    let plane_sv = state.read(cx).plane_sv();
    let event_stream = event_stream.clone();

    vec![
        cx.subscribe(&color_ring, {
            let event_stream = event_stream.clone();
            move |_, _, event: &SliderEvent, cx| {
                if let Some(line) = format_slider_event("Hue Ring", event) {
                    event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
                }
            }
        }),
        cx.subscribe(&plane_sv, move |_, _, event: &ColorFieldEvent, cx| {
            if let Some(line) = format_color_field_event(event) {
                event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
            }
        }),
    ]
}
