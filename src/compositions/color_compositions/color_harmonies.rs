#![allow(clippy::too_many_arguments)]

//! Color harmonies composition — HSL hue/saturation wheel at current lightness, with harmony palette readout.

use std::sync::Arc;

use gpui::{
    AnyElement, Context, Entity, FontWeight, Hsla, Render, Subscription, Window, div, hsla, prelude::*, px, rgb,
    transparent_black,
};
use gpui_luma_color::color_field::{CircleDomain, ColorFieldEvent, ColorFieldState, HslWheelModel};
use gpui_luma_color::color_ring::{
    ColorRingBuilder, ColorRingDomainRenderer, ColorRingTrackContext, LightnessRingDelegate, primary_slider_value,
};
use gpui_luma_color::color_slider::color_spec::Hsv;
use gpui_luma_color::composition::ColorCompositionSync;
use gpui_luma::controls::selector::{Selector, SelectorEvent, SelectorItem};
use gpui_luma::controls::slider::SliderControl;
use gpui_luma::controls::textfield::{TextField, TextFieldEvent};
use gpui_luma::vstack;
use gpui_luma_look_radix as radix;
use crate::theme::{Look, ColorVizLookExt, TypographyExt, TextSize};

use super::super::color_exposition_common::{composition_demo_card_width, format_hsl_label};

const COMPONENT_GAP_PX: f32 = 20.0;
const ROW_GAP_PX: f32 = 12.0;
const COLOR_SWATCH_SIZE: f32 = 48.0;
const MIN_COLOR_INPUT_WIDTH: f32 = 152.0;
const MIN_SELECTOR_WIDTH: f32 = 158.0;
const COLOR_LABEL_RESERVE: f32 = 40.0;
const COMBINATION_LABEL_RESERVE: f32 = 88.0;

pub struct ColorHarmoniesDemo {
    look: Arc<Look>,
    metrics: ColorHarmoniesMetrics,
    sync: ColorCompositionSync,
    wheel: Entity<ColorFieldState>,
    lightness_ring: Entity<SliderControl>,
    lightness_renderer: Arc<ColorRingDomainRenderer>,
    lightness_context: ColorRingTrackContext,
    harmony_menu: Entity<Selector>,
    color_input: TextField,
    color_input_programmatic_update: bool,
    color: Hsla,
    selected_combination: ColorCombination,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy)]
struct ColorHarmoniesMetrics {
    ring_size: f32,
    ring_canvas_padding: f32,
    ring_to_wheel_inner_gap: f32,
    ring_thickness: f32,
    wheel_thumb_size: f32,
    ring_thumb_size: f32,
}

impl ColorHarmoniesMetrics {
    fn form_content_width() -> f32 {
        let color_row_width = COLOR_LABEL_RESERVE + ROW_GAP_PX + COLOR_SWATCH_SIZE + ROW_GAP_PX + MIN_COLOR_INPUT_WIDTH;
        let combination_row_width = COMBINATION_LABEL_RESERVE + ROW_GAP_PX + MIN_SELECTOR_WIDTH;
        color_row_width.max(combination_row_width)
    }

    fn new() -> Self {
        let mut metrics = Self::proportional();
        let content_width = Self::form_content_width();
        let wheel_block = metrics.ring_size + metrics.ring_canvas_padding;
        if wheel_block < content_width {
            metrics.ring_size += content_width - wheel_block;
            metrics.wheel_thumb_size = (metrics.wheel_size() * 0.07).max(10.0);
        }
        metrics
    }

    fn proportional() -> Self {
        let ring_size: f32 = 238.0;
        let scale = ring_size / 300.0;
        let ring_thickness = 20.0 * scale;
        let ring_to_wheel_inner_gap = (12.0 * scale).max(8.0);
        let wheel_size = (ring_size - 2.0 * ring_thickness - ring_to_wheel_inner_gap).max(40.0);

        Self {
            ring_size,
            ring_canvas_padding: (20.0 * scale).max(12.0),
            ring_to_wheel_inner_gap,
            ring_thickness,
            wheel_thumb_size: (wheel_size * 0.07).max(10.0),
            ring_thumb_size: (14.0 * scale).max(12.0),
        }
    }

    fn wheel_size(self) -> f32 {
        (self.ring_size - 2.0 * self.ring_thickness - self.ring_to_wheel_inner_gap).max(40.0)
    }

    fn wheel_half(self) -> f32 {
        self.wheel_size() * 0.5
    }

    fn content_width(self) -> f32 {
        (self.ring_size + self.ring_canvas_padding).max(Self::form_content_width())
    }

