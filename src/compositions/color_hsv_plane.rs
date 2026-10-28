//! HSV plane composition exposition — Photoshop-style HSV plane with H, S, and V sliders.

use std::sync::Arc;

use gpui::{Context, Entity, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma_color::color_field::ColorFieldEvent;
use gpui_luma::controls::slider::SliderEvent;
use gpui_luma_look_radix::Look;

use super::color_compositions::hsv_plane::HsvPlaneDemo;
use super::color_exposition_common::{format_color_field_event, format_slider_event, render_demo_section, render_demo_card};
use super::event_stream::ControlEventStream;
use super::template::render_composition_exposition;

pub struct ColorHsvPlaneControlExposition {
    look: Arc<Look>,
    state: Entity<HsvPlaneDemo>,
    event_stream: Entity<ControlEventStream>,
    _subscriptions: Vec<Subscription>,
}

impl ColorHsvPlaneControlExposition {
    pub fn new(cx: &mut Context<Self>, look: Arc<Look>) -> Self {
        gpui_luma::theme::observe_theme_revision(cx, |this, cx| this.sync_look(this.look.clone(), cx)).detach();

        let state = cx.new(|cx| HsvPlaneDemo::new(look.clone(), cx));

        let event_stream = cx.new(|cx| {
            ControlEventStream::new(
                cx,
                look.clone(),
                "controls-color-hsv-plane-event-log",
                "Edit the HSV plane; ColorFieldEvent and SliderEvent variants appear below.",
            )
        });

        let subscriptions = wire_hsv_plane_events(&state, &event_stream, cx);

        Self { look, state, event_stream, _subscriptions: subscriptions }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look.clone();
        self.state.update(cx, |demo, cx| demo.sync_look(look, cx));
        self.event_stream.update(cx, |stream, cx| stream.sync_look(self.look.clone(), cx));
        cx.notify();
    }
}

impl Render for ColorHsvPlaneControlExposition {
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
                "HSV Plane",
                "Photoshop-style HSV plane with linked H, S, and V channel sliders.",
                render_demo_card(look, HsvPlaneDemo::card_width(), self.state.clone()),
            ))
            .child(self.event_stream.clone());

        render_composition_exposition("color-hsv-plane", preview.into_any_element())
    }
}

fn wire_hsv_plane_events(
    state: &Entity<HsvPlaneDemo>,
    event_stream: &Entity<ControlEventStream>,
    cx: &mut Context<ColorHsvPlaneControlExposition>,
) -> Vec<Subscription> {
    let plane = state.read(cx).plane();
    let slider_h = state.read(cx).slider_h();
    let slider_s = state.read(cx).slider_s();
    let slider_v = state.read(cx).slider_v();
    let event_stream = event_stream.clone();

    vec![
        cx.subscribe(&plane, {
            let event_stream = event_stream.clone();
            move |_, _, event: &ColorFieldEvent, cx| {
                if let Some(line) = format_color_field_event(event) {
                    event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
                }
            }
        }),
        cx.subscribe(&slider_h, {
            let event_stream = event_stream.clone();
            move |_, _, event: &SliderEvent, cx| {
                if let Some(line) = format_slider_event("Hue", event) {
                    event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
                }
            }
        }),
        cx.subscribe(&slider_s, {
            let event_stream = event_stream.clone();
            move |_, _, event: &SliderEvent, cx| {
                if let Some(line) = format_slider_event("Saturation", event) {
                    event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
                }
            }
        }),
        cx.subscribe(&slider_v, move |_, _, event: &SliderEvent, cx| {
            if let Some(line) = format_slider_event("Value", event) {
                event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
            }
        }),
    ]
}
