use gpui_luma::infra::attachments::TooltipEntityExt;
use gpui_luma::controls::tooltip::Tooltip;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use gpui::{AppContext, Bounds, Context, Entity, Pixels, RenderImage, Size, Subscription, px, size};
use gpui_luma_color::color_slider::{ColorSliderBuilder, ColorSliderDomainRenderer, ColorSliderTrackContext};
use gpui_luma::controls::button::{Button, ButtonEvent};
use gpui_luma::prelude::HasPresenter;
use gpui_luma::controls::selector::{Selector, SelectorEvent};
use gpui_luma::controls::slider::{SliderControl, SliderEvent, SliderThumbPolicy, ThumbId};
use gpui_luma::controls::tabs::{Tabs, TabsEvent, TabsWidthMode};
use gpui_luma::theme::{ControlSize, ThemeMode};
use crate::theme::{Look, TextSize};
use gpui_luma_look_radix as radix;
use lucide_svg_static::Icon as LucideIcon;

mod actions;
mod mesh;
mod view;
mod freeform;
mod color_list;
mod sections;

use super::paint::{GradientType, MeshPoint, PreviewRenderer};
use super::sv_triangle_picker::SvTrianglePicker;
use mesh::{
    BuilderTab, MeshAspectRatioPreset, MeshColorTarget, MeshGridPreset, PreviewImageCacheKey, builder_tab_items,
    default_mesh_background, default_mesh_points, mesh_aspect_ratio_items, mesh_grid_items, renderer_items, type_items,
};