    fn palette_swatch_diameter(self, swatch_count: usize) -> f32 {
        if swatch_count == 0 {
            return COLOR_SWATCH_SIZE;
        }

        let count = swatch_count as f32;
        let gap_total = (swatch_count.saturating_sub(1)) as f32 * ROW_GAP_PX;
        ((self.content_width() - gap_total) / count).clamp(0.0, COLOR_SWATCH_SIZE)
    }

    fn card_width(self) -> f32 {
        composition_demo_card_width(self.content_width())
    }
}

impl ColorHarmoniesDemo {
    pub fn card_width() -> f32 {
        ColorHarmoniesMetrics::new().card_width()
    }

    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let color: Hsla = rgb(0x47c424).into();
        let metrics = ColorHarmoniesMetrics::new();

        let wheel = cx.new(|_| {
            ColorFieldState::new(
                "controls-doc-color-harmonies-wheel",
                hsla_to_wheel_hsv(color),
                Arc::new(CircleDomain),
                Arc::new(HslWheelModel),
            )
            .thumb_size(metrics.wheel_thumb_size)
            .inside_field()
            .raster_image_prewarmed_square(metrics.wheel_size())
        });
        let lightness_builder =
            ColorRingBuilder::lightness("controls-doc-color-harmonies-lightness", color.l, color.h * 360.0, color.s)
                .size(px(metrics.ring_size))
                .ring_thickness(metrics.ring_thickness)
                .thumb_size(metrics.ring_thumb_size)
                .allow_inner_target(true)
                .rotation_degrees(180.0);
        let lightness_renderer = lightness_builder.domain_renderer();
        let lightness_context = lightness_builder.track_context();
        let lightness_ring = lightness_builder.spawn(cx);
        let harmony_menu = radix::Selector::new("controls-doc-color-harmonies-harmony")
            .look(look.as_ref())
            .label("Combination")
            .size(radix::ButtonSize::Two)
            .items(color_combination_items())
            .selected_id(ColorCombination::Tetradic.id())
            .spawn(cx);
        let color_input = radix::TextField::new("controls-doc-color-harmonies-input")
            .look(look.as_ref())
            .value(format_hsl_input(color))
            .placeholder("hsl(120 50% 40%) or #006081")
            .size(radix::TextFieldSize::Two)
            .full_width(true)
            .spawn(cx);

        let subscriptions = vec![
            cx.subscribe(&wheel, |this, _, event: &ColorFieldEvent, cx| {
                let hsv = match event {
                    ColorFieldEvent::Change(hsv) | ColorFieldEvent::Release(hsv) => *hsv,
                    _ => return,
                };
                if !this.sync.begin_sync() {
                    return;
                }
                this.color.h = (hsv.h / 360.0).rem_euclid(1.0);
                this.color.s = hsv.s.clamp(0.0, 1.0);
                this.sync_ring(cx);
                this.sync.end_sync();
                cx.notify();
            }),
            cx.subscribe(&lightness_ring, |this, _, event, cx| {
                let Some(lightness) = primary_slider_value(event) else {
                    return;
                };
                if !this.sync.begin_sync() {
                    return;
                }
                this.color.l = lightness.clamp(0.0, 1.0);
                this.sync_wheel(cx);
                this.sync_ring(cx);
                this.sync.end_sync();
                cx.notify();
            }),
            cx.subscribe(&harmony_menu, |this, _, event: &SelectorEvent, cx| {
                if let SelectorEvent::Change { item_id, .. } = event {
                    this.selected_combination = ColorCombination::from_id(item_id.as_ref());
                    cx.notify();
                }
            }),
            cx.subscribe(&color_input, |this, _, event: &TextFieldEvent, cx| {
                if this.color_input_programmatic_update {
                    return;
                }

                if let TextFieldEvent::Change { value } | TextFieldEvent::Submit { value } = event
                    && let Some(color) = parse_color_input(value)
                {
                    this.color = color;
                    this.sync_wheel(cx);
                    this.sync_ring(cx);
                    cx.notify();
                }
            }),
        ];

