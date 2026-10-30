use super::*;
use super::view::{INSPECTOR_WIDTH, compact_slider_row, render_mesh_background_row, render_mesh_point_row, render_stop_row};

use crate::theme::{ColorVizLookExt, TypographyExt};
use gpui::{IntoElement, MouseButton, Window, div, prelude::*};
use gpui_luma::{form_field};
use super::mesh::render_info_row;
use gpui_luma::infra::StyledExt;
use gpui_luma::controls::accordion::{
    AccordionBuilder, AccordionContent, AccordionEvent, AccordionItem, AccordionTrigger, AccordionTheme,
    AccordionPalette, AccordionContentPalette, AccordionScale, ThemedAccordionTemplate,
};
use gpui_luma::theme::{InteractionState, MetricTokens};
use std::sync::atomic::{AtomicBool, Ordering};

const SECTION_SPACING: f32 = 8.0;

struct SectionTheme(Arc<Look>);
impl AccordionTheme for SectionTheme {
    fn resolve_trigger(&self, _: InteractionState, _: ControlSize) -> AccordionPalette {
        let chrome = self.0.chrome();
        AccordionPalette {
            background: None,
            foreground: chrome.muted_text,
            border_color: chrome.border,
            icon_color: chrome.muted_text,
            chevron_color: chrome.muted_text,
            typography: self.0.typography_scale(TextSize::Base),
            font_family: "System UI".into(),
        }
    }
    fn resolve_content(&self, _: bool) -> AccordionContentPalette {
        AccordionContentPalette { background: None, foreground: self.0.chrome().body_text }
    }
    fn metrics(&self) -> MetricTokens {
        self.0.metrics()
    }
    fn resolve_scale(&self, _: ControlSize, _: f32) -> AccordionScale {
        AccordionScale {
            drag_handle_padding: 0.0,
            disabled_opacity: self.0.metrics().disabled_opacity,
            trigger_height: 14.0,
            padding_x: 0.0,
            padding_y: SECTION_SPACING,
            content_padding_y: SECTION_SPACING,
            radius: 0.0,
            item_gap: 0.0,
            inner_gap: SECTION_SPACING,
            icon_size: 12.0,
            chevron_size: 0.0,
        }
    }
}