pub struct GradientBuilder {
    look: Arc<Look>,
    top_tabs: Entity<Tabs>,
    section_accordions: Vec<Vec<Entity<gpui_luma::controls::accordion::AccordionControl>>>,
    selected_tab: BuilderTab,
    gradient_stops: Entity<SliderControl>,
    stop_colors: HashMap<ThumbId, gpui::Hsla>,
    selected_stop: Option<ThumbId>,
    domain_renderer: Arc<ColorSliderDomainRenderer>,
    track_context: ColorSliderTrackContext,
    add_stop_button: Entity<Button>,
    mesh_reset_button: Entity<Button>,
    export_button: Entity<Button>,
    export_status: Option<String>,
    stop_delete_buttons: HashMap<ThumbId, Entity<Button>>,
    stop_list: color_list::ColorFieldList,
    point_list: color_list::ColorFieldList,
    rotation_slider: Entity<SliderControl>,
    saturation_slider: Entity<SliderControl>,
    vibrance_slider: Entity<SliderControl>,
    image_adjustments: super::paint::ImageAdjustments,
    spread_slider: Entity<SliderControl>,
    spread_percent: f32,
    point_spread_slider: Entity<SliderControl>,
    point_spreads: Vec<f32>,
    softness_slider: Entity<SliderControl>,
    softness_percent: f32,
    blend_selector: Entity<Selector>,
    freeform_hsl: bool,
    type_selector: Entity<Selector>,
    renderer_selector: Entity<Selector>,
    mesh_grid_selector: Entity<Selector>,
    mesh_aspect_ratio_selector: Entity<Selector>,
    rotation_deg: f32,
    gradient_type: GradientType,
    preview_renderer: PreviewRenderer,
    mesh_grid_preset: MeshGridPreset,
    mesh_aspect_ratio_preset: MeshAspectRatioPreset,
    mesh_points: Vec<MeshPoint>,
    inactive_mesh_points: Vec<MeshPoint>,
    mesh_point_ids: Vec<u64>,
    inactive_mesh_point_ids: Vec<u64>,
    next_point_id: u64,
    inactive_mesh_background: gpui::Hsla,
    mesh_state_is_freeform: bool,
    random_seed: u64,
    freeform_buttons: Vec<Entity<Button>>,
    mesh_background: gpui::Hsla,
    selected_mesh_point: Option<usize>,
    mesh_color_target: MeshColorTarget,
    active_mesh_drag: Option<usize>,
    mesh_controls_visible: bool,
    freeform_controls_visible: bool,
    visibility_button: Entity<Button<bool>>,
    copy_css_button: Entity<Button>,
    point_buttons: Vec<[Entity<Button>; 2]>,
    preview_size: Size<Pixels>,
    preview_image_cache: Option<(PreviewImageCacheKey, Arc<RenderImage>)>,
    active_preview_strategy: &'static str,
    last_render_ms: Option<f32>,
    color_picker: Entity<SvTrianglePicker>,
    color_picker_open: bool,
    stop_swatch_bounds: HashMap<ThumbId, Bounds<Pixels>>,
    mesh_swatch_bounds: HashMap<usize, Bounds<Pixels>>,
    mesh_background_swatch_bounds: Option<Bounds<Pixels>>,
    mesh_preview_container_size: Size<Pixels>,
    mesh_preview_bounds: Option<Bounds<Pixels>>,
    render_task: Option<gpui::Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl GradientBuilder {
    pub(crate) fn export_button(&self) -> Entity<Button> {
        self.export_button.clone()
    }

    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let theme_is_dark = matches!(look.mode(), ThemeMode::Dark);
        let start = gpui::hsla(198.0 / 360.0, 1.0, 0.24, 1.0);
        let middle = gpui::hsla(313.0 / 360.0, 0.48, 0.45, 1.0);
        let end = gpui::hsla(33.0 / 360.0, 1.0, 0.46, 1.0);
        let mesh_background = default_mesh_background();

        let slider_builder = ColorSliderBuilder::gradient(
            "color-viz-gradient-stops",
            0.0,
            vec![start, middle, end].into_iter().map(gpui_luma::color::gpui_bridge::from_hsla).collect(),
        )
        .expect("valid demo gradient colors")
        .thumb_policy(SliderThumbPolicy {
            min_count: 2,
            max_count: 8,
            min_distance: 0.02,
            allow_insert: true,
            allow_remove: true,
            allow_overlap: false,
        })
        .thumb_values([(0.0, Some(start)), (0.5, Some(middle)), (1.0, Some(end))])
        .size(ControlSize::Sm)
        .thumb_medium()
        .theme_is_dark(theme_is_dark);
        let domain_renderer = slider_builder.domain_renderer();
        let track_context = slider_builder.track_context();
        let gradient_stops = slider_builder.spawn(cx);

        let mut stop_colors = HashMap::new();
        for thumb in gradient_stops.read(cx).thumbs() {
            stop_colors.insert(thumb.id, thumb.preview.unwrap_or(start));
        }
        let selected_stop = None;
        let add_stop_button = crate::theme::icon_button("color-viz-gradient-add-stop", LucideIcon::Plus)
            .look(look.as_ref())
            .content_only()
            .radius(radix::Radius::Medium)
            .size(radix::ButtonSize::One)
            .spawn(cx)
            .tooltip(Tooltip::new("Add color stop"), cx);
        let mesh_reset_button = crate::theme::icon_button("color-viz-mesh-reset", LucideIcon::RotateCcw)
            .look(look.as_ref())
            .content_only()
            .radius(radix::Radius::Medium)
            .size(radix::ButtonSize::One)
            .spawn(cx)
            .tooltip(Tooltip::new("Reset canvas"), cx);

        let export_button = radix::Button::new("color-viz-export")
            .look(look.as_ref())
            .outline()
            .size(radix::ButtonSize::One)
            .icon(LucideIcon::Download)
            .label("Export PNG")
            .spawn(cx);
        let rotation_slider = radix::Slider::new("color-viz-gradient-rotation")
            .look(look.as_ref())
            .size(radix::SliderSize::One)
            .range(0.0..360.0)
            .step(1.0)
            .value(90.0)
            .spawn(cx);
        let saturation_slider = radix::Slider::new("color-viz-image-saturation")
            .look(look.as_ref())
            .size(radix::SliderSize::One)
            .range(-100.0..100.0)
            .step(1.0)
            .value(0.0)
            .spawn(cx);
        let vibrance_slider = radix::Slider::new("color-viz-image-vibrance")
            .look(look.as_ref())
            .size(radix::SliderSize::One)
            .range(-100.0..100.0)
            .step(1.0)
            .value(0.0)
            .spawn(cx);
        let spread_slider = radix::Slider::new("color-viz-freeform-spread")
            .look(look.as_ref())
            .size(radix::SliderSize::One)
            .range(10.0..150.0)
            .step(1.0)
            .value(100.0)
            .spawn(cx);
        let point_spread_slider = radix::Slider::new("color-viz-point-spread")
            .look(look.as_ref())
            .size(radix::SliderSize::One)
            .range(10.0..200.0)
            .step(1.0)
            .value(100.0)
            .with_template_modifier(|root, _| {
                use gpui::prelude::*;
                root.debug_selector(|| "point-spread-control".into())
            })
            .spawn(cx);
        let softness_slider = radix::Slider::new("color-viz-falloff-softness")
            .look(look.as_ref())
            .size(radix::SliderSize::One)
            .range(25.0..200.0)
            .step(1.0)
            .value(100.0)
            .spawn(cx);
        let blend_selector = radix::Selector::new("color-viz-freeform-blend")
            .look(look.as_ref())
            .size(radix::ButtonSize::One)
            .items(vec![
                gpui_luma::controls::selector::SelectorItem::new("rgb").label("RGB"),
                gpui_luma::controls::selector::SelectorItem::new("hsl").label("HSL"),
            ])
            .selected_id("rgb")
            .spawn(cx);
        let top_tabs = Tabs::new("color-viz-builder-tabs")
            .template(crate::studio_tabs::template(look.clone(), true))
            .items(builder_tab_items())
            .active("gradients")
            .size(ControlSize::Sm)
            .width_mode(TabsWidthMode::Uniform)
            .spawn(cx);
        let type_selector = radix::Selector::new("color-viz-gradient-type")
            .look(look.as_ref())
            .size(radix::ButtonSize::One)
            .items(type_items())
            .selected_id("linear")
            .spawn(cx);
        let renderer_selector = radix::Selector::new("color-viz-gradient-renderer")
            .look(look.as_ref())
            .size(radix::ButtonSize::One)
            .items(renderer_items())
            .selected_id("quads")
            .spawn(cx);
        let mesh_grid_selector = radix::Selector::new("color-viz-mesh-grid")
            .look(look.as_ref())
            .size(radix::ButtonSize::One)
            .items(mesh_grid_items())
            .selected_id("3x4")
            .spawn(cx);
        let mesh_aspect_ratio_selector = radix::Selector::new("color-viz-mesh-aspect")
            .look(look.as_ref())
            .size(radix::ButtonSize::One)
            .items(mesh_aspect_ratio_items())
            .selected_id("fill")
            .spawn(cx);
        let color_picker = cx.new(|cx| SvTrianglePicker::new(start, cx));
        let mesh_grid_preset = MeshGridPreset::SampleThreeByFour;
        let mesh_aspect_ratio_preset = MeshAspectRatioPreset::Fill;

        let visibility_button = radix::Button::new("color-viz-point-visibility")
            .typed(true)
            .role(gpui_luma::controls::button_family::ButtonFamilyRole::Icon)
            .content(|context, _| {
                use gpui::prelude::*;
                gpui::svg()
                    .path(
                        if context.data {
                            LucideIcon::Eye
                        } else {
                            LucideIcon::EyeOff
                        }
                        .asset_path(),
                    )
                    .size(px(context.look.icon_size))
                    .text_color(context.look.foreground)
                    .into_any_element()
            })
            .look(&look)
            .content_only()
            .size(radix::ButtonSize::One)
            .with_template_modifier(|root, _| {
                use gpui::prelude::*;
                root.debug_selector(|| "point-visibility".into())
            })
            .spawn(cx)
            .tooltip(Tooltip::new("Hide/show points (right-click canvas)"), cx);
        let copy_css_button = crate::theme::icon_button("color-viz-copy-css", LucideIcon::Copy)
            .look(&look)
            .content_only()
            .size(radix::ButtonSize::One)
            .spawn(cx)
            .tooltip(Tooltip::new("Copy gradient CSS"), cx);
        let mut builder = Self {
            look: look.clone(),
            top_tabs: top_tabs.clone(),
            section_accordions: Vec::new(),
            selected_tab: BuilderTab::Gradients,
            gradient_stops: gradient_stops.clone(),
            stop_colors,
            selected_stop,
            domain_renderer,
            track_context,
            add_stop_button: add_stop_button.clone(),
            mesh_reset_button: mesh_reset_button.clone(),
            export_button: export_button.clone(),
            export_status: None,
            stop_delete_buttons: HashMap::new(),
            stop_list: color_list::ColorFieldList::new(cx),
            point_list: color_list::ColorFieldList::new(cx),
            rotation_slider: rotation_slider.clone(),
            saturation_slider: saturation_slider.clone(),
            vibrance_slider: vibrance_slider.clone(),
            image_adjustments: super::paint::ImageAdjustments::default(),
            spread_slider: spread_slider.clone(),
            spread_percent: 100.0,
            point_spread_slider: point_spread_slider.clone(),
            point_spreads: vec![100.0; freeform::default_points().len()],
            softness_slider: softness_slider.clone(),
            softness_percent: 100.0,
            blend_selector: blend_selector.clone(),
            freeform_hsl: false,
            type_selector: type_selector.clone(),
            renderer_selector: renderer_selector.clone(),
            mesh_grid_selector: mesh_grid_selector.clone(),
            mesh_aspect_ratio_selector: mesh_aspect_ratio_selector.clone(),
            rotation_deg: 90.0,
            gradient_type: GradientType::Linear,
            preview_renderer: PreviewRenderer::Quads,
            mesh_grid_preset,
            mesh_aspect_ratio_preset,
            mesh_points: default_mesh_points(mesh_grid_preset),
            inactive_mesh_points: freeform::default_points(),
            mesh_point_ids: (0..default_mesh_points(mesh_grid_preset).len() as u64).collect(),
            inactive_mesh_point_ids: (default_mesh_points(mesh_grid_preset).len() as u64
                ..(default_mesh_points(mesh_grid_preset).len() + freeform::default_points().len()) as u64)
                .collect(),
            next_point_id: (default_mesh_points(mesh_grid_preset).len() + freeform::default_points().len()) as u64,
            inactive_mesh_background: mesh_background,
            mesh_state_is_freeform: false,
            random_seed: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64
                | 1,
            freeform_buttons: Vec::new(),
            mesh_background,
            selected_mesh_point: None,
            mesh_color_target: MeshColorTarget::Background,
            active_mesh_drag: None,
            mesh_controls_visible: true,
            freeform_controls_visible: true,
            visibility_button: visibility_button.clone(),
            copy_css_button: copy_css_button.clone(),
            point_buttons: Vec::new(),
            preview_size: size(px(0.0), px(0.0)),
            preview_image_cache: None,
            active_preview_strategy: "quads",
            last_render_ms: None,
            color_picker: color_picker.clone(),
            color_picker_open: false,
            stop_swatch_bounds: HashMap::new(),
            mesh_swatch_bounds: HashMap::new(),
            mesh_background_swatch_bounds: None,
            mesh_preview_container_size: size(px(0.0), px(0.0)),
            mesh_preview_bounds: None,
            render_task: None,
            _subscriptions: Vec::new(),
        };

        builder._subscriptions.push(cx.subscribe(&visibility_button, |this, _, event: &ButtonEvent, cx| {
            if event.is_click() {
                this.toggle_mesh_controls(cx);
            }
        }));
        builder._subscriptions.push(cx.subscribe(&copy_css_button, |this, _, event: &ButtonEvent, cx| {
            if event.is_click() {
                this.copy_gradient_css(cx);
            }
        }));
        builder.wire_subscriptions(
            cx,
            top_tabs,
            gradient_stops,
            add_stop_button,
            mesh_reset_button,
            rotation_slider,
            type_selector,
            renderer_selector,
            mesh_grid_selector,
            mesh_aspect_ratio_selector,
            color_picker,
        );
        builder._subscriptions.push(cx.subscribe(&spread_slider, |this, _, event: &SliderEvent, cx| {
            if let SliderEvent::Change { value, .. } | SliderEvent::Release { value, .. } = event {
                this.spread_percent = *value;
                this.preview_image_cache = None;
                let _ = this.ensure_preview_image_cache(this.preview_size, cx);
                cx.notify();
            }
        }));
        builder._subscriptions.push(cx.subscribe(&blend_selector, |this, _, event: &SelectorEvent, cx| {
            if let SelectorEvent::Change { item_id, .. } = event {
                this.freeform_hsl = item_id.as_ref() == "hsl";
                this.preview_image_cache = None;
                let _ = this.ensure_preview_image_cache(this.preview_size, cx);
                cx.notify();
            }
        }));
        builder._subscriptions.push(cx.subscribe(&point_spread_slider, |this, _, event: &SliderEvent, cx| {
            if this.selected_tab != BuilderTab::Freeform {
                return;
            }
            if let SliderEvent::Change { value, .. } | SliderEvent::Release { value, .. } = event {
                if let Some(spread) = this.selected_mesh_point.and_then(|index| this.point_spreads.get_mut(index)) {
                    *spread = *value;
                }
                this.preview_image_cache = None;
                let _ = this.ensure_preview_image_cache(this.preview_size, cx);
                cx.notify();
            }
        }));
        builder._subscriptions.push(cx.subscribe(&softness_slider, |this, _, event: &SliderEvent, cx| {
            if let SliderEvent::Change { value, .. } | SliderEvent::Release { value, .. } = event {
                this.softness_percent = *value;
                this.preview_image_cache = None;
                let _ = this.ensure_preview_image_cache(this.preview_size, cx);
                cx.notify();
            }
        }));
        for (slider, saturation) in [(saturation_slider, true), (vibrance_slider, false)] {
            builder._subscriptions.push(cx.subscribe(&slider, move |this, _, event: &SliderEvent, cx| {
                if let SliderEvent::Change { value, .. } | SliderEvent::Release { value, .. } = event {
                    if saturation {
                        this.image_adjustments.saturation = *value;
                    } else {
                        this.image_adjustments.vibrance = *value;
                    }
                    this.preview_image_cache = None;
                    this.render_task = None;
                    let _ = this.ensure_preview_image_cache(this.preview_size, cx);
                    cx.notify();
                }
            }));
        }
        builder._subscriptions.push(cx.subscribe(&export_button, |this, _, event: &ButtonEvent, cx| {
            if matches!(event, ButtonEvent::Click) {
                this.export_png(cx);
            }
        }));
        builder.init_section_accordions(cx);
        builder.init_freeform_buttons(cx);
        builder.sync_stop_buttons(cx);
        builder.rebuild_stops(cx);
        builder
    }