        Self {
            look,
            metrics,
            sync: ColorCompositionSync::new(),
            wheel,
            lightness_ring,
            lightness_renderer,
            lightness_context,
            harmony_menu,
            color_input,
            color_input_programmatic_update: false,
            color,
            selected_combination: ColorCombination::Tetradic,
            _subscriptions: subscriptions,
        }
    }

    pub fn sync_look(&mut self, look: Arc<Look>, cx: &mut Context<Self>) {
        self.look = look;
        self.wheel.update(cx, |_, cx| cx.notify());
        self.lightness_ring.update(cx, |_, cx| cx.notify());
        self.harmony_menu.update(cx, |_, cx| cx.notify());
        self.color_input.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn wheel(&self) -> Entity<ColorFieldState> {
        self.wheel.clone()
    }

    pub fn lightness_ring(&self) -> Entity<SliderControl> {
        self.lightness_ring.clone()
    }

    fn sync_ring(&mut self, cx: &mut Context<Self>) {
        let color = self.color;
        self.sync.sync_color_ring(
            &self.lightness_ring,
            &self.lightness_renderer,
            Arc::new(LightnessRingDelegate { hue: color.h * 360.0, saturation: color.s }),
            self.lightness_context.clone(),
            color.l,
            cx,
        );
        self.sync_color_input(cx);
    }

    fn sync_wheel(&mut self, cx: &mut Context<Self>) {
        let color = self.color;
        self.wheel.update(cx, |wheel, cx| {
            let hsv = hsla_to_wheel_hsv(color);
            wheel.set_hsv_components(hsv.h, hsv.s, hsv.v, cx);
        });
        self.sync_color_input(cx);
    }

    fn sync_color_input(&mut self, cx: &mut Context<Self>) {
        let value = format_hsl_input(self.color);
        if self.color_input.read(cx).value() == value {
            return;
        }

        self.color_input_programmatic_update = true;
        self.color_input.update(cx, |input, cx| input.set_value(value, cx));
        self.color_input_programmatic_update = false;
    }
}

impl Render for ColorHarmoniesDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let color = self.color;
        let swatches = self.selected_combination.palette(color);
        let harmony_points = harmony_points_from_swatches(&swatches);
        let look = &self.look;
        let title_text = look.chrome().title_text;
        let border = look.chrome().border;
        let metrics = self.metrics;
        let label_text_size = TextSize::Base;

        render_combinations_body(
            look,
            title_text,
            border,
            label_text_size,
            self.lightness_ring.clone(),
            self.wheel.clone(),
            self.color_input.clone(),
            self.harmony_menu.clone(),
            metrics,
            color,
            harmony_points,
            swatches,
        )
    }
}

/// Maps the shared HSL color into wheel HSV — `v` carries lightness so the disk re-tints with the ring.
fn hsla_to_wheel_hsv(color: Hsla) -> Hsv {
    Hsv {
        h: (color.h * 360.0).rem_euclid(360.0),
        s: color.s.clamp(0.0, 1.0),
        v: color.l.clamp(0.0, 1.0),
        a: color.a.clamp(0.0, 1.0),
    }
}

