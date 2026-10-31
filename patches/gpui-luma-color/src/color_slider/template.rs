use std::sync::{Arc, OnceLock, RwLock};

use gpui::{App, Corners, CornersRefinement, Div, Pixels, Stateful, Window, canvas, div, px, relative, prelude::*};

use crate::style::StyledExt;
use gpui_luma::controls::slider::{
    SliderInteractionHandlers, SliderOrientation, SliderRenderModel, SliderTemplate, SliderTemplateHandlers,
    SliderThumbValue, ThumbId, TrackSegment, TrackSegmentKind, attach_linear_interaction, attach_thumb_drag,
    display_position, render_domain_track_layer, segment_corner_radii, segment_display_span,
};
use gpui_luma::theme::ControlSize;

use super::color_thumb::{ColorThumb, ThumbAxis, ThumbShape};
use super::radius::{inner_track_corner_radius, inset_corner_radii, resolve_track_corner_radii, TRACK_BORDER_WIDTH};
use super::types::{Axis, ThumbConfig, ThumbPosition, ThumbSize, sizing};
use super::visual::default_color_slider_visual;

#[derive(Clone, Debug)]
pub struct ColorSliderTemplateConfig {
    pub thumb: ThumbConfig,
    pub corner_radii: CornersRefinement<gpui::AbsoluteLength>,
    pub size: ControlSize,
}

impl Default for ColorSliderTemplateConfig {
    fn default() -> Self {
        Self { thumb: ThumbConfig::default(), corner_radii: CornersRefinement::default(), size: ControlSize::Md }
    }
}

#[derive(Clone)]
pub struct ColorSliderTemplate {
    config: Arc<RwLock<ColorSliderTemplateConfig>>,
}

impl ColorSliderTemplate {
    pub fn new(config: Arc<RwLock<ColorSliderTemplateConfig>>) -> Self {
        Self { config }
    }

    pub fn shared_config(&self) -> Arc<RwLock<ColorSliderTemplateConfig>> {
        Arc::clone(&self.config)
    }
}

pub fn default_color_slider_template() -> Arc<dyn SliderTemplate> {
    static TEMPLATE: OnceLock<Arc<dyn SliderTemplate>> = OnceLock::new();
    TEMPLATE
        .get_or_init(|| Arc::new(ColorSliderTemplate::new(Arc::new(RwLock::new(ColorSliderTemplateConfig::default())))))
        .clone()
}

pub fn color_slider_template(config: Arc<RwLock<ColorSliderTemplateConfig>>) -> Arc<dyn SliderTemplate> {
    Arc::new(ColorSliderTemplate::new(config))
}

