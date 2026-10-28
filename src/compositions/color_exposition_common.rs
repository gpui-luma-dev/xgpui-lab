//! Shared layout and readout helpers for color control expositions.

use gpui_luma::color::gpui_bridge::from_hsla;
use gpui::{FontWeight, Hsla, div, prelude::*, px};
use gpui_luma_color::ColorSwatch;
use gpui_luma_color::color_field::ColorFieldEvent;
use gpui_luma::controls::slider::SliderEvent;
use crate::theme::{Look, ColorVizLookExt, TypographyExt, TextSize};

/// Inner demo-card horizontal padding (`render_demo_card` default).
pub(super) const COMPOSITION_DEMO_CARD_PADDING_X: f32 = 18.0;
pub(super) const COMPOSITION_PRIMARY_READOUT_GAP: f32 = 10.0;
pub(super) const COMPOSITION_READOUT_ROW_GAP: f32 = 4.0;
pub(super) const COMPOSITION_READOUT_SWATCH_HEIGHT: f32 = 24.0;

pub(super) fn composition_demo_card_width(content_width: f32) -> f32 {
    content_width + COMPOSITION_DEMO_CARD_PADDING_X * 2.0
}

pub(super) fn composition_card_width(content_width: f32, horizontal_padding: f32) -> f32 {
    content_width + horizontal_padding * 2.0
}

pub(super) fn composition_card_radius(look: &Look) -> f32 {
    look.metrics().radius.sm
}

pub(super) fn composition_inset_radius(look: &Look) -> f32 {
    look.metrics().radius.sm
}

pub(super) fn render_demo_section(
    look: &Look,
    title: &'static str,
    description: &'static str,
    content: gpui::AnyElement,
) -> gpui::AnyElement {
    let chrome = look.chrome();
    let title_style = look.typography_scale(TextSize::Sm);
    let description_style = look.typography_scale(TextSize::Sm);

    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.0))
        .child(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(2.0))
                .child(
                    div()
                        .typography_style(title_style)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(chrome.title_text)
                        .child(title),
                )
                .child(div().typography_style(description_style).text_color(chrome.muted_text).child(description)),
        )
        .child(content)
        .into_any_element()
}

pub(super) fn render_demo_card(look: &Look, width_px: f32, content: impl IntoElement) -> gpui::AnyElement {
    render_demo_card_with_padding(look, width_px, 18.0, 18.0, content)
}

pub(super) fn render_demo_card_with_padding(
    look: &Look,
    width_px: f32,
    horizontal_padding: f32,
    vertical_padding: f32,
    content: impl IntoElement,
) -> gpui::AnyElement {
    let chrome = look.chrome();

    div()
        .w(px(width_px))
        .max_w_full()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .rounded(px(composition_card_radius(look)))
        .border_1()
        .border_color(chrome.border)
        .bg(chrome.panel_background)
        .px(px(horizontal_padding))
        .py(px(vertical_padding))
        .child(div().w_full().min_w(px(0.0)).pb(px(2.0)).child(content))
        .into_any_element()
}

pub(super) fn detail_row_sized(
    look: &Look,
    label: &'static str,
    value: String,
    text_size: TextSize,
) -> gpui::AnyElement {
    let chrome = look.chrome();
    let text_style = look.typography_scale(text_size);

    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .flex_shrink_0()
                .typography_style(text_style)
                .font_weight(FontWeight::MEDIUM)
                .text_color(chrome.muted_text)
                .whitespace_nowrap()
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .typography_style(text_style)
                .text_color(chrome.body_text)
                .whitespace_nowrap()
                .child(value),
        )
        .into_any_element()
}

/// Compact Hex/HSLA footer shared by ring/plane compositions.
///
/// `preview_width`: when set, renders a thin swatch strip matching the primary control width.
pub(super) fn render_composition_readout_footer(
    look: &Look,
    color: Hsla,
    text_size: TextSize,
    preview_width: Option<f32>,
) -> gpui::AnyElement {
    let radius = px(composition_inset_radius(look));
    let hex = format_hex_color(color);
    let hsla = format_compact_hsla(color);

    let mut footer = div().w_full().flex().flex_col().gap(px(COMPOSITION_READOUT_ROW_GAP));

    if let Some(width) = preview_width {
        footer = footer.child(
            div().w_full().flex().justify_center().child(
                div().w(px(width)).child(
                    ColorSwatch::new(from_hsla(color))
                        .checkerboard(false)
                        .height(px(COMPOSITION_READOUT_SWATCH_HEIGHT))
                        .rounded(radius),
                ),
            ),
        );
    }

    footer
        .child(detail_row_sized(look, "Hex", hex, text_size))
        .child(detail_row_sized(look, "HSLA", hsla, text_size))
        .into_any_element()
}

pub(super) fn control_label_sized(look: &Look, label: &'static str, text_size: TextSize) -> gpui::AnyElement {
    div()
        .flex_shrink_0()
        .typography_style(look.typography_scale(text_size))
        .font_weight(FontWeight::MEDIUM)
        .text_color(look.chrome().muted_text)
        .whitespace_nowrap()
        .child(label)
        .into_any_element()
}

