//! Shared presentation chrome for the Color Spaces and Italy labs.
use gpui::{prelude::*, *};
use gpui_luma_look_radix::Look;
use crate::theme::{ColorVizLookExt, TypographyExt};

pub(crate) fn page(id: &'static str, look: &Look) -> Stateful<Div> {
    let chrome = look.chrome();
    div()
        .id(id)
        .size_full()
        .overflow_y_scroll()
        .bg(chrome.content_background)
        .text_color(chrome.body_text)
}

pub(crate) fn container() -> Div {
    div().flex().flex_col().w_full().max_w(px(1240.)).mx_auto().p_6().gap_5()
}

pub(crate) fn header(look: &Look, title: &'static str, subtitle: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_3xl().font_weight(FontWeight::SEMIBOLD).child(title))
        .child(caption(look, subtitle))
}

pub(crate) fn caption(look: &Look, text: impl Into<SharedString>) -> Div {
    div().text_sm().text_color(look.chrome().muted_text).child(text.into())
}

pub(crate) fn control_row() -> Div {
    div().flex().flex_wrap().items_center().gap_3()
}

/// Apply the reference switch label typography to a labeled control.
pub(crate) fn control(look: &Look, element: Div) -> Div {
    let label = gpui_luma_look_radix::switch_theme(look).resolve(
        false,
        gpui_luma::theme::InteractionState::default(),
        gpui_luma::theme::ControlSize::Md,
    );
    element
        .typography_style(label.label_typography)
        .font_family(label.label_font_family)
        .text_color(label.label_color)
}

pub(crate) fn panel(look: &Look) -> Div {
    let chrome = look.chrome();
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .bg(chrome.panel_background)
        .border_1()
        .border_color(chrome.border)
}

pub(crate) fn comparison_card(look: &Look) -> Div {
    comparison_surface(div().flex().flex_col(), look)
}

/// Card chrome can also be applied to a Luma button template row.
pub(crate) fn comparison_surface<V: Styled>(view: V, look: &Look) -> V {
    let chrome = look.chrome();
    view.p_3().gap_3().rounded_lg().bg(chrome.panel_background).border_1().border_color(chrome.border)
}