fn format_hsl_input(color: Hsla) -> String {
    format!("hsl({})", format_hsl_label(color))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ColorCombination {
    Monochromatic,
    Complementary,
    Analogous,
    Triadic,
    #[default]
    Tetradic,
    Pentadic,
    Hexadic,
}

impl ColorCombination {
    fn id(self) -> &'static str {
        match self {
            Self::Monochromatic => "monochromatic",
            Self::Complementary => "complementary",
            Self::Analogous => "analogous",
            Self::Triadic => "triadic",
            Self::Tetradic => "tetradic",
            Self::Pentadic => "pentadic",
            Self::Hexadic => "hexadic",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Monochromatic => "Monochromatic",
            Self::Complementary => "Complementary",
            Self::Analogous => "Analogous",
            Self::Triadic => "Triadic",
            Self::Tetradic => "Tetradic",
            Self::Pentadic => "Pentadic",
            Self::Hexadic => "Hexadic",
        }
    }

    fn from_id(id: &str) -> Self {
        match id {
            "monochromatic" => Self::Monochromatic,
            "complementary" => Self::Complementary,
            "analogous" => Self::Analogous,
            "triadic" => Self::Triadic,
            "tetradic" => Self::Tetradic,
            "pentadic" => Self::Pentadic,
            "hexadic" => Self::Hexadic,
            _ => Self::Tetradic,
        }
    }

    fn palette(self, base: Hsla) -> Vec<CombinationSwatch> {
        match self {
            Self::Monochromatic => monochromatic_palette(base),
            Self::Complementary => complementary_palette(base),
            Self::Analogous => analogous_palette(base),
            Self::Triadic => triadic_palette(base),
            Self::Tetradic => tetradic_palette(base),
            Self::Pentadic => pentadic_palette(base),
            Self::Hexadic => hexadic_palette(base),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CombinationSwatch {
    role: &'static str,
    color: Hsla,
}

fn color_combination_items() -> Vec<SelectorItem> {
    [
        ColorCombination::Monochromatic,
        ColorCombination::Complementary,
        ColorCombination::Analogous,
        ColorCombination::Triadic,
        ColorCombination::Tetradic,
        ColorCombination::Pentadic,
        ColorCombination::Hexadic,
    ]
    .into_iter()
    .map(|combination| SelectorItem::new(combination.id()).label(combination.label()))
    .collect()
}

fn monochromatic_palette(base: Hsla) -> Vec<CombinationSwatch> {
    vec![
        CombinationSwatch { role: "Base", color: base },
        CombinationSwatch {
            role: "Tint",
            color: hsla(base.h, (base.s * 0.78).clamp(0.0, 1.0), (base.l + 0.14).clamp(0.0, 1.0), base.a),
        },
    ]
}

fn complementary_palette(base: Hsla) -> Vec<CombinationSwatch> {
    vec![
        CombinationSwatch { role: "Base", color: base },
        CombinationSwatch { role: "Complement", color: rotate_hue(base, 180.0) },
    ]
}

fn analogous_palette(base: Hsla) -> Vec<CombinationSwatch> {
    vec![
        CombinationSwatch { role: "Left", color: rotate_hue(base, -30.0) },
        CombinationSwatch { role: "Base", color: base },
        CombinationSwatch { role: "Right", color: rotate_hue(base, 30.0) },
    ]
}

fn triadic_palette(base: Hsla) -> Vec<CombinationSwatch> {
    vec![
        CombinationSwatch { role: "Base", color: base },
        CombinationSwatch { role: "Triad B", color: rotate_hue(base, 120.0) },
        CombinationSwatch { role: "Triad C", color: rotate_hue(base, 240.0) },
    ]
}

fn tetradic_palette(base: Hsla) -> Vec<CombinationSwatch> {
    vec![
        CombinationSwatch { role: "Base", color: base },
        CombinationSwatch { role: "Tetrad B", color: rotate_hue(base, 90.0) },
        CombinationSwatch { role: "Tetrad C", color: rotate_hue(base, 180.0) },
        CombinationSwatch { role: "Tetrad D", color: rotate_hue(base, 270.0) },
    ]
}

fn pentadic_palette(base: Hsla) -> Vec<CombinationSwatch> {
    vec![
        CombinationSwatch { role: "Base", color: base },
        CombinationSwatch { role: "Pentad B", color: rotate_hue(base, 72.0) },
        CombinationSwatch { role: "Pentad C", color: rotate_hue(base, 144.0) },
        CombinationSwatch { role: "Pentad D", color: rotate_hue(base, 216.0) },
        CombinationSwatch { role: "Pentad E", color: rotate_hue(base, 288.0) },
    ]
}

fn hexadic_palette(base: Hsla) -> Vec<CombinationSwatch> {
    vec![
        CombinationSwatch { role: "Base", color: base },
        CombinationSwatch { role: "Hexad B", color: rotate_hue(base, 60.0) },
        CombinationSwatch { role: "Hexad C", color: rotate_hue(base, 120.0) },
        CombinationSwatch { role: "Hexad D", color: rotate_hue(base, 180.0) },
        CombinationSwatch { role: "Hexad E", color: rotate_hue(base, 240.0) },
        CombinationSwatch { role: "Hexad F", color: rotate_hue(base, 300.0) },
    ]
}

fn rotate_hue(base: Hsla, delta_degrees: f32) -> Hsla {
    hsla((base.h + delta_degrees / 360.0).rem_euclid(1.0), base.s, base.l, base.a)
}

fn harmony_points_from_swatches(swatches: &[CombinationSwatch]) -> Vec<CombinationSwatch> {
    swatches.iter().copied().filter(|swatch| swatch.role != "Base").collect()
}

fn render_combo_wheel_layer(
    wheel: Entity<ColorFieldState>,
    harmony_points: Vec<CombinationSwatch>,
    metrics: ColorHarmoniesMetrics,
    border: Hsla,
) -> impl gpui::IntoElement {
    let wheel_size = metrics.wheel_size();
    let center = wheel_size * 0.5;
    let marker_size = metrics.wheel_thumb_size;
    let marker_half = marker_size * 0.5;
    let marker_radius_max = (center - marker_half).max(0.0);

    div()
        .size(px(wheel_size))
        .relative()
        .child(wheel)
        .children(harmony_points.into_iter().map(|swatch| {
            let angle = (swatch.color.h * 360.0).rem_euclid(360.0).to_radians();
            let marker_radius = marker_radius_max * swatch.color.s.clamp(0.0, 1.0);
            let left = center + marker_radius * angle.cos() - marker_half;
            let top = center - marker_radius * angle.sin() - marker_half;

            div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .size(px(marker_size))
                .rounded_full()
                .border_1()
                .border_color(border)
                .bg(transparent_black())
                .into_any_element()
        }))
}

fn render_combinations_body(
    look: &Look,
    title_text: Hsla,
    border: Hsla,
    label_text_size: TextSize,
    lightness_ring: Entity<SliderControl>,
    wheel: Entity<ColorFieldState>,
    color_input: TextField,
    harmony_menu: Entity<Selector>,
    metrics: ColorHarmoniesMetrics,
    color: Hsla,
    harmony_points: Vec<CombinationSwatch>,
    swatches: Vec<CombinationSwatch>,
) -> AnyElement {
    vstack! {
        gap=COMPONENT_GAP_PX align=start;
        div()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .size(px(metrics.ring_size + metrics.ring_canvas_padding))
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(lightness_ring)
                    .child(
                        div()
                            .absolute()
                            .left_1_2()
                            .top_1_2()
                            .ml(px(-metrics.wheel_half()))
                            .mt(px(-metrics.wheel_half()))
                            .size(px(metrics.wheel_size()))
                            .rounded_full()
                            .overflow_hidden()
                            .child(render_combo_wheel_layer(wheel, harmony_points, metrics, border)),
                    ),
            ),
        render_color_field_row(look, title_text, label_text_size, border, color, color_input),
        render_combination_field_row(look, title_text, label_text_size, harmony_menu),
        render_palette_swatches(border, metrics, swatches),
    }
    .w_full()
    .min_w(px(0.0))
    .into_any_element()
}

fn render_palette_swatches(
    border: Hsla,
    metrics: ColorHarmoniesMetrics,
    swatches: Vec<CombinationSwatch>,
) -> AnyElement {
    let swatch_size = metrics.palette_swatch_diameter(swatches.len());

    div()
        .w_full()
        .h(px(COLOR_SWATCH_SIZE))
        .flex()
        .items_center()
        .gap(px(ROW_GAP_PX))
        .children(swatches.into_iter().map(|swatch| {
            div()
                .flex_shrink_0()
                .size(px(swatch_size))
                .rounded_full()
                .border_1()
                .border_color(border)
                .bg(swatch.color)
                .into_any_element()
        }))
        .into_any_element()
}

fn render_color_field_row(
    look: &Look,
    title_text: Hsla,
    label_text_size: TextSize,
    border: Hsla,
    color: Hsla,
    color_input: TextField,
) -> AnyElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(ROW_GAP_PX))
        .child(field_label(look, "Color", title_text, label_text_size))
        .child(
            div()
                .flex_shrink_0()
                .size(px(COLOR_SWATCH_SIZE))
                .rounded_full()
                .border_1()
                .border_color(border)
                .bg(color),
        )
        .child(div().w(px(MIN_COLOR_INPUT_WIDTH)).child(color_input))
        .into_any_element()
}

