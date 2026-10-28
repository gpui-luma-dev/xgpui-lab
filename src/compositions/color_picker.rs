//! Color picker control exposition — Photoshop-style SV field, hue, and alpha composition.

use gpui_luma::color::gpui_bridge::from_hsla;
use std::sync::Arc;

use gpui::{Context, Entity, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma_color::ColorSwatch;
use gpui_luma_color::color_field::{ColorFieldEvent, ColorFieldState};
use gpui_luma_color::color_slider::color_spec::Hsv;
use gpui_luma_color::color_slider::{AlphaDelegate, ColorSliderBuilder, ColorSliderDomainRenderer, primary_slider_value};
use gpui_luma_color::composition::ColorCompositionSync;
use gpui_luma::controls::slider::{SliderControl, SliderEvent};
use gpui_luma::theme::ControlSize;
use crate::theme::{Look, TextSize};

use super::color_exposition_common::{
    composition_card_width, composition_inset_radius, detail_row_sized, format_color_field_event, format_compact_hsla,
    format_hex_color, format_slider_event, render_demo_section, render_demo_card_with_padding,
};
use super::event_stream::ControlEventStream;
use super::template::render_composition_exposition;

const PICKER_HORIZONTAL_PADDING: f32 = 25.0;
const PICKER_VERTICAL_PADDING: f32 = 18.0;

pub struct ColorPickerControlExposition {
    look: Arc<Look>,
    state: Entity<ColorPickerDemo>,
    event_stream: Entity<ControlEventStream>,
    _subscriptions: Vec<Subscription>,
}

impl ColorPickerControlExposition {
    pub fn new(cx: &mut Context<Self>, look: Arc<Look>) -> Self {
        gpui_luma::theme::observe_theme_revision(cx, |this, cx| this.sync_look(this.look.clone(), cx)).detach();

        let state = cx.new(|cx| ColorPickerDemo::new(look.clone(), cx));

        let event_stream = cx.new(|cx| {
            ControlEventStream::new(
                cx,
                look.clone(),
                "controls-color-picker-event-log",
                "Edit the picker; ColorFieldEvent and SliderEvent variants appear below.",
            )
        });

        let subscriptions = wire_picker_events(&state, &event_stream, cx);

        Self { look, state, event_stream, _subscriptions: subscriptions }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look.clone();
        self.state.update(cx, |demo, cx| demo.sync_look(look, cx));
        self.event_stream.update(cx, |stream, cx| stream.sync_look(self.look.clone(), cx));
        cx.notify();
    }
}

impl Render for ColorPickerControlExposition {
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
                "Photoshop-style Picker",
                "A composed picker built from the migrated field and slider primitives.",
                render_demo_card_with_padding(
                    look,
                    ColorPickerDemo::card_width(PICKER_HORIZONTAL_PADDING),
                    PICKER_HORIZONTAL_PADDING,
                    PICKER_VERTICAL_PADDING,
                    self.state.clone(),
                ),
            ))
            .child(self.event_stream.clone());

        render_composition_exposition("color-picker", preview.into_any_element())
    }
}

struct ColorPickerDemo {
    look: Arc<Look>,
    metrics: ColorPickerMetrics,
    sync: ColorCompositionSync,
    field: Entity<ColorFieldState>,
    hue_slider: Entity<SliderControl>,
    alpha_slider: Entity<SliderControl>,
    alpha_domain: Arc<ColorSliderDomainRenderer>,
    hsv: Hsv,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy)]
struct ColorPickerMetrics {
    control_width: f32,
    swatch_height: f32,
}

impl ColorPickerMetrics {
    fn new() -> Self {
        let control_width: f32 = 218.0;
        let scale = control_width / 260.0;
        Self { control_width, swatch_height: (44.0 * scale).max(32.0) }
    }

    fn card_width(self, horizontal_padding: f32) -> f32 {
        composition_card_width(self.control_width, horizontal_padding)
    }
}