impl SliderTemplate for ColorSliderTemplate {
    fn render(
        &self,
        model: &SliderRenderModel<'_>,
        handlers: SliderTemplateHandlers,
        primary_thumb_id: ThumbId,
        window: &mut Window,
        _cx: &mut App,
    ) -> Stateful<Div> {
        let config = self.config.read().expect("color slider template config").clone();
        let is_vertical = model.orientation == SliderOrientation::Vertical;
        let _axis = if is_vertical { Axis::Vertical } else { Axis::Horizontal };
        let visual = default_color_slider_visual(model.enabled);
        let track_thickness = track_thickness_for_size(config.size);
        let thumb_size = thumb_size_for_config(&config.thumb);
        let track_inset = track_inset(&config.thumb, thumb_size);
        let track_hitsize = track_thickness.max(thumb_size);
        let track_radius =
            resolve_track_corner_radii(&config.corner_radii, window.rem_size(), px(track_thickness / 2.0));
        let inner_track_radii = inset_corner_radii(track_radius, TRACK_BORDER_WIDTH);
        let inner_track_radius = inner_track_corner_radius(track_radius.top_left);

        let SliderTemplateHandlers {
            track_bounds,
            hover,
            mouse_down,
            mouse_move,
            mouse_up,
            mouse_up_out,
            drag_move,
            thumb_mouse_down,
        } = handlers;
        let interaction = SliderInteractionHandlers {
            hover,
            mouse_down,
            mouse_move,
            mouse_up,
            mouse_up_out,
            drag_move: drag_move.clone(),
        };

        let mut track = div()
            .id(format!("{}-track", model.id))
            .absolute()
            .border_1()
            .border_color(visual.border)
            .corner_radii(track_radius)
            .overflow_hidden()
            .when(is_vertical, |this| {
                this.w(px(track_thickness))
                    .left(px((track_hitsize - track_thickness) / 2.0))
                    .top(px(track_inset))
                    .bottom(px(track_inset))
            })
            .when(!is_vertical, |this| {
                this.h(px(track_thickness))
                    .top(px((track_hitsize - track_thickness) / 2.0))
                    .left(px(track_inset))
                    .right(px(track_inset))
            });

        if let Some(layer) = render_domain_track_layer(model, inner_track_radius) {
            track = track.child(layer);
        }

        if model.track_segments.iter().any(|segment| segment.kind == TrackSegmentKind::Blocked) {
            track = track.children(
                model.track_segments.iter().filter(|segment| segment.kind == TrackSegmentKind::Blocked).map(
                    |segment| {
                        render_blocked_segment(
                            segment,
                            model.orientation,
                            model.reversed,
                            inner_track_radius,
                            visual.blocked_overlay,
                        )
                    },
                ),
            );
        }

        let thumb_axis = if is_vertical {
            ThumbAxis::Vertical
        } else {
            ThumbAxis::Horizontal
        };
        let thumb_cross_offset = px((track_hitsize - thumb_size) / 2.0);
        let render_thumb = |thumb: &SliderThumbValue, id_suffix: &str| {
            let display_percentage = display_position(thumb.position.clamp(0.0, 1.0), model.reversed);
            let thumb_main_adjust = px(thumb_main_axis_size(&config.thumb, thumb_size) * display_percentage);
            let fill_color = thumb_fill_color(&config.thumb, thumb_size, track_thickness, thumb);
            let mut node = div()
                .id(format!("{}-thumb{}", model.id, id_suffix))
                .absolute()
                .when(is_vertical, |this| {
                    this.left(thumb_cross_offset).top(relative(display_percentage)).mt(-thumb_main_adjust)
                })
                .when(!is_vertical, |this| {
                    this.top(thumb_cross_offset).left(relative(display_percentage)).ml(-thumb_main_adjust)
                })
                .child(
                    ColorThumb::new(px(thumb_size))
                        .shape(config.thumb.shape)
                        .axis(thumb_axis)
                        .active(model.active_thumb_id == Some(thumb.id))
                        .when_some(fill_color, |this, color| this.color(color)),
                );
            if model.enabled {
                node = attach_thumb_drag(
                    node,
                    model.id,
                    thumb.id,
                    thumb_mouse_down.clone(),
                    drag_move.clone(),
                    model.enabled,
                );
            }
            node
        };

        let mut root = div()
            .id(model.id.clone())
            .relative()
            .when(is_vertical, |this| this.w(px(track_hitsize)).h_full())
            .when(!is_vertical, |this| this.h(px(track_hitsize)).w_full())
            .child(track.child(canvas(move |bounds, window, cx| track_bounds(&bounds, window, cx), |_, _, _, _| {})))
            .when(!model.enabled, |this| {
                this.child(disabled_overlay(
                    is_vertical,
                    track_thickness,
                    track_hitsize,
                    track_inset,
                    inner_track_radii,
                    visual.disabled_overlay,
                ))
            });

        if model.enabled {
            if model.thumb_policy.is_multi_thumb() {
                root = root.children(
                    ordered_thumbs(model)
                        .into_iter()
                        .map(|thumb| render_thumb(thumb, &format!("-{}", thumb.id.as_u64()))),
                );
            } else {
                let primary_thumb =
                    model.thumbs.iter().find(|thumb| thumb.id == primary_thumb_id).or_else(|| model.thumbs.first());
                if let Some(thumb) = primary_thumb {
                    root = root.child(render_thumb(thumb, ""));
                }
            }
        }

        root = attach_linear_interaction(root, model, interaction);
        root
    }
}