pub(super) fn format_hsl_label(color: Hsla) -> String {
    format!(
        "{} {}% {}%",
        rounded_channel(color.h * 360.0),
        rounded_channel(color.s * 100.0),
        rounded_channel(color.l * 100.0)
    )
}

pub(super) fn slider_labeled_row_compact_sized(
    look: &Look,
    label: &'static str,
    slider: gpui::Entity<gpui_luma::controls::slider::SliderControl>,
    label_text_size: TextSize,
) -> gpui::AnyElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(div().flex_shrink_0().child(control_label_sized(look, label, label_text_size)))
        .child(div().flex_1().min_w(px(0.0)).child(slider))
        .into_any_element()
}

pub(super) fn format_color_field_event(event: &ColorFieldEvent) -> Option<String> {
    match event {
        ColorFieldEvent::Change(hsv) => Some(format!(
            "ColorFieldEvent::Change(Hsv {{ h: {:.1}, s: {:.3}, v: {:.3}, a: {:.3} }})",
            hsv.h, hsv.s, hsv.v, hsv.a
        )),
        ColorFieldEvent::Release(hsv) => Some(format!(
            "ColorFieldEvent::Release(Hsv {{ h: {:.1}, s: {:.3}, v: {:.3}, a: {:.3} }})",
            hsv.h, hsv.s, hsv.v, hsv.a
        )),
        ColorFieldEvent::DragStart { hsv } => Some(format!(
            "ColorFieldEvent::DragStart {{ hsv: Hsv {{ h: {:.1}, s: {:.3}, v: {:.3} }} }}",
            hsv.h, hsv.s, hsv.v
        )),
        ColorFieldEvent::DragEnd { hsv } => Some(format!(
            "ColorFieldEvent::DragEnd {{ hsv: Hsv {{ h: {:.1}, s: {:.3}, v: {:.3} }} }}",
            hsv.h, hsv.s, hsv.v
        )),
        ColorFieldEvent::HoverChanged { hovered } => {
            Some(format!("ColorFieldEvent::HoverChanged {{ hovered: {hovered} }}"))
        }
        ColorFieldEvent::EnabledChanged { enabled } => {
            Some(format!("ColorFieldEvent::EnabledChanged {{ enabled: {enabled} }}"))
        }
        _ => None,
    }
}

pub(super) fn format_slider_event(source: &str, event: &SliderEvent) -> Option<String> {
    match event {
        SliderEvent::Change { value, .. } => Some(format!("SliderEvent::Change - {source} ({value:.3})")),
        SliderEvent::Release { value, .. } => Some(format!("SliderEvent::Release - {source} ({value:.3})")),
        SliderEvent::DragStart { .. } => Some(format!("SliderEvent::DragStart - {source}")),
        SliderEvent::DragEnd { value, .. } => Some(format!("SliderEvent::DragEnd - {source} ({value:.3})")),
        SliderEvent::FocusChanged { focused } => Some(format!("SliderEvent::FocusChanged {{ focused: {focused} }}")),
        SliderEvent::HoverChanged { hovered } => Some(format!("SliderEvent::HoverChanged {{ hovered: {hovered} }}")),
        SliderEvent::EnabledChanged { enabled } => {
            Some(format!("SliderEvent::EnabledChanged {{ enabled: {enabled} }}"))
        }
        _ => None,
    }
}

pub(super) fn format_compact_hsla(color: Hsla) -> String {
    format!(
        "hsla({} {}% {}% / {})",
        rounded_channel(color.h * 360.0),
        rounded_channel(color.s * 100.0),
        rounded_channel(color.l * 100.0),
        compact_alpha(color.a)
    )
}

pub(super) fn format_hex_color(color: Hsla) -> String {
    let (r, g, b) = hsla_to_rgb8(color);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn rounded_channel(value: f32) -> i32 {
    value.round() as i32
}

fn compact_alpha(alpha: f32) -> String {
    let rounded = (alpha * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < f32::EPSILON {
        format!("{}", rounded.round() as i32)
    } else {
        format!("{rounded:.2}")
    }
}

fn hsla_to_rgb8(color: Hsla) -> (u8, u8, u8) {
    let h = color.h.fract() * 6.0;
    let s = color.s.clamp(0.0, 1.0);
    let l = color.l.clamp(0.0, 1.0);

    if s <= f32::EPSILON {
        let gray = (l * 255.0).round() as u8;
        return (gray, gray, gray);
    }

    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let r = hue_to_channel(p, q, h + 2.0);
    let g = hue_to_channel(p, q, h);
    let b = hue_to_channel(p, q, h - 2.0);
    ((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
}

fn hue_to_channel(p: f32, q: f32, t: f32) -> f32 {
    let mut t = t;
    if t < 0.0 {
        t += 6.0;
    }
    if t >= 6.0 {
        t -= 6.0;
    }
    if t < 1.0 {
        p + (q - p) * t
    } else if t < 3.0 {
        q
    } else if t < 4.0 {
        p + (q - p) * (4.0 - t)
    } else {
        p
    }
}