    #[allow(clippy::too_many_arguments)]
    fn wire_subscriptions(
        &mut self,
        cx: &mut Context<Self>,
        top_tabs: Entity<Tabs>,
        gradient_stops: Entity<SliderControl>,
        add_stop_button: Entity<Button>,
        mesh_reset_button: Entity<Button>,
        rotation_slider: Entity<SliderControl>,
        type_selector: Entity<Selector>,
        renderer_selector: Entity<Selector>,
        mesh_grid_selector: Entity<Selector>,
        mesh_aspect_ratio_selector: Entity<Selector>,
        color_picker: Entity<SvTrianglePicker>,
    ) {
        self._subscriptions.push(cx.subscribe(&gradient_stops, |this, _, event, cx| {
            this.handle_stops_event(event, cx);
        }));
        self._subscriptions.push(cx.subscribe(&top_tabs, |this, _, event: &TabsEvent, cx| {
            this.handle_top_tabs_event(event, cx);
        }));
        self._subscriptions.push(cx.subscribe(&add_stop_button, |this, _, event: &ButtonEvent, cx| {
            if matches!(event, ButtonEvent::Click) {
                this.handle_add_stop(cx);
            }
        }));
        self._subscriptions.push(cx.subscribe(&mesh_reset_button, |this, _, event: &ButtonEvent, cx| {
            if matches!(event, ButtonEvent::Click) {
                this.reset_mesh_state(cx);
            }
        }));
        self._subscriptions.push(cx.subscribe(&rotation_slider, |this, _, event, cx| {
            this.handle_rotation_event(event, cx);
        }));
        self._subscriptions.push(cx.subscribe(&type_selector, |this, _, event, cx| {
            this.handle_type_event(event, cx);
        }));
        self._subscriptions.push(cx.subscribe(&renderer_selector, |this, _, event, cx| {
            this.handle_renderer_event(event, cx);
        }));
        self._subscriptions.push(cx.subscribe(&mesh_grid_selector, |this, _, event, cx| {
            this.handle_mesh_grid_event(event, cx);
        }));
        self._subscriptions.push(cx.subscribe(&mesh_aspect_ratio_selector, |this, _, event, cx| {
            this.handle_mesh_aspect_ratio_event(event, cx);
        }));
        self._subscriptions.push(cx.observe(&color_picker, |this, _, cx| {
            this.apply_picker_color(cx);
        }));
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[test]
    fn redesigned_inspector_retains_freeform_edits_across_modes() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let look = crate::theme::default_look();
        look.set_mode(ThemeMode::Dark);
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(look.clone(), cx));
        cx.run_until_parked();
        let tabs = cx.update(|_, cx| view.read(cx).top_tabs.clone());
        for id in ["freeform", "mesh", "gradients", "freeform"] {
            tabs.update(cx, |tabs, cx| {
                tabs.set_active(id, cx);
                cx.emit(TabsEvent::Activate { tab_id: id.into(), label: id.into() });
            });
            cx.run_until_parked();
            if id == "freeform" {
                view.update(cx, |view, cx| {
                    assert_eq!(view.mesh_points.len(), 4);
                    if view.point_spreads[0] == 100.0 {
                        view.point_spreads[0] = 65.0;
                        view.mesh_points[0].u = 0.25;
                        view.preview_image_cache = None;
                        cx.notify();
                    } else {
                        assert_eq!(view.point_spreads[0], 65.0);
                        assert_eq!(view.mesh_points[0].u, 0.25);
                    }
                });
                cx.run_until_parked();
            }
        }
        view.update(cx, |view, cx| {
            view.deselect_freeform_point(cx);
            assert_eq!(view.selected_mesh_point, None);
            for index in [1, 0, 3, 2] {
                view.begin_mesh_drag(index, cx);
                assert_eq!(view.selected_mesh_point, Some(index));
                assert_eq!(view.mesh_color_target, MeshColorTarget::Point(index));
                view.finish_mesh_drag(cx);
                view.open_color_picker_for_mesh_point(index, cx);
                assert_eq!(view.selected_mesh_point, Some(index));
            }
            view.deselect_freeform_point(cx);
            assert_eq!(view.selected_mesh_point, None);
        });
        cx.run_until_parked();
    }
    #[test]
    fn full_aspect_and_visibility_follow_each_mode_and_both_inputs() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(Look::built_in().into(), cx));
        cx.run_until_parked();
        let tabs = cx.update(|_, cx| view.read(cx).top_tabs.clone());
        let mut freeform_visible = true;
        for id in ["gradients", "mesh", "freeform", "mesh", "freeform"] {
            tabs.update(cx, |tabs, cx| {
                tabs.set_active(id, cx);
                cx.emit(TabsEvent::Activate { tab_id: id.into(), label: id.into() });
            });
            cx.run_until_parked();
            cx.update(|_, cx| {
                let builder = view.read(cx);
                assert_eq!(builder.mesh_aspect_ratio_preset, MeshAspectRatioPreset::Fill);
                assert_eq!(
                    builder.mesh_aspect_ratio_selector.read(cx).selected_id().map(|id| id.as_ref()),
                    Some("fill")
                );
                assert!(builder.preview_size.width > px(0.0));
                assert!(builder.preview_size.height > px(0.0));
                if id != "gradients" {
                    assert_eq!(*builder.visibility_button.read(cx).data(), builder.points_visible());
                }
            });
            if id == "mesh" {
                let bounds = cx.debug_bounds("point-visibility").expect("visible toggle");
                cx.simulate_click(bounds.center(), Default::default());
                cx.run_until_parked();
                cx.update(|_, cx| {
                    let builder = view.read(cx);
                    assert!(!builder.mesh_controls_visible);
                    assert_eq!(builder.freeform_controls_visible, freeform_visible);
                    assert!(!*builder.visibility_button.read(cx).data());
                });
                let point = cx.update(|_, cx| view.read(cx).mesh_preview_bounds.expect("preview bounds").center());
                cx.simulate_mouse_down(point, gpui::MouseButton::Right, Default::default());
                cx.simulate_mouse_up(point, gpui::MouseButton::Right, Default::default());
                cx.run_until_parked();
                cx.update(|_, cx| assert!(view.read(cx).mesh_controls_visible));
            } else if id == "freeform" {
                cx.update(|_, cx| assert_eq!(view.read(cx).freeform_controls_visible, freeform_visible));
                freeform_visible = !freeform_visible;
                let point = cx.update(|_, cx| view.read(cx).mesh_preview_bounds.expect("preview bounds").center());
                cx.simulate_mouse_down(point, gpui::MouseButton::Right, Default::default());
                cx.simulate_mouse_up(point, gpui::MouseButton::Right, Default::default());
                cx.run_until_parked();
                cx.update(|_, cx| {
                    let builder = view.read(cx);
                    assert!(builder.mesh_controls_visible);
                    assert_eq!(builder.freeform_controls_visible, freeform_visible);
                    assert_eq!(*builder.visibility_button.read(cx).data(), builder.freeform_controls_visible);
                });
            }
        }
    }

    #[test]
    fn row_actions_and_spread_target_the_corresponding_point_after_deletion() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(Look::built_in().into(), cx));
        view.update(cx, |builder, cx| {
            builder.handle_top_tabs_event(
                &TabsEvent::Activate { tab_id: "freeform".into(), label: "Freeform".into() },
                cx,
            );
            builder.point_spreads = vec![50.0, 70.0, 90.0, 110.0];
            builder.selected_mesh_point = Some(0);
            builder.random_seed = 1;
            builder.sync_point_spread(cx);
        });
        cx.run_until_parked();
        let retained_key = cx.update(|_, cx| view.read(cx).mesh_point_ids[2]);
        let (color_action, delete_action, before) = cx.update(|_, cx| {
            let builder = view.read(cx);
            (
                builder.point_buttons[2][1].clone(),
                builder.point_buttons[1][0].clone(),
                builder.mesh_points.clone(),
            )
        });
        color_action.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let builder = view.read(cx);
            assert_eq!(builder.selected_mesh_point, Some(2));
            assert_ne!(builder.mesh_points[2].color, before[2].color);
            for index in [0, 1, 3] {
                assert_eq!(builder.mesh_points[index].color, before[index].color);
            }
            assert_eq!(builder.point_spread_slider.read(cx).value(), 90.0);
        });
        delete_action.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        let (slider, action_after_delete) = cx.update(|_, cx| {
            let builder = view.read(cx);
            assert_eq!(builder.mesh_points.len(), 3);
            assert_eq!(builder.mesh_point_ids[1], retained_key);
            assert_eq!(builder.point_list.state.selected_keys().copied().collect::<Vec<_>>(), vec![retained_key]);
            assert_eq!(builder.mesh_points[1].u, before[2].u);
            assert_eq!(builder.point_spreads, vec![50.0, 90.0, 110.0]);
            (builder.point_spread_slider.clone(), builder.point_buttons[1][1].clone())
        });
        action_after_delete.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        slider.update(cx, |slider, cx| {
            let thumb_id = slider.thumbs()[0].id;
            cx.emit(SliderEvent::Change { thumb_id, value: 125.0 });
        });
        cx.run_until_parked();
        cx.update(|_, cx| assert_eq!(view.read(cx).point_spreads, vec![50.0, 125.0, 110.0]));
        view.update(cx, |builder, cx| {
            builder.deselect_freeform_point(cx);
            assert_eq!(builder.selected_mesh_point, None);
        });
    }

    #[test]
    fn css_copy_uses_the_displayed_gradient_after_edits() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(Look::built_in().into(), cx));
        view.update(cx, |builder, cx| {
            builder.rotation_deg = 137.0;
            builder.handle_add_stop(cx);
        });
        cx.run_until_parked();
        let (button, expected) =
            view.update(cx, |builder, cx| (builder.copy_css_button.clone(), builder.gradient_spec(cx)));
        button.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        cx.update(|_, cx| assert_eq!(cx.read_from_clipboard().and_then(|item| item.text()), Some(expected)));
    }
    #[test]
    fn collapsed_accordion_sections_have_uniform_spacing() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(Look::built_in().into(), cx));
        let sections = cx.update(|_, cx| view.read(cx).section_accordions[0].clone());
        for section in sections {
            section.update(cx, |section, cx| section.toggle_item(0, cx));
        }
        cx.run_until_parked();
        let bounds = [
            "color-viz-gradients-section-CANVAS",
            "color-viz-gradients-section-FIELDS",
            "color-viz-gradients-section-COLOR",
            "color-viz-gradients-section-RENDER",
        ]
        .map(|selector| cx.debug_bounds(selector).expect("collapsed section header"));
        let step = bounds[1].top() - bounds[0].top();
        assert!(step > px(0.0) && step < px(50.0));
        for pair in bounds.windows(2) {
            assert!(((pair[1].top() - pair[0].top()) - step).abs() < px(0.5));
            assert!((pair[1].size.height - pair[0].size.height).abs() < px(0.5));
        }
    }

    #[test]
    fn gradient_sections_collapse_independently_and_preserve_controls() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(Look::built_in().into(), cx));
        cx.run_until_parked();
        for (index, id, selector, row_selector) in [
            (0, "gradients", "color-viz-gradients-section-FIELDS", "color-viz-stop-list-first-row"),
            (1, "mesh", "color-viz-mesh-section-FIELDS", "color-viz-point-list-first-row"),
            (2, "freeform", "color-viz-freeform-section-FIELDS", "color-viz-point-list-first-row"),
        ] {
            view.update(cx, |builder, cx| {
                builder.handle_top_tabs_event(&TabsEvent::Activate { tab_id: id.into(), label: id.into() }, cx)
            });
            cx.run_until_parked();
            let (header, slider, value) = cx.update(|_, cx| {
                let builder = view.read(cx);
                (
                    builder.section_accordions[index][1].clone(),
                    builder.saturation_slider.clone(),
                    builder.saturation_slider.read(cx).value(),
                )
            });
            assert!(cx.debug_bounds(row_selector).is_some());
            let bounds = cx.debug_bounds(selector).expect("section disclosure");
            cx.simulate_click(bounds.center(), Default::default());
            cx.run_until_parked();
            assert!(cx.debug_bounds(row_selector).is_none());
            cx.update(|_, cx| {
                let builder = view.read(cx);
                assert!(!header.read(cx).is_expanded(&"section".into()));
                assert!(builder.section_accordions[index][0].read(cx).is_expanded(&"section".into()));
                assert!(builder.section_accordions[index][2].read(cx).is_expanded(&"section".into()));
                assert_eq!(slider.read(cx).value(), value);
            });
            cx.simulate_keystrokes("enter");
            cx.run_until_parked();
            assert!(cx.debug_bounds(row_selector).is_some());
        }
    }

    #[test]
    fn color_lists_start_unselected_and_mode_changes_do_not_select_items() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(Look::built_in().into(), cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let builder = view.read(cx);
            assert_eq!(builder.selected_stop, None);
            assert_eq!(builder.selected_mesh_point, None);
            assert_eq!(builder.stop_list.state.selected_keys().count(), 0);
        });
        for id in ["mesh", "freeform", "mesh", "freeform"] {
            view.update(cx, |builder, cx| {
                builder.handle_top_tabs_event(&TabsEvent::Activate { tab_id: id.into(), label: id.into() }, cx)
            });
            cx.run_until_parked();
            view.update(cx, |builder, cx| {
                assert_eq!(builder.selected_mesh_point, None);
                assert_eq!(builder.point_list.state.selected_keys().count(), 0);
                builder.begin_mesh_drag(0, cx);
                assert_eq!(builder.selected_mesh_point, Some(0));
                builder.finish_mesh_drag(cx);
                builder.reset_mesh_state(cx);
            });
            cx.run_until_parked();
            cx.update(|_, cx| {
                let builder = view.read(cx);
                assert_eq!(builder.selected_mesh_point, None);
                assert_eq!(builder.point_list.state.selected_keys().count(), 0);
            });
        }
    }

    #[test]
    fn color_lists_own_focus_and_leave_child_controls_interactive() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(Look::built_in().into(), cx));
        cx.run_until_parked();
        let thumb_id = view.update(cx, |builder, cx| builder.ordered_stop_rows(cx)[0].0);
        let bounds = cx.debug_bounds("color-viz-stop-list-first-row").expect("stop row");
        // The leading label selects/focuses the collection without opening a picker.
        cx.simulate_click(gpui::point(bounds.left() + gpui::px(12.0), bounds.center().y), Default::default());
        cx.run_until_parked();
        cx.update(|window, cx| {
            let builder = view.read(cx);
            assert_eq!(builder.selected_stop, Some(thumb_id));
            assert!(builder.stop_list.binding.focus_handle().is_focused(window));
        });
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        view.update(cx, |builder, cx| assert_eq!(builder.selected_stop, Some(builder.ordered_stop_rows(cx)[1].0)));
        let canvas = cx.debug_bounds("gradient-preview").expect("preview");
        cx.simulate_click(canvas.center(), Default::default());
        cx.run_until_parked();
        cx.update(|window, cx| {
            let builder = view.read(cx);
            assert_eq!(builder.selected_stop, None);
            assert!(!builder.stop_list.binding.focus_handle().is_focused(window));
        });
        for id in ["mesh", "freeform"] {
            view.update(cx, |builder, cx| {
                builder.handle_top_tabs_event(&TabsEvent::Activate { tab_id: id.into(), label: id.into() }, cx)
            });
            cx.run_until_parked();
            let bounds = cx.debug_bounds("color-viz-point-list-first-row").expect("point row");
            cx.simulate_click(
                gpui::point(
                    bounds.left() + gpui::px(if id == "freeform" { 42.0 } else { 12.0 }),
                    bounds.top() + gpui::px(20.0),
                ),
                Default::default(),
            );
            cx.run_until_parked();
            cx.update(|window, cx| {
                let builder = view.read(cx);
                assert_eq!(builder.selected_mesh_point, Some(0));
                assert!(builder.point_list.binding.focus_handle().is_focused(window));
            });
            cx.simulate_keystrokes("down");
            cx.run_until_parked();
            cx.update(|_, cx| assert_eq!(view.read(cx).selected_mesh_point, Some(1)));
            if id == "freeform" {
                let spread = cx.debug_bounds("point-spread-control").expect("selected spread");
                cx.simulate_click(spread.center(), Default::default());
                cx.run_until_parked();
                cx.simulate_keystrokes("down");
                cx.run_until_parked();
                cx.update(|window, cx| {
                    let builder = view.read(cx);
                    assert_eq!(builder.selected_mesh_point, Some(1));
                    assert!(!builder.point_list.binding.focus_handle().is_focused(window));
                });
            }
            let canvas = cx.update(|_, cx| view.read(cx).mesh_preview_bounds.expect("preview"));
            cx.simulate_click(canvas.center(), Default::default());
            cx.run_until_parked();
            cx.update(|window, cx| {
                let builder = view.read(cx);
                assert_eq!(builder.selected_mesh_point, None);
                assert!(!builder.point_list.binding.focus_handle().is_focused(window));
            });
        }
    }
}