impl ColorPickerDemo {
    pub fn card_width(horizontal_padding: f32) -> f32 {
        ColorPickerMetrics::new().card_width(horizontal_padding)
    }

    fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let hsv = Hsv { h: 12.0, s: 0.78, v: 0.86, a: 0.92 };
        let metrics = ColorPickerMetrics::new();

        let field_radius = px(look.metrics().radius.sm);

        let field = cx.new(move |_| {
            ColorFieldState::saturation_value("controls-doc-color-picker-field", hsv, 20.0)
                .rounded(field_radius)
                .vector()
        });
        let hue_slider = ColorSliderBuilder::hue("controls-doc-color-picker-hue", hsv.h)
            .size(ControlSize::Sm)
            .thumb_medium()
            .edge_to_edge()
            .spawn(cx);
        let alpha_builder = ColorSliderBuilder::alpha("controls-doc-color-picker-alpha", hsv.a, hsv)
            .size(ControlSize::Sm)
            .thumb_medium()
            .edge_to_edge();
        let alpha_domain = alpha_builder.domain_renderer();
        let alpha_slider = alpha_builder.spawn(cx);

        let subscriptions = vec![
            cx.subscribe(&field, |this, _, event: &ColorFieldEvent, cx| {
                let hsv = match event {
                    ColorFieldEvent::Change(hsv) | ColorFieldEvent::Release(hsv) => *hsv,
                    _ => return,
                };
                let Some(_sync_guard) = this.sync.begin_guard() else {
                    return;
                };
                this.hsv.h = hsv.h;
                this.hsv.s = hsv.s;
                this.hsv.v = hsv.v;
                this.sync_controls(cx, false);
                cx.notify();
            }),
            cx.subscribe(&hue_slider, |this, _, event, cx| {
                let Some(hue) = primary_slider_value(event) else {
                    return;
                };
                let Some(_sync_guard) = this.sync.begin_guard() else {
                    return;
                };
                this.hsv.h = hue;
                this.sync_controls(cx, true);
                cx.notify();
            }),
            cx.subscribe(&alpha_slider, |this, _, event, cx| {
                let Some(alpha) = primary_slider_value(event) else {
                    return;
                };
                let Some(_sync_guard) = this.sync.begin_guard() else {
                    return;
                };
                this.hsv.a = alpha;
                this.sync_controls(cx, true);
                cx.notify();
            }),
        ];

        Self {
            look,
            metrics,
            field,
            hue_slider,
            alpha_slider,
            alpha_domain,
            sync: ColorCompositionSync::new(),
            hsv,
            _subscriptions: subscriptions,
        }
    }

    fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look;
        self.field.update(cx, |_, cx| cx.notify());
        self.hue_slider.update(cx, |_, cx| cx.notify());
        self.alpha_slider.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn sync_controls(&self, cx: &mut Context<Self>, sync_field: bool) {
        let hsv = self.hsv;

        if sync_field {
            self.field.update(cx, |field, cx| {
                field.set_hsv(hsv, cx);
            });
        }

        self.sync.sync_slider_value(&self.hue_slider, hsv.h, cx);
        self.sync.sync_color_slider(
            &self.alpha_slider,
            &self.alpha_domain,
            Arc::new(AlphaDelegate { spec: hsv }),
            self.alpha_domain.context(),
            hsv.a,
            cx,
        );
    }
}

impl Render for ColorPickerDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let selected = self.hsv.to_hsla_ext();
        let control_width = self.metrics.control_width;
        let swatch_height = self.metrics.swatch_height;
        let look = &self.look;
        let text_size = TextSize::Base;

        div()
            .w_full()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(14.0))
            .child(div().size(px(control_width)).child(self.field.clone()))
            .child(div().w(px(control_width)).child(self.hue_slider.clone()))
            .child(div().w(px(control_width)).child(self.alpha_slider.clone()))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        ColorSwatch::new(from_hsla(selected))
                            .checkerboard(true)
                            .height(px(swatch_height))
                            .rounded(px(composition_inset_radius(look))),
                    )
                    .child(detail_row_sized(look, "Hex", format_hex_color(selected), text_size))
                    .child(detail_row_sized(look, "HSLA", format_compact_hsla(selected), text_size)),
            )
    }
}

