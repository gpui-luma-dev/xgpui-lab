//! Local copies of Studio's color composition examples and their event streams.

use crate::theme::ColorVizLookExt;

mod color_compositions;
mod color_exposition_common;
mod color_harmonies;
mod color_hsv_plane;
mod color_hsv_wheel;
mod color_picker;
mod color_split_ring;
mod color_sv_triangle;
mod event_log_view;
mod event_stream;
mod template;

use std::sync::Arc;

use gpui::{AnyView, Context, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma::controls::scroll_container::ScrollContainer;
use gpui_luma::controls::scrollbar::ScrollbarEvent;
use gpui_luma_look_radix::Look;

use color_harmonies::ColorHarmoniesControlExposition;
use color_hsv_plane::ColorHsvPlaneControlExposition;
use color_hsv_wheel::ColorHsvWheelControlExposition;
use color_picker::ColorPickerControlExposition;
use color_split_ring::ColorSplitRingControlExposition;
use color_sv_triangle::ColorSvTriangleControlExposition;

const EXAMPLES: [(&str, &str); 6] = [
    ("picker", "Color Picker"),
    ("hsv-plane", "HSV Plane"),
    ("hsv-wheel", "HSV Wheel"),
    ("sv-triangle", "SV Triangle"),
    ("split-ring", "Split Ring"),
    ("harmonies", "Color Harmonies"),
];

pub struct ColorCompositions {
    look: Arc<Look>,
    pages: [AnyView; 6],
    scrolls: [ScrollContainer; 6],
    active: usize,
    _subscriptions: Vec<Subscription>,
}

impl ColorCompositions {
    pub(crate) fn activate_page(&mut self, page: &str, cx: &mut Context<Self>) {
        if let Some(index) = EXAMPLES.iter().position(|(id, _)| *id == page) {
            self.active = index;
            cx.notify();
        }
    }

    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        // Retain every example so switching pages preserves edits and event history.
        let pages = [
            cx.new(|cx| ColorPickerControlExposition::new(cx, look.clone())).into(),
            cx.new(|cx| ColorHsvPlaneControlExposition::new(cx, look.clone())).into(),
            cx.new(|cx| ColorHsvWheelControlExposition::new(cx, look.clone())).into(),
            cx.new(|cx| ColorSvTriangleControlExposition::new(cx, look.clone())).into(),
            cx.new(|cx| ColorSplitRingControlExposition::new(cx, look.clone())).into(),
            cx.new(|cx| ColorHarmoniesControlExposition::new(cx, look.clone())).into(),
        ];
        let scrolls = EXAMPLES.map(|(id, _)| {
            ScrollContainer::new(format!("color-viz-composition-{id}-scroll"), look.scrollbar_template(), cx)
                .overlay(true)
                .auto_hide(true)
        });
        let mut subscriptions = Vec::new();
        for (index, scroll) in scrolls.iter().enumerate() {
            subscriptions.push(cx.subscribe(&scroll.scrollbar(), move |this, _, event: &ScrollbarEvent, cx| {
                if let ScrollbarEvent::Change { value } = event {
                    this.scrolls[index].set_vertical_offset(*value, cx);
                }
            }));
        }
        Self { look, pages, scrolls, active: 0, _subscriptions: subscriptions }
    }
}

impl Render for ColorCompositions {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chrome = self.look.chrome();
        let scroll = &self.scrolls[self.active];
        scroll.sync_scrollbar(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_color(chrome.body_text)
            .bg(chrome.content_background)
            .child(div().flex_1().min_h_0().child(
                scroll.render(div().w_full().pt(px(16.0)).child(self.pages[self.active].clone()).into_any_element()),
            ))
    }
}