/// Inactive thumbs first; active thumb last so it paints on top while dragging.
fn ordered_thumbs<'a>(model: &'a SliderRenderModel<'_>) -> Vec<&'a SliderThumbValue> {
    let mut thumbs: Vec<_> = model.thumbs.iter().collect();
    thumbs.sort_by_key(|thumb| model.active_thumb_id == Some(thumb.id));
    thumbs
}

fn render_blocked_segment(
    segment: &TrackSegment,
    orientation: SliderOrientation,
    reversed: bool,
    track_radius: Pixels,
    color: gpui::Hsla,
) -> Div {
    let (display_start, display_span) = segment_display_span(segment, reversed);
    let corner_radii = segment_corner_radii(display_start, display_span, orientation, track_radius);

    match orientation {
        SliderOrientation::Horizontal => div()
            .absolute()
            .left(relative(display_start))
            .top(px(0.0))
            .h_full()
            .w(relative(display_span))
            .bg(color)
            .corner_radii(corner_radii),
        SliderOrientation::Vertical => div()
            .absolute()
            .left(px(0.0))
            .bottom(relative(display_start))
            .w_full()
            .h(relative(display_span))
            .bg(color)
            .corner_radii(corner_radii),
    }
}

fn track_thickness_for_size(size: ControlSize) -> f32 {
    match size {
        ControlSize::Sm => sizing::TRACK_THICKNESS_SMALL,
        ControlSize::Md => sizing::TRACK_THICKNESS_MEDIUM,
        ControlSize::Lg => sizing::TRACK_THICKNESS_LARGE,
    }
}

fn thumb_size_for_config(thumb: &ThumbConfig) -> f32 {
    match thumb.size {
        ThumbSize::XSmall => sizing::THUMB_SIZE_XSMALL,
        ThumbSize::Small => sizing::THUMB_SIZE_SMALL,
        ThumbSize::Medium => sizing::THUMB_SIZE_MEDIUM,
        ThumbSize::Large => sizing::THUMB_SIZE_LARGE,
    }
}

fn effective_thumb_position(thumb: &ThumbConfig) -> ThumbPosition {
    let hint = thumb.shape.layout_hint();
    if hint.supported_positions.contains(&thumb.position) {
        thumb.position
    } else {
        hint.preferred_position
    }
}

fn track_inset(thumb: &ThumbConfig, thumb_size: f32) -> f32 {
    match effective_thumb_position(thumb) {
        ThumbPosition::InsideSlider => 0.0,
        ThumbPosition::EdgeToEdge => thumb_main_axis_size(thumb, thumb_size) / 2.0,
    }
}

fn thumb_main_axis_size(thumb: &ThumbConfig, thumb_size: f32) -> f32 {
    match thumb.shape {
        ThumbShape::Bar => super::color_thumb::bar_main_axis_size(px(thumb_size)).as_f32(),
        ThumbShape::Circle | ThumbShape::Square => thumb_size,
    }
}

fn thumb_fill_color(
    thumb: &ThumbConfig,
    thumb_size: f32,
    track_thickness: f32,
    slider_thumb: &SliderThumbValue,
) -> Option<gpui::Hsla> {
    if thumb.shape == ThumbShape::Bar {
        return slider_thumb.preview;
    }

    let thumb_extends_beyond_track = thumb_size > track_thickness;
    match effective_thumb_position(thumb) {
        ThumbPosition::InsideSlider if !thumb_extends_beyond_track => None,
        _ => slider_thumb.preview,
    }
}

fn disabled_overlay(
    is_vertical: bool,
    track_thickness: f32,
    track_hitsize: f32,
    track_inset: f32,
    radius: Corners<Pixels>,
    color: gpui::Hsla,
) -> Div {
    div()
        .absolute()
        .when(is_vertical, |this| {
            this.w(px(track_thickness))
                .left(px((track_hitsize - track_thickness) / 2.0))
                .top(px(track_inset))
                .bottom(px(track_inset))
        })
        .when(!is_vertical, |this| {
            this.h(px(track_thickness))
                .top(px((track_hitsize - track_thickness) / 2.0))
                .left(px(track_inset))
                .right(px(track_inset))
        })
        .corner_radii(radius)
        .bg(color)
}
