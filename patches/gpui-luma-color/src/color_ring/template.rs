use std::sync::{Arc, OnceLock, RwLock};

use crate::style::active_color_control_theme;
use gpui::{App, Div, Stateful, Window, div, img, px, prelude::*};

use crate::color_slider::color_thumb::{ColorThumb, ThumbShape};
use gpui_luma::controls::slider::{
    SliderRenderModel, SliderTemplate, SliderTemplateHandlers, SliderThumbValue, ThumbId, attach_radial_interaction,
    display_position, render_domain_track_layer, track_bounds_canvas,
};

use super::track_context::ColorRingTrackContext;
use super::visual::default_color_ring_visual;

#[derive(Clone, Debug, Default)]
pub struct ColorRingTemplateConfig {
    pub context: ColorRingTrackContext,
}

#[derive(Clone)]
pub struct ColorRingTemplate {
    config: Arc<RwLock<ColorRingTemplateConfig>>,
}

impl ColorRingTemplate {
    pub fn new(config: Arc<RwLock<ColorRingTemplateConfig>>) -> Self {
        Self { config }
    }
}

pub fn default_color_ring_template() -> Arc<dyn SliderTemplate> {
    static TEMPLATE: OnceLock<Arc<dyn SliderTemplate>> = OnceLock::new();
    TEMPLATE
        .get_or_init(|| Arc::new(ColorRingTemplate::new(Arc::new(RwLock::new(ColorRingTemplateConfig::default())))))
        .clone()
}

pub fn color_ring_template(config: Arc<RwLock<ColorRingTemplateConfig>>) -> Arc<dyn SliderTemplate> {
    Arc::new(ColorRingTemplate::new(config))
}

impl SliderTemplate for ColorRingTemplate {
    fn render(
        &self,
        model: &SliderRenderModel<'_>,
        handlers: SliderTemplateHandlers,
        primary_thumb_id: ThumbId,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Stateful<Div> {
        let config = self.config.read().expect("color ring template config").clone();
        let context = config.context;
        let dial_size = context.size_px();
        let ring_thickness = context.ring_thickness_px();
        let thumb_size = context.thumb_size_px();
        let outer_radius = dial_size * 0.5;
        let track_radius = (outer_radius - ring_thickness * 0.5).max(0.0);
        let visual = default_color_ring_visual(model.enabled);
        let border_color = active_color_control_theme().border;

        let primary_thumb =
            model.thumbs.iter().find(|thumb| thumb.id == primary_thumb_id).or_else(|| model.thumbs.first());
        let thumb_position = primary_thumb.map(|thumb| thumb.position).unwrap_or(0.0);
        let display_percentage = display_position(thumb_position, model.reversed);
        let theta = (display_percentage - 0.25 + context.rotation_turns()) * std::f32::consts::TAU;
        let thumb_x = outer_radius + track_radius * theta.cos();
        let thumb_y = outer_radius + track_radius * theta.sin();

        let (track_bounds, interaction) = handlers.into();

        let mut root = div()
            .id(model.id.clone())
            .relative()
            .size(px(dial_size))
            .child({
                let mut track = div().absolute().size_full().rounded_full().overflow_hidden();
                if let Some(renderer) = model.domain_track.as_ref()
                    && let Some(image) = renderer.raster_image(
                        gpui::Bounds {
                            origin: gpui::point(px(0.0), px(0.0)),
                            size: gpui::size(px(dial_size), px(dial_size)),
                        },
                        model.orientation,
                        model.reversed,
                    )
                {
                    track = track.child(img(image).absolute().size_full());
                } else if let Some(layer) = render_domain_track_layer(model, px(outer_radius)) {
                    track = track.child(layer);
                }
                track
            })
            .when(context.ring_outer_border, |this| {
                this.child(div().absolute().inset_0().rounded_full().border_1().border_color(border_color))
            })
            .when(context.ring_inner_border, |this| {
                this.child(
                    div().absolute().inset(px(ring_thickness)).rounded_full().border_1().border_color(border_color),
                )
            })
            .when(model.enabled, |this| {
                this.child(render_color_ring_thumb(
                    format!("{}-thumb", model.id),
                    thumb_x,
                    thumb_y,
                    thumb_size,
                    primary_thumb,
                ))
            })
            .when(!model.enabled, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded_full()
                        .bg(visual.disabled_overlay)
                        .child(div().absolute().inset(px(ring_thickness)).rounded_full().bg(visual.center_hole)),
                )
            })
            .child(track_bounds_canvas(track_bounds));

        root = attach_radial_interaction(root, model, interaction, primary_thumb_id);
        root
    }
}

fn render_color_ring_thumb(
    id: impl Into<gpui::ElementId>,
    center_x: f32,
    center_y: f32,
    thumb_size: f32,
    thumb: Option<&SliderThumbValue>,
) -> Stateful<Div> {
    let color = thumb.and_then(|thumb| thumb.preview).unwrap_or(gpui::hsla(0.0, 0.0, 0.0, 0.0));
    let half = thumb_size * 0.5;

    div()
        .id(id)
        .absolute()
        .left(px(center_x - half))
        .top(px(center_y - half))
        .child(ColorThumb::new(px(thumb_size)).shape(ThumbShape::Circle).active(false).color(color))
}