impl GradientBuilder {
    pub(super) fn init_section_accordions(&mut self, cx: &mut Context<Self>) {
        for mode in ["gradients", "mesh", "freeform"] {
            let labels = if mode == "gradients" {
                &["CANVAS", "FIELDS", "COLOR", "RENDER"][..]
            } else {
                &["CANVAS", "FIELDS", "COLOR"][..]
            };
            let sections = labels
                .iter()
                .enumerate()
                .map(|(index, &label)| {
                    let owner = cx.entity().downgrade();
                    let look = self.look.clone();
                    let expanded = Arc::new(AtomicBool::new(true));
                    let header_expanded = expanded.clone();
                    let id = format!("color-viz-{mode}-section-{label}");
                    let selector = id.clone();
                    let display_label = match label {
                        "CANVAS" => "Canvas",
                        "FIELDS" => "Fields",
                        "COLOR" => "Color",
                        _ => "Render",
                    };
                    let trigger = AccordionTrigger::custom(move |_, _| {
                        let chrome = look.chrome();
                        let label_color = gpui::Hsla { a: chrome.muted_text.a * 0.75, ..chrome.muted_text };
                        div()
                            .debug_selector({
                                let selector = selector.clone();
                                move || selector.clone()
                            })
                            .flex()
                            .items_center()
                            .gap_2()
                            .typography_style(look.typography_scale(TextSize::Base))
                            .font_weight(gpui::FontWeight::NORMAL)
                            .text_color(label_color)
                            .child(
                                gpui::svg()
                                    .path(
                                        if header_expanded.load(Ordering::Relaxed) {
                                            LucideIcon::ChevronDown
                                        } else {
                                            LucideIcon::ChevronRight
                                        }
                                        .asset_path(),
                                    )
                                    .size(px(12.0))
                                    .text_color(chrome.muted_text),
                            )
                            .child(display_label)
                            .into_any_element()
                    });
                    let content = AccordionContent::custom(move |window, cx| {
                        owner
                            .update(cx, |owner, cx| owner.render_accordion_section(index, window, cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    });
                    let accordion = AccordionBuilder::new(id)
                        .item(AccordionItem::new("section", trigger, content).expanded(true))
                        .collapsible(true)
                        .motion_enabled(false)
                        .item_dividers(false)
                        .trigger_min_height(14.0)
                        .trigger_padding_y(SECTION_SPACING)
                        .content_padding_y(SECTION_SPACING)
                        .template(Arc::new(ThemedAccordionTemplate::new(Arc::new(SectionTheme(self.look.clone())))))
                        .spawn(cx);
                    self._subscriptions.push(cx.subscribe(&accordion, move |_, _, event: &AccordionEvent, cx| {
                        if let AccordionEvent::ExpandedChanged { expanded: value, .. } = event {
                            expanded.store(*value, Ordering::Relaxed);
                            cx.notify();
                        }
                    }));
                    accordion
                })
                .collect();
            self.section_accordions.push(sections);
        }
    }

    pub(super) fn render_inspector(&self, id: &'static str, background: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
        let mode = match self.selected_tab {
            BuilderTab::Gradients => 0,
            BuilderTab::Mesh => 1,
            BuilderTab::Freeform => 2,
        };
        div()
            .id(id)
            .w(px(INSPECTOR_WIDTH))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(SECTION_SPACING))
            .corner_radii(super::mesh::controls_panel_corner_radii())
            .overflow_y_scroll()
            .px(px(12.0))
            .pb(px(12.0))
            .bg(background)
            .children(
                self.section_accordions[mode]
                    .iter()
                    .map(|accordion| div().w_full().flex_shrink_0().child(accordion.clone())),
            )
            .when(self.selected_tab != BuilderTab::Gradients, |panel| {
                panel.child(div().text_size(px(10.0)).text_color(self.look.chrome().muted_text).child(
                    if self.selected_tab == BuilderTab::Freeform {
                        "Click canvas to deselect · Right-click to hide points".to_string()
                    } else {
                        format!(
                            "{} · {} · Right-click to hide guides",
                            self.mesh_grid_preset.label(),
                            self.mesh_aspect_ratio_preset.label()
                        )
                    },
                ))
            })
    }
}

impl GradientBuilder {
    pub(super) fn render_accordion_section(
        &mut self,
        section: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let chrome = self.look.chrome();
        let info_label_style = self.look.typography_scale(TextSize::Sm);
        let info_value_style = info_label_style;
        let freeform = self.selected_tab == BuilderTab::Freeform;
        match section {
            0 if self.selected_tab == BuilderTab::Gradients => {
                let type_selector = self.type_selector.clone();
                let renderer_selector = self.renderer_selector.clone();
                let rotation_slider = self.rotation_slider.clone();
                let rotation_deg = self.rotation_deg;
                let stop_editor = div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .bg(chrome.panel_background)
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().flex_1().min_w_0().child(form_field!("Type", chrome; type_selector)))
                            .child(div().flex_1().min_w_0().child(form_field!("Renderer", chrome; renderer_selector))),
                    )
                    .child(self.p3_switch.clone())
                    .when(self.preview_p3, |this| {
                        this.child(div().text_size(px(11.0)).child(if super::p3::available(window) {
                            "Display P3 · native shader"
                        } else {
                            "Display P3 unavailable · sRGB fallback"
                        }))
                    })
                    .child(form_field!("Aspect", chrome; self.mesh_aspect_ratio_selector.clone()))
                    .when(self.gradient_type != GradientType::Radial, |this| {
                        this.child(compact_slider_row(
                            "Angle",
                            rotation_slider,
                            format!("{rotation_deg:.0}°"),
                            info_label_style,
                            chrome,
                        ))
                        .child(self.animate_angle_button.clone())
                        .child(compact_slider_row(
                            "Speed",
                            self.angle_speed_slider.clone(),
                            format!("{:.0}°/s", self.angle_speed),
                            info_label_style,
                            chrome,
                        ))
                    });

                stop_editor.into_any_element()
            }
            0 => {
                let mesh_reset_button = self.mesh_reset_button.clone();
                let blend_selector = self.blend_selector.clone();
                let mesh_grid_selector = self.mesh_grid_selector.clone();
                let mesh_aspect_ratio_selector = self.mesh_aspect_ratio_selector.clone();
                let mesh_background = self.mesh_background;
                let builder = cx.entity();
                let panel = div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .bg(chrome.panel_background)
                    .child(div().flex().items_center().justify_between().child(mesh_reset_button))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().flex_1().min_w_0().child(if freeform {
                                form_field!("Blend", chrome; blend_selector).into_any_element()
                            } else {
                                form_field!("Grid", chrome; mesh_grid_selector).into_any_element()
                            }))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(form_field!("Aspect", chrome; mesh_aspect_ratio_selector)),
                            ),
                    )
                    .when(!freeform, |panel| panel
                        .child(div().flex().items_center().gap_2()
                            .child(self.mesh_cycle_button.clone())
                            .child(div().text_size(px(11.)).child(format!("{} · {}/5",
                                super::mesh::MESH_PALETTE_NAMES[self.mesh_palette], self.mesh_palette + 1))))
                        .child(self.mesh_palette_animate_button.clone())
                        .child(compact_slider_row(
                            "Speed", self.mesh_palette_speed_slider.clone(),
                            format!("{:.1}/s", self.mesh_palette_speed), info_label_style, chrome,
                        ))
                        .child(form_field!("Renderer", chrome; self.mesh_renderer_selector.clone()))
                        .child(self.mesh_p3_switch.clone())
                        .when(self.mesh_p3, |panel| panel.child(div().text_size(px(10.)).child(
                            if cfg!(target_os = "macos") && super::p3::available(window) {
                                "Display P3 · native mesh shader"
                            } else { "Display P3 unavailable · sRGB fallback" }
                        )))
                        .when(self.mesh_shader_enabled, |panel| panel.child(div().text_size(px(10.)).child(
                            "Native Hermite mesh · opaque colors · interpolation differs from CPU Coons"
                        ))))
                    .child(render_mesh_background_row(mesh_background, info_label_style, chrome, builder.clone()));
                panel.into_any_element()
            }
            1 if self.selected_tab == BuilderTab::Gradients => {
                let stop_rows = self.ordered_stop_rows(cx);
                let stop_content = stop_rows
                    .into_iter()
                    .enumerate()
                    .map(|(index, (thumb_id, position, color))| {
                        (
                            thumb_id.as_u64(),
                            render_stop_row(
                                index + 1,
                                thumb_id,
                                position,
                                color,
                                self.selected_stop == Some(thumb_id),
                                self.stop_delete_buttons.get(&thumb_id).cloned(),
                                self.look.typography_scale(TextSize::Sm),
                                self.look.typography_scale(TextSize::Sm),
                                chrome,
                                cx.entity(),
                            )
                            .into_any_element(),
                        )
                    })
                    .collect();
                let stop_list = self.stop_list.render(
                    stop_content,
                    self.selected_stop.map(|id| id.as_u64()),
                    "color-viz-stop-list",
                    chrome,
                    window,
                    cx,
                    GradientBuilder::handle_stop_list_input,
                );

                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_start()
                            .justify_between()
                            .gap_4()
                            .child(div().flex_1().min_w(px(0.0)).pt_1().child(self.gradient_stops.clone()))
                            .child(self.add_stop_button.clone()),
                    )
                    .child(stop_list)
                    .into_any_element()
            }
            1 => {
                let point_content = self
                    .mesh_points
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, point)| {
                        let selected = self.selected_mesh_point == Some(index);
                        let row = div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(render_mesh_point_row(
                                if self.selected_tab == BuilderTab::Freeform {
                                    self.point_buttons.get(index).cloned()
                                } else {
                                    None
                                },
                                index,
                                point,
                                selected,
                                info_label_style,
                                chrome,
                                cx.entity(),
                            ))
                            .when(self.selected_tab == BuilderTab::Freeform && selected, |row| {
                                row.child(
                                    div()
                                        .px_2()
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                        .child(compact_slider_row(
                                            "Point spread",
                                            self.point_spread_slider.clone(),
                                            format!("{:.0}%", self.point_spreads.get(index).copied().unwrap_or(100.0)),
                                            info_label_style,
                                            chrome,
                                        )),
                                )
                            });
                        (self.mesh_point_ids[index], row.into_any_element())
                    })
                    .collect();
                let point_list = self.point_list.render(
                    point_content,
                    self.selected_mesh_point.map(|index| self.mesh_point_ids[index]),
                    "color-viz-point-list",
                    chrome,
                    window,
                    cx,
                    GradientBuilder::handle_point_list_input,
                );
                let freeform_buttons = self.freeform_buttons.clone();
                let spread_slider = self.spread_slider.clone();
                let spread_percent = self.spread_percent;
                let softness_slider = self.softness_slider.clone();
                let softness_percent = self.softness_percent;
                let fields =
                    div().w_full().flex().flex_col().gap_2().bg(chrome.panel_background).when(freeform, |this| {
                        this.child(div().flex().items_center().gap_2().children(freeform_buttons))
                            .child(compact_slider_row(
                                "Spread · All",
                                spread_slider,
                                format!("{spread_percent:.0}%"),
                                info_label_style,
                                chrome,
                            ))
                            .child(compact_slider_row(
                                "Softness · All",
                                softness_slider,
                                format!("{softness_percent:.0}%"),
                                info_label_style,
                                chrome,
                            ))
                    });
                div().w_full().flex().flex_col().gap_2().child(fields).child(point_list).into_any_element()
            }
            2 => self.render_image_adjustments().into_any_element(),
            _ => {
                let gradient_spec = self.gradient_spec(cx);
                let stops = self.preview_stops(cx);
                let diagnostics_panel = div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .bg(chrome.panel_background)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(10.0))
                                    .text_color(chrome.muted_text)
                                    .child(gradient_spec),
                            )
                            .child(self.copy_css_button.clone()),
                    )
                    .child(render_info_row(
                        "Active",
                        self.active_preview_strategy,
                        info_label_style,
                        info_value_style,
                        chrome,
                    ))
                    .child(render_info_row(
                        "Requested",
                        match self.preview_renderer {
                            PreviewRenderer::Quads => "quads",
                            PreviewRenderer::RenderImageSync => "render (sync)",
                            PreviewRenderer::RenderImageAsync => "render (async)",
                            PreviewRenderer::ShaderPaint => "shader paint",
                            PreviewRenderer::ShaderBrush => "shader brush",
                        },
                        info_label_style,
                        info_value_style,
                        chrome,
                    ))
                    .child(render_info_row(
                        "Type",
                        self.gradient_type.label(),
                        info_label_style,
                        info_value_style,
                        chrome,
                    ))
                    .child(render_info_row(
                        "Stops",
                        format!("{}", stops.len()),
                        info_label_style,
                        info_value_style,
                        chrome,
                    ))
                    .child(render_info_row(
                        "Preview",
                        format!(
                            "{} x {}",
                            self.preview_size.width.as_f32().round() as i32,
                            self.preview_size.height.as_f32().round() as i32
                        ),
                        info_label_style,
                        info_value_style,
                        chrome,
                    ))
                    .child(render_info_row(
                        "Raster",
                        self.last_render_ms.map(|ms| format!("{ms:.2} ms")).unwrap_or_else(|| "n/a".to_string()),
                        info_label_style,
                        info_value_style,
                        chrome,
                    ));
                diagnostics_panel.into_any_element()
            }
        }
    }
}
