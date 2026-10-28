use std::sync::Arc;

use gpui::{Context, Entity, FontWeight, Render, Window, div, prelude::*, px};
use crate::theme::{Look, ColorVizLookExt, TypographyExt, TextSize};

use super::template::controls_mono_font;
use super::event_log_view::{EventLogView, EventLogViewLookExt, radix_event_log_theme};

pub struct ControlEventStream {
    look: Arc<Look>,
    log_id: &'static str,
    intro: &'static str,
    event_log: Entity<EventLogView>,
    next_index: usize,
}

impl ControlEventStream {
    pub fn new(cx: &mut Context<Self>, look: Arc<Look>, log_id: &'static str, intro: &'static str) -> Self {
        let event_log = look
            .event_log_view(log_id)
            .placeholder("Event history will appear here…")
            .full_width(true)
            .rows(6)
            .font_family(controls_mono_font())
            .spawn(cx);

        Self { look, log_id, intro, event_log, next_index: 1 }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look.clone();
        self.event_log.update(cx, |log, cx| {
            log.set_theme(radix_event_log_theme(look), cx);
            log.set_font_family(controls_mono_font(), cx);
        });
        cx.notify();
    }

    pub fn append_line(&mut self, line: &str, cx: &mut Context<Self>) {
        let line = format!("[{}] {line}", self.next_index);
        self.next_index += 1;
        self.event_log.update(cx, |log, cx| log.append_line(&line, cx));
        cx.notify();
    }
}

impl Render for ControlEventStream {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let chrome = self.look.chrome();
        let body_style = self.look.typography_scale(TextSize::Xs);
        let title_style = self.look.typography_scale(TextSize::Sm);

        div()
            .id(self.log_id)
            .w_full()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(12.0))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(4.0))
                    .child(
                        div()
                            .typography_style(title_style)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(chrome.title_text)
                            .child("Eventing"),
                    )
                    .child(div().w_full().typography_style(body_style).text_color(chrome.muted_text).child(self.intro)),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(6.0))
                    .child(
                        div()
                            .typography_style(body_style)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(chrome.title_text)
                            .child("Event stream"),
                    )
                    .child(self.event_log.clone()),
            )
    }
}
