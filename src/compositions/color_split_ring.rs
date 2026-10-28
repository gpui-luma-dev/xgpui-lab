//! Split ring composition exposition — offset saturation/lightness arcs with inner hue ring.

use std::sync::Arc;

use gpui::{Context, Entity, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma::controls::slider::SliderEvent;
use gpui_luma_look_radix::Look;

use super::color_compositions::split_ring::SplitRingDemo;
use super::color_exposition_common::{format_slider_event, render_demo_section, render_demo_card};
use super::event_stream::ControlEventStream;
use super::template::render_composition_exposition;

pub struct ColorSplitRingControlExposition {
    look: Arc<Look>,
    state: Entity<SplitRingDemo>,
    event_stream: Entity<ControlEventStream>,
    _subscriptions: Vec<Subscription>,
}

impl ColorSplitRingControlExposition {
    pub fn new(cx: &mut Context<Self>, look: Arc<Look>) -> Self {
        gpui_luma::theme::observe_theme_revision(cx, |this, cx| this.sync_look(this.look.clone(), cx)).detach();

        let state = cx.new(|cx| SplitRingDemo::new(look.clone(), cx));

        let event_stream = cx.new(|cx| {
            ControlEventStream::new(
                cx,
                look.clone(),
                "controls-color-split-ring-event-log",
                "Edit the split ring; hue, saturation, and lightness SliderEvent variants appear below.",
            )
        });

        let subscriptions = wire_split_ring_events(&state, &event_stream, cx);

        Self { look, state, event_stream, _subscriptions: subscriptions }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look.clone();
        self.state.update(cx, |demo, cx| demo.sync_look(look, cx));
        self.event_stream.update(cx, |stream, cx| stream.sync_look(self.look.clone(), cx));
        cx.notify();
    }
}

impl Render for ColorSplitRingControlExposition {
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
                "Split Ring",
                "Offset saturation and lightness arcs with an inner hue ring.",
                render_demo_card(look, SplitRingDemo::card_width(), self.state.clone()),
            ))
            .child(self.event_stream.clone());

        render_composition_exposition("color-split-ring", preview.into_any_element())
    }
}

fn wire_split_ring_events(
    state: &Entity<SplitRingDemo>,
    event_stream: &Entity<ControlEventStream>,
    cx: &mut Context<ColorSplitRingControlExposition>,
) -> Vec<Subscription> {
    let hue_ring = state.read(cx).hue_ring();
    let saturation_arc = state.read(cx).saturation_arc();
    let lightness_arc = state.read(cx).lightness_arc();
    let event_stream = event_stream.clone();

    vec![
        cx.subscribe(&hue_ring, {
            let event_stream = event_stream.clone();
            move |_, _, event: &SliderEvent, cx| {
                if let Some(line) = format_slider_event("Hue Ring", event) {
                    event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
                }
            }
        }),
        cx.subscribe(&saturation_arc, {
            let event_stream = event_stream.clone();
            move |_, _, event: &SliderEvent, cx| {
                if let Some(line) = format_slider_event("Saturation Arc", event) {
                    event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
                }
            }
        }),
        cx.subscribe(&lightness_arc, move |_, _, event: &SliderEvent, cx| {
            if let Some(line) = format_slider_event("Lightness Arc", event) {
                event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
            }
        }),
    ]
}
