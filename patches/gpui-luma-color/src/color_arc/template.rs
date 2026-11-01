use std::sync::{Arc, OnceLock, RwLock};

use gpui::{App, Div, Stateful, Window, div, img, px, prelude::*};

use crate::color_slider::color_thumb::{ColorThumb, ThumbShape};
use gpui_luma::controls::slider::{
    SliderRenderModel, SliderTemplate, SliderTemplateHandlers, SliderThumbValue, ThumbId, attach_radial_interaction,
    display_position, render_domain_track_layer, track_bounds_canvas,
};

use super::common::{position_to_turn, turn_to_theta};
use super::track_context::ColorArcTrackContext;
use super::visual::default_color_arc_visual;

#[derive(Clone, Debug, Default)]
pub struct ColorArcTemplateConfig {
    pub context: ColorArcTrackContext,
    pub thumb_size: Option<f32>,
}

#[derive(Clone)]
pub struct ColorArcTemplate {
    config: Arc<RwLock<ColorArcTemplateConfig>>,
}

impl ColorArcTemplate {
    pub fn new(config: Arc<RwLock<ColorArcTemplateConfig>>) -> Self {
        Self { config }
    }

    pub fn shared_config(&self) -> Arc<RwLock<ColorArcTemplateConfig>> {
        Arc::clone(&self.config)
    }
}

pub fn default_color_arc_template() -> Arc<dyn SliderTemplate> {
    static TEMPLATE: OnceLock<Arc<dyn SliderTemplate>> = OnceLock::new();
    TEMPLATE
        .get_or_init(|| Arc::new(ColorArcTemplate::new(Arc::new(RwLock::new(ColorArcTemplateConfig::default())))))
        .clone()
}

pub fn color_arc_template(config: Arc<RwLock<ColorArcTemplateConfig>>) -> Arc<dyn SliderTemplate> {
    Arc::new(ColorArcTemplate::new(config))
}

impl SliderTemplate for ColorArcTemplate {
    fn render(
        &self,
        model: &SliderRenderModel<'_>,
        handlers: SliderTemplateHandlers,
        primary_thumb_id: ThumbId,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Stateful<Div> {
        let config = self.config.read().expect("color arc template config").clone();
        let context = config.context;
        let dial_size = context.dial_size_px();
        let arc_thickness = context.arc_thickness_px();
        let thumb_size = context.thumb_size_px(config.thumb_size);
        let visual = default_color_arc_visual(model.enabled);

        let primary_thumb =
            model.thumbs.iter().find(|thumb| thumb.id == primary_thumb_id).or_else(|| model.thumbs.first());
        let thumb_position = primary_thumb.map(|thumb| thumb.position).unwrap_or(0.0);
        let display_percentage = display_position(thumb_position, model.reversed);

        let center = dial_size * 0.5;
        let outer_radius = dial_size * 0.5;
        let track_radius = (outer_radius - arc_thickness * 0.5).max(0.0);
        let thumb_turn = position_to_turn(display_percentage, context.start_turns(), context.sweep_turns());
        let theta = turn_to_theta(thumb_turn);
        let thumb_x = center + track_radius * theta.cos();
        let thumb_y = center + track_radius * theta.sin();

        let (track_bounds, interaction) = handlers.into();

        let mut root = div()
            .id(model.id.clone())
            .relative()
            .size(px(dial_size))
            .child({
                let mut track = div().absolute().size_full();
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
            .when(model.enabled, |this| {
                this.child(render_color_arc_thumb(
                    format!("{}-thumb", model.id),
                    thumb_x,
                    thumb_y,
                    thumb_size,
                    primary_thumb,
                ))
            })
            .when(!model.enabled, |this| this.child(div().absolute().inset_0().bg(visual.disabled_overlay)))
            .child(track_bounds_canvas(track_bounds));

        root = attach_radial_interaction(root, model, interaction, primary_thumb_id);
        root
    }
}

fn render_color_arc_thumb(
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
