//! SV triangle composition exposition — hue ring with Photoshop-style saturation/value triangle.

use std::sync::Arc;

use gpui::{Context, Entity, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma_color::color_field::ColorFieldEvent;
use gpui_luma::controls::slider::SliderEvent;
use gpui_luma_look_radix::Look;

use super::color_compositions::sv_triangle::SvTriangleDemo;
use super::color_exposition_common::{format_color_field_event, format_slider_event, render_demo_section, render_demo_card};
use super::event_stream::ControlEventStream;
use super::template::render_composition_exposition;

pub struct ColorSvTriangleControlExposition {
    look: Arc<Look>,
    state: Entity<SvTriangleDemo>,
    event_stream: Entity<ControlEventStream>,
    _subscriptions: Vec<Subscription>,
}

impl ColorSvTriangleControlExposition {
    pub fn new(cx: &mut Context<Self>, look: Arc<Look>) -> Self {
        gpui_luma::theme::observe_theme_revision(cx, |this, cx| this.sync_look(this.look.clone(), cx)).detach();

        let state = cx.new(|cx| SvTriangleDemo::new(look.clone(), cx));

        let event_stream = cx.new(|cx| {
            ControlEventStream::new(
                cx,
                look.clone(),
                "controls-color-sv-triangle-event-log",
                "Edit the SV triangle; SliderEvent and ColorFieldEvent variants appear below.",
            )
        });

        let subscriptions = wire_sv_triangle_events(&state, &event_stream, cx);

        Self { look, state, event_stream, _subscriptions: subscriptions }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look.clone();
        self.state.update(cx, |demo, cx| demo.sync_look(look, cx));
        self.event_stream.update(cx, |stream, cx| stream.sync_look(self.look.clone(), cx));
        cx.notify();
    }
}

impl Render for ColorSvTriangleControlExposition {
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
                "SV Triangle",
                "Hue ring with a centered Photoshop-style saturation/value triangle.",
                render_demo_card(look, SvTriangleDemo::card_width(), self.state.clone()),
            ))
            .child(self.event_stream.clone());

        render_composition_exposition("color-sv-triangle", preview.into_any_element())
    }
}

fn wire_sv_triangle_events(
    state: &Entity<SvTriangleDemo>,
    event_stream: &Entity<ControlEventStream>,
    cx: &mut Context<ColorSvTriangleControlExposition>,
) -> Vec<Subscription> {
    let ring = state.read(cx).ring();
    let triangle = state.read(cx).triangle();
    let event_stream = event_stream.clone();

    vec![
        cx.subscribe(&ring, {
            let event_stream = event_stream.clone();
            move |_, _, event: &SliderEvent, cx| {
                if let Some(line) = format_slider_event("Hue Ring", event) {
                    event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
                }
            }
        }),
        cx.subscribe(&triangle, move |_, _, event: &ColorFieldEvent, cx| {
            if let Some(line) = format_color_field_event(event) {
                event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
            }
        }),
    ]
}