fn render_combination_field_row(
    look: &Look,
    title_text: Hsla,
    label_text_size: TextSize,
    harmony_menu: Entity<Selector>,
) -> AnyElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(ROW_GAP_PX))
        .child(field_label(look, "Combination", title_text, label_text_size))
        .child(div().w(px(MIN_SELECTOR_WIDTH)).child(harmony_menu))
        .into_any_element()
}

fn field_label(look: &Look, label: &'static str, color: Hsla, text_size: TextSize) -> gpui::Div {
    div()
        .flex_shrink_0()
        .typography_style(look.typography_scale(text_size))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .whitespace_nowrap()
        .child(label)
}

fn parse_color_input(value: &str) -> Option<Hsla> {
    parse_hex(value).or_else(|| parse_hsl(value))
}

fn parse_hex(value: &str) -> Option<Hsla> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if hex.len() != 6 {
        return None;
    }

    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some(rgb(((red as u32) << 16) | ((green as u32) << 8) | (blue as u32)).into())
}

fn parse_hsl(value: &str) -> Option<Hsla> {
    let trimmed = value.trim();
    let inner = trimmed.strip_prefix("hsl(").and_then(|s| s.strip_suffix(')')).unwrap_or(trimmed);

    let normalized = inner.replace(',', " ");
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }

    let hue = parts[0].parse::<f32>().ok()?.rem_euclid(360.0) / 360.0;
    let saturation = parts[1].strip_suffix('%')?.parse::<f32>().ok()?.clamp(0.0, 100.0) / 100.0;
    let lightness = parts[2].strip_suffix('%')?.parse::<f32>().ok()?.clamp(0.0, 100.0) / 100.0;

    Some(hsla(hue, saturation, lightness, 1.0))
}