fn wire_picker_events(
    state: &Entity<ColorPickerDemo>,
    event_stream: &Entity<ControlEventStream>,
    cx: &mut Context<ColorPickerControlExposition>,
) -> Vec<Subscription> {
    let field = state.read(cx).field.clone();
    let hue_slider = state.read(cx).hue_slider.clone();
    let alpha_slider = state.read(cx).alpha_slider.clone();
    let event_stream = event_stream.clone();

    vec![
        cx.subscribe(&field, {
            let event_stream = event_stream.clone();
            move |_, _, event: &ColorFieldEvent, cx| {
                if let Some(line) = format_color_field_event(event) {
                    event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
                }
            }
        }),
        cx.subscribe(&hue_slider, {
            let event_stream = event_stream.clone();
            move |_, _, event: &SliderEvent, cx| {
                if let Some(line) = format_slider_event("Hue", event) {
                    event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
                }
            }
        }),
        cx.subscribe(&alpha_slider, move |_, _, event: &SliderEvent, cx| {
            if let Some(line) = format_slider_event("Alpha", event) {
                event_stream.update(cx, |stream, cx| stream.append_line(&line, cx));
            }
        }),
    ]
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    use gpui::{Focusable, TestAppContext};
    use gpui_luma::controls::tabs::TabsEvent;
    use super::super::{ColorCompositions, EXAMPLES};

    #[test]
    fn composition_pages_render_and_preserve_picker_edits_when_switching() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            gpui_luma::init(cx).expect("initialize SDK");
            gpui_luma::key_handling::bind_default_control_keys(cx);
        });
        let (gallery, cx) = app.add_window_view(|window, cx| {
            window.activate_window();
            ColorCompositions::new(Arc::new(Look::built_in()), cx)
        });
        cx.run_until_parked();
        let (picker, tabs) = cx.update(|_, cx| {
            let gallery = gallery.read(cx);
            (
                gallery.pages[0].clone().downcast::<ColorPickerControlExposition>().expect("picker page"),
                gallery.tabs.clone(),
            )
        });
        let (state, events, hue_slider, before) = cx.update(|window, cx| {
            let picker = picker.read(cx);
            let state = picker.state.clone();
            let events = picker.event_stream.clone();
            let demo = state.read(cx);
            let slider = demo.hue_slider.clone();
            let before = demo.hsv.h;
            slider.read(cx).focus_handle(cx).focus(window, cx);
            (state, events, slider, before)
        });
        cx.simulate_keystrokes("right");
        cx.run_until_parked();
        let edited = cx.update(|_, cx| state.read(cx).hsv);
        assert!(edited.h > before, "the copied hue slider must still update the composed picker");

        for (index, (id, label)) in EXAMPLES.iter().enumerate() {
            tabs.update(cx, |tabs, cx| {
                tabs.set_active(*id, cx);
                cx.emit(TabsEvent::Activate { tab_id: (*id).into(), label: (*label).into() });
            });
            cx.run_until_parked();
            cx.update(|_, cx| assert_eq!(gallery.read(cx).active, index));
        }
        tabs.update(cx, |tabs, cx| {
            tabs.set_active("picker", cx);
            cx.emit(TabsEvent::Activate { tab_id: "picker".into(), label: "Color Picker".into() });
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert_eq!(state.read(cx).hsv, edited);
            assert_eq!(picker.read(cx).event_stream, events);
            assert_eq!(state.read(cx).hue_slider, hue_slider);
        });
    }
}
