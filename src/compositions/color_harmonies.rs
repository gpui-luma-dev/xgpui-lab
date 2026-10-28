//! Color harmonies composition exposition — wheel, lightness ring, harmony selector, and palette readout.

use std::sync::Arc;

use gpui::{Context, Entity, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma_color::color_field::ColorFieldEvent;
use gpui_luma::controls::slider::SliderEvent;
use gpui_luma_look_radix::Look;

use super::color_compositions::color_harmonies::ColorHarmoniesDemo;
use super::color_exposition_common::{format_color_field_event, format_slider_event, render_demo_section, render_demo_card};
use super::event_stream::ControlEventStream;
use super::template::render_composition_exposition;

pub struct ColorHarmoniesControlExposition {
    look: Arc<Look>,
    state: Entity<ColorHarmoniesDemo>,
    event_stream: Entity<ControlEventStream>,
    _subscriptions: Vec<Subscription>,
}

impl ColorHarmoniesControlExposition {
    pub fn new(cx: &mut Context<Self>, look: Arc<Look>) -> Self {
        gpui_luma::theme::observe_theme_revision(cx, |this, cx| this.sync_look(this.look.clone(), cx)).detach();

        let state = cx.new(|cx| ColorHarmoniesDemo::new(look.clone(), cx));

        let event_stream = cx.new(|cx| {
            ControlEventStream::new(
                cx,
                look.clone(),
                "controls-color-harmonies-event-log",
                "Edit the harmonies demo; ColorFieldEvent and SliderEvent variants appear below.",
            )
        });

        let subscriptions = wire_harmonies_events(&state, &event_stream, cx);

        Self { look, state, event_stream, _subscriptions: subscriptions }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look.clone();
        self.state.update(cx, |demo, cx| demo.sync_look(look, cx));
        self.event_stream.update(cx, |stream, cx| stream.sync_look(self.look.clone(), cx));
        cx.notify();
    }
}

impl Render for ColorHarmoniesControlExposition {
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
                "Color Harmonies",
                "Hue wheel and lightness ring with harmony selector and palette readout.",
                render_demo_card(look, ColorHarmoniesDemo::card_width(), self.state.clone()),
            ))
            .child(self.event_stream.clone());

        render_composition_exposition("color-harmonies", preview.into_any_element())
    }
}

fn wire_harmonies_events(
    state: &Entity<ColorHarmoniesDemo>,
    event_stream: &Entity<ControlEventStream>,
    cx: &mut Context<ColorHarmoniesControlExposition>,
) -> Vec<Subscription> {
    let wheel = state.read(cx).wheel();
    let lightness_ring = state.read(cx).lightness_ring();
    let event_stream = event_stream.clone();

    vec![
        cx.subscribe(&wheel, {
            let event_stream = event_stream.clone();
            move |_, _, event: &ColorFieldEvent, cx| {
                if let Some(line) = format_color_field_event(event) {
                    event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
                }
            }
        }),
        cx.subscribe(&lightness_ring, move |_, _, event: &SliderEvent, cx| {
            if let Some(line) = format_slider_event("Lightness Ring", event) {
                event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
            }
        }),
    ]
}
