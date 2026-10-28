use super::*;

use gpui_luma_color::color_slider::{GradientDelegate, GradientStop, refresh_color_slider, update_domain_delegate};

use super::super::color::format_css_gradient;
use super::super::paint::{
    color_at_position, mesh_dimensions, rasterize_gradient_preview, rasterize_mesh_gradient_preview, sorted_stops,
};
use super::mesh::{
    MESH_POINT_GAP, default_mesh_background, default_mesh_points, fit_aspect_ratio, mesh_point_index,
    mesh_preview_content_size, preview_gradient_cache_key, preview_mesh_cache_key,
};

impl GradientBuilder {
    pub(super) fn handle_stops_event(&mut self, event: &SliderEvent, cx: &mut Context<Self>) {
        match event {
            SliderEvent::ThumbAdded { thumb_id, .. } => {
                self.hydrate_thumb_color(*thumb_id, cx);
                self.selected_stop = Some(*thumb_id);
                self.sync_stop_buttons(cx);
                self.rebuild_stops(cx);
            }
            SliderEvent::ThumbRemoved { thumb_id } => {
                self.stop_colors.remove(thumb_id);
                self.sync_stop_buttons(cx);
                self.rebuild_stops(cx);
            }
            SliderEvent::ThumbSelected { thumb_id } => {
                self.selected_stop = Some(*thumb_id);
                if self.color_picker_open {
                    let color = self.selected_color(cx);
                    self.color_picker.update(cx, |picker, cx| picker.set_color(color, cx));
                }
                cx.notify();
            }
            SliderEvent::Change { .. } | SliderEvent::Release { .. } => {
                self.rebuild_stops(cx);
            }
            _ => {}
        }
    }

    pub(super) fn handle_add_stop(&mut self, cx: &mut Context<Self>) {
        let position = self.new_stop_position(cx);
        let color = color_at_position(&self.preview_stops(cx), position);
        let mut inserted = None;
        self.gradient_stops.update(cx, |slider, cx| {
            inserted = slider.insert_thumb_at(position, cx);
        });

        if let Some(thumb_id) = inserted {
            self.stop_colors.insert(thumb_id, color);
            self.selected_stop = Some(thumb_id);
            self.sync_stop_buttons(cx);
            self.rebuild_stops(cx);
            cx.notify();
        }
    }

    pub(super) fn handle_delete_stop(&mut self, thumb_id: ThumbId, cx: &mut Context<Self>) {
        self.stop_colors.remove(&thumb_id);
        if self.selected_stop == Some(thumb_id) {
            self.selected_stop = None;
        }
        self.gradient_stops.update(cx, |slider, cx| {
            slider.remove_thumb_id(thumb_id, cx);
        });
        self.sync_stop_buttons(cx);
        self.rebuild_stops(cx);
        cx.notify();
    }

    pub(super) fn handle_rotation_event(&mut self, event: &SliderEvent, cx: &mut Context<Self>) {
        if let SliderEvent::Change { value, .. } | SliderEvent::Release { value, .. } = event {
            self.rotation_deg = *value;
            let _ = self.ensure_preview_image_cache(self.preview_size, cx);
            cx.notify();
        }
    }

    pub(super) fn handle_top_tabs_event(&mut self, event: &TabsEvent, cx: &mut Context<Self>) {
        let TabsEvent::Activate { tab_id, .. } = event else {
            return;
        };
        let next = match tab_id.as_ref() {
            "mesh" => BuilderTab::Mesh,
            "freeform" => BuilderTab::Freeform,
            _ => BuilderTab::Gradients,
        };
        if self.selected_tab != next {
            if next != BuilderTab::Gradients && (next == BuilderTab::Freeform) != self.mesh_state_is_freeform {
                std::mem::swap(&mut self.mesh_points, &mut self.inactive_mesh_points);
                std::mem::swap(&mut self.mesh_point_ids, &mut self.inactive_mesh_point_ids);
                std::mem::swap(&mut self.mesh_background, &mut self.inactive_mesh_background);
                self.mesh_state_is_freeform = next == BuilderTab::Freeform;
                self.selected_mesh_point = None;
                self.mesh_color_target = MeshColorTarget::Background;
                self.mesh_swatch_bounds.clear();
            }
            self.preview_image_cache = None;
            self.selected_tab = next;
            self.sync_visibility_button(cx);
            if next == BuilderTab::Freeform {
                self.sync_freeform_buttons(cx);
                self.sync_point_spread(cx);
            }
            self.render_task = None;
            self.active_mesh_drag = None;
            self.color_picker_open = false;
            let _ = self.ensure_preview_image_cache(self.preview_size, cx);
            cx.notify();
        }
    }

    pub(super) fn handle_type_event(&mut self, event: &SelectorEvent, cx: &mut Context<Self>) {
        let SelectorEvent::Change { item_id, .. } = event else {
            return;
        };
        let next = match item_id.as_ref() {
            "radial" => GradientType::Radial,
            "angular" => GradientType::Angular,
            _ => GradientType::Linear,
        };
        if self.gradient_type != next {
            self.gradient_type = next;
            let _ = self.ensure_preview_image_cache(self.preview_size, cx);
            cx.notify();
        }
    }

    pub(super) fn handle_renderer_event(&mut self, event: &SelectorEvent, cx: &mut Context<Self>) {
        let SelectorEvent::Change { item_id, .. } = event else {
            return;
        };
        let next = match item_id.as_ref() {
            "render" => PreviewRenderer::RenderImageSync,
            "render_async" => PreviewRenderer::RenderImageAsync,
            _ => PreviewRenderer::Quads,
        };
        if self.preview_renderer != next {
            self.preview_renderer = next;
            let _ = self.ensure_preview_image_cache(self.preview_size, cx);
            cx.notify();
        }
    }

    pub(super) fn handle_mesh_grid_event(&mut self, event: &SelectorEvent, cx: &mut Context<Self>) {
        let SelectorEvent::Change { item_id, .. } = event else {
            return;
        };
        let next = MeshGridPreset::from_item_id(item_id.as_ref());
        if self.mesh_grid_preset != next {
            self.mesh_grid_preset = next;
            self.reset_mesh_state(cx);
        }
    }

    pub(super) fn handle_mesh_aspect_ratio_event(&mut self, event: &SelectorEvent, cx: &mut Context<Self>) {
        let SelectorEvent::Change { item_id, .. } = event else {
            return;
        };
        let next = MeshAspectRatioPreset::from_item_id(item_id.as_ref());
        if self.mesh_aspect_ratio_preset != next {
            self.mesh_aspect_ratio_preset = next;
            let fitted_size = fit_aspect_ratio(self.mesh_preview_container_size, self.mesh_aspect_ratio_preset);
            self.preview_size = fitted_size;
            let _ = self.ensure_preview_image_cache(fitted_size, cx);
            cx.notify();
        }
    }

    pub(super) fn apply_picker_color(&mut self, cx: &mut Context<Self>) {
        if !self.color_picker_open {
            return;
        }
        let color = self.color_picker.read(cx).color();
        match self.selected_tab {
            BuilderTab::Gradients => {
                let Some(thumb_id) = self.selected_stop else {
                    return;
                };
                self.stop_colors.insert(thumb_id, color);
                self.rebuild_stops(cx);
            }
            BuilderTab::Mesh | BuilderTab::Freeform => match self.mesh_color_target {
                MeshColorTarget::Point(point_index) => {
                    if let Some(point) = self.mesh_points.get_mut(point_index) {
                        point.color = color;
                        let _ = self.ensure_preview_image_cache(self.preview_size, cx);
                        cx.notify();
                    }
                }
                MeshColorTarget::Background => {
                    self.mesh_background = color;
                    let _ = self.ensure_preview_image_cache(self.preview_size, cx);
                    cx.notify();
                }
            },
        }
    }

    pub(super) fn open_color_picker_for_stop(&mut self, thumb_id: ThumbId, cx: &mut Context<Self>) {
        self.selected_stop = Some(thumb_id);
        let color = self.selected_color(cx);
        self.color_picker.update(cx, |picker, cx| picker.set_color(color, cx));
        self.color_picker_open = true;
        cx.notify();
    }

    pub(super) fn open_color_picker_for_mesh_point(&mut self, point_index: usize, cx: &mut Context<Self>) {
        self.selected_mesh_point = Some(point_index);
        self.sync_point_spread(cx);
        self.mesh_color_target = MeshColorTarget::Point(point_index);
        let color = self.selected_mesh_color();
        self.color_picker.update(cx, |picker, cx| picker.set_color(color, cx));
        self.color_picker_open = true;
        cx.notify();
    }

    pub(super) fn open_color_picker_for_mesh_background(&mut self, cx: &mut Context<Self>) {
        self.mesh_color_target = MeshColorTarget::Background;
        self.color_picker.update(cx, |picker, cx| picker.set_color(self.mesh_background, cx));
        self.color_picker_open = true;
        cx.notify();
    }

    pub(super) fn reset_mesh_state(&mut self, cx: &mut Context<Self>) {
        self.preview_image_cache = None;
        self.mesh_points = if self.selected_tab == BuilderTab::Freeform {
            super::freeform::default_points()
        } else {
            default_mesh_points(self.mesh_grid_preset)
        };
        self.mesh_point_ids = (self.next_point_id..self.next_point_id + self.mesh_points.len() as u64).collect();
        self.next_point_id += self.mesh_points.len() as u64;
        if self.selected_tab == BuilderTab::Freeform {
            self.point_spreads = vec![100.0; self.mesh_points.len()];
        }
        self.sync_freeform_buttons(cx);
        self.mesh_background = default_mesh_background();
        self.selected_mesh_point = None;
        self.sync_point_spread(cx);
        self.mesh_color_target = MeshColorTarget::Background;
        self.active_mesh_drag = None;
        self.color_picker_open = false;
        let _ = self.ensure_preview_image_cache(self.preview_size, cx);
        cx.notify();
    }

    pub(super) fn select_stop_row(&mut self, thumb_id: ThumbId, cx: &mut Context<Self>) {
        self.selected_stop = Some(thumb_id);
        self.gradient_stops.update(cx, |slider, cx| {
            slider.select_thumb_id(thumb_id, cx);
        });
        cx.notify();
    }

    pub(super) fn sync_stop_buttons(&mut self, cx: &mut Context<Self>) {
        let thumbs = self.gradient_stops.read(cx).thumbs().to_vec();
        let thumb_ids = thumbs.iter().map(|thumb| thumb.id).collect::<Vec<_>>();
        self.stop_delete_buttons.retain(|thumb_id, _| thumb_ids.contains(thumb_id));
        self.stop_swatch_bounds.retain(|thumb_id, _| thumb_ids.contains(thumb_id));

        let can_remove = thumb_ids.len() > 2;
        for thumb_id in thumb_ids {
            if let Some(button) = self.stop_delete_buttons.get(&thumb_id) {
                button.update(cx, |button, cx| button.set_enabled(can_remove, cx));
                continue;
            }

            let button = crate::theme::icon_button(
                format!("color-viz-gradient-stop-delete-{}", thumb_id.as_u64()),
                LucideIcon::Trash,
            )
            .look(self.look.as_ref())
            .content_only()
            .radius(radix::Radius::Medium)
            .size(radix::ButtonSize::One)
            .spawn(cx)
            .tooltip(Tooltip::new("Remove color stop"), cx);
            button.update(cx, |button, cx| button.set_enabled(can_remove, cx));
            self._subscriptions.push(cx.subscribe(&button, move |this, _, event: &ButtonEvent, cx| {
                if matches!(event, ButtonEvent::Click) {
                    this.handle_delete_stop(thumb_id, cx);
                }
            }));
            self.stop_delete_buttons.insert(thumb_id, button);
        }
    }

    pub(super) fn hydrate_thumb_color(&mut self, thumb_id: ThumbId, cx: &Context<Self>) {
        if self.stop_colors.contains_key(&thumb_id) {
            return;
        }

        let slider = self.gradient_stops.read(cx);
        let Some(thumb) = slider.thumbs().iter().find(|thumb| thumb.id == thumb_id) else {
            return;
        };
        let fallback_stops = slider
            .thumbs()
            .iter()
            .filter(|candidate| candidate.id != thumb_id)
            .map(|candidate| {
                let color = self
                    .stop_colors
                    .get(&candidate.id)
                    .copied()
                    .or(candidate.preview)
                    .unwrap_or_else(|| gpui::hsla(0.0, 0.0, 0.5, 1.0));
                (candidate.position, color)
            })
            .collect::<Vec<_>>();

        let color = if fallback_stops.is_empty() {
            thumb.preview.unwrap_or_else(|| gpui::hsla(0.0, 0.0, 0.5, 1.0))
        } else {
            color_at_position(&sorted_stops(&fallback_stops), thumb.position)
        };
        self.stop_colors.insert(thumb_id, color);
    }

    pub(super) fn new_stop_position(&self, cx: &Context<Self>) -> f32 {
        let stops = self.ordered_stop_rows(cx);
        if stops.is_empty() {
            return 0.5;
        }

        let Some(selected_stop) = self.selected_stop else {
            return 0.5;
        };
        let Some(index) = stops.iter().position(|(thumb_id, _, _)| *thumb_id == selected_stop) else {
            return 0.5;
        };

        if index + 1 < stops.len() {
            return (stops[index].1 + stops[index + 1].1) * 0.5;
        }
        if index > 0 {
            return (stops[index - 1].1 + stops[index].1) * 0.5;
        }

        (stops[index].1 + 0.5).min(1.0)
    }

    pub(super) fn rebuild_stops(&mut self, cx: &mut Context<Self>) {
        self.sync_stop_buttons(cx);
        let slider = self.gradient_stops.read(cx);
        let stops = sorted_stops(
            &slider
                .thumbs()
                .iter()
                .map(|thumb| {
                    let color = self
                        .stop_colors
                        .get(&thumb.id)
                        .copied()
                        .or(thumb.preview)
                        .unwrap_or_else(|| gpui::hsla(0.0, 0.0, 0.5, 1.0));
                    (thumb.position, color)
                })
                .collect::<Vec<_>>(),
        );
        let stops = stops
            .into_iter()
            .map(|(position, color)| GradientStop { position, color: gpui_luma::color::gpui_bridge::from_hsla(color) })
            .collect();
        let mut track_context = self.track_context.clone();
        track_context.theme_is_dark = matches!(self.look.mode(), ThemeMode::Dark);
        let Ok(delegate) = GradientDelegate::new(stops, gpui_luma::color::GamutMapping::CssLocalMinde) else {
            return;
        };
        update_domain_delegate(&self.domain_renderer, Arc::new(delegate), track_context);
        let _ = self.ensure_preview_image_cache(self.preview_size, cx);
        refresh_color_slider(&self.gradient_stops, cx);
        cx.notify();
    }

    pub(super) fn selected_color(&self, cx: &Context<Self>) -> gpui::Hsla {
        let Some(thumb_id) = self.selected_stop else {
            return gpui::hsla(0.0, 0.0, 0.5, 1.0);
        };
        self.stop_colors
            .get(&thumb_id)
            .copied()
            .or_else(|| {
                self.gradient_stops
                    .read(cx)
                    .thumbs()
                    .iter()
                    .find(|thumb| thumb.id == thumb_id)
                    .and_then(|thumb| thumb.preview)
            })
            .unwrap_or_else(|| gpui::hsla(0.0, 0.0, 0.5, 1.0))
    }

    pub(super) fn selected_mesh_color(&self) -> gpui::Hsla {
        match self.mesh_color_target {
            MeshColorTarget::Point(point_index) => self
                .mesh_points
                .get(point_index)
                .map(|point| point.color)
                .unwrap_or_else(|| gpui::hsla(0.0, 0.0, 0.5, 1.0)),
            MeshColorTarget::Background => self.mesh_background,
        }
    }

    pub(super) fn handle_stop_swatch_bounds(&mut self, thumb_id: ThumbId, bounds: &Bounds<Pixels>) {
        self.stop_swatch_bounds.insert(thumb_id, *bounds);
    }

    pub(super) fn handle_mesh_swatch_bounds(&mut self, point_index: usize, bounds: &Bounds<Pixels>) {
        self.mesh_swatch_bounds.insert(point_index, *bounds);
    }

    pub(super) fn handle_mesh_background_swatch_bounds(&mut self, bounds: &Bounds<Pixels>) {
        self.mesh_background_swatch_bounds = Some(*bounds);
    }

    pub(super) fn preview_stops(&self, cx: &Context<Self>) -> Vec<(f32, gpui::Hsla)> {
        let slider = self.gradient_stops.read(cx);
        let stops = slider
            .thumbs()
            .iter()
            .map(|thumb| {
                let color = self
                    .stop_colors
                    .get(&thumb.id)
                    .copied()
                    .or(thumb.preview)
                    .unwrap_or_else(|| gpui::hsla(0.0, 0.0, 0.5, 1.0));
                (thumb.position, color)
            })
            .collect::<Vec<_>>();
        sorted_stops(&stops)
    }

    pub(super) fn ordered_stop_rows(&self, cx: &Context<Self>) -> Vec<(ThumbId, f32, gpui::Hsla)> {
        let mut stops = self
            .gradient_stops
            .read(cx)
            .thumbs()
            .iter()
            .map(|thumb| {
                let color = self
                    .stop_colors
                    .get(&thumb.id)
                    .copied()
                    .or(thumb.preview)
                    .unwrap_or_else(|| gpui::hsla(0.0, 0.0, 0.5, 1.0));
                (thumb.id, thumb.position, color)
            })
            .collect::<Vec<_>>();
        stops.sort_by(|left, right| left.1.total_cmp(&right.1));
        stops
    }

    pub(super) fn gradient_spec(&self, cx: &Context<Self>) -> String {
        format_css_gradient(self.gradient_type, &self.preview_stops(cx), self.rotation_deg)
    }

    pub(super) fn effective_preview_renderer(&self) -> PreviewRenderer {
        if !self.image_adjustments.is_neutral() {
            return PreviewRenderer::RenderImageSync;
        }
        match (self.gradient_type, self.preview_renderer) {
            (GradientType::Linear, renderer) => renderer,
            (_, PreviewRenderer::RenderImageAsync) => PreviewRenderer::RenderImageAsync,
            _ => PreviewRenderer::RenderImageSync,
        }
    }

    pub(super) fn uses_render_preview(&self, cx: &Context<Self>) -> bool {
        if self.selected_tab != BuilderTab::Gradients || !self.image_adjustments.is_neutral() {
            return true;
        }
        let stop_count = self.preview_stops(cx).len();
        self.gradient_type != GradientType::Linear
            || (matches!(
                self.effective_preview_renderer(),
                PreviewRenderer::RenderImageSync | PreviewRenderer::RenderImageAsync
            ) && stop_count > 1)
    }

    pub(super) fn sync_preview_strategy(&mut self, stop_count: usize) {
        if self.selected_tab != BuilderTab::Gradients {
            self.active_preview_strategy = "render (sync)";
            return;
        }
        self.active_preview_strategy = match (self.gradient_type, stop_count, self.effective_preview_renderer()) {
            (_, 0, _) => "empty",
            (_, 1, _) => "solid",
            (GradientType::Linear, _, PreviewRenderer::Quads) => "quads",
            (_, _, PreviewRenderer::RenderImageAsync) => "render (async)",
            _ => "render (sync)",
        };
        if !self.active_preview_strategy.starts_with("render") {
            self.last_render_ms = None;
        }
    }

    pub(super) fn handle_preview_bounds(&mut self, bounds: Bounds<Pixels>, cx: &mut Context<Self>) -> bool {
        let mut refreshed = false;
        if self.preview_size != bounds.size {
            self.preview_size = bounds.size;
        }
        if self.selected_tab != BuilderTab::Gradients {
            self.mesh_preview_bounds = Some(bounds);
        }

        if self.uses_render_preview(cx) && self.ensure_preview_image_cache(bounds.size, cx) {
            refreshed = true;
            cx.notify();
        }
        refreshed
    }

    pub(super) fn handle_mesh_preview_container_bounds(
        &mut self,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut refreshed = false;
        let content_size = mesh_preview_content_size(bounds.size);
        if self.mesh_preview_container_size != content_size {
            self.mesh_preview_container_size = content_size;
            refreshed = true;
        }

        let fitted_size = fit_aspect_ratio(content_size, self.mesh_aspect_ratio_preset);
        if self.preview_size != fitted_size {
            self.preview_size = fitted_size;
            refreshed = true;
        }

        if self.uses_render_preview(cx) && self.ensure_preview_image_cache(fitted_size, cx) {
            refreshed = true;
        }
        if refreshed {
            cx.notify();
        }
        refreshed
    }

    pub(super) fn ensure_preview_image_cache(&mut self, preview_size: Size<Pixels>, cx: &mut Context<Self>) -> bool {
        if !self.uses_render_preview(cx) || preview_size.width <= px(0.0) || preview_size.height <= px(0.0) {
            return false;
        }

        let key = match self.selected_tab {
            BuilderTab::Gradients => {
                let stops = self.preview_stops(cx);
                preview_gradient_cache_key(self.gradient_type, preview_size, self.rotation_deg, &stops)
            }
            BuilderTab::Mesh | BuilderTab::Freeform => {
                preview_mesh_cache_key(preview_size, &self.mesh_points, self.mesh_background)
            }
        };
        if let Some((cached_key, _)) = &self.preview_image_cache
            && *cached_key == key
        {
            return false;
        }

        self.render_task = None;

        if self.selected_tab != BuilderTab::Gradients
            || self.effective_preview_renderer() == PreviewRenderer::RenderImageSync
        {
            let started = Instant::now();
            let image = match self.selected_tab {
                BuilderTab::Gradients => {
                    let stops = self.preview_stops(cx);
                    rasterize_gradient_preview(self.gradient_type, preview_size, &stops, self.rotation_deg)
                }
                BuilderTab::Freeform => super::super::paint::rasterize_freeform_preview(
                    preview_size,
                    &self.mesh_points,
                    self.mesh_background,
                    self.spread_percent,
                    self.freeform_hsl,
                    &super::super::paint::FieldShape {
                        point_spreads: &self.point_spreads,
                        softness_percent: self.softness_percent,
                    },
                ),
                BuilderTab::Mesh => rasterize_mesh_gradient_preview(
                    preview_size,
                    &self.mesh_points,
                    self.mesh_background,
                    self.mesh_controls_visible,
                ),
            };
            let Some(image) = image.and_then(|image| self.image_adjustments.apply(image)) else {
                return false;
            };
            self.preview_image_cache = Some((key, image));
            self.last_render_ms = Some(started.elapsed().as_secs_f32() * 1000.0);
            true
        } else {
            let gradient_type = self.gradient_type;
            let rotation_deg = self.rotation_deg;
            let stops = self.preview_stops(cx);
            self.render_task = Some(cx.spawn(async move |this, cx| {
                let started = Instant::now();
                let image = cx
                    .background_executor()
                    .spawn(async move { rasterize_gradient_preview(gradient_type, preview_size, &stops, rotation_deg) })
                    .await;

                let _ = this.update(cx, |this, cx| {
                    if let Some(image) = image {
                        this.preview_image_cache = Some((key, image));
                        this.last_render_ms = Some(started.elapsed().as_secs_f32() * 1000.0);
                        cx.notify();
                    }
                });
            }));
            false
        }
    }

    pub(super) fn export_png(&mut self, cx: &mut Context<Self>) {
        // Export the artwork, excluding editor handles and mesh guide lines.
        let image = match self.selected_tab {
            BuilderTab::Gradients => rasterize_gradient_preview(
                self.gradient_type,
                self.preview_size,
                &self.preview_stops(cx),
                self.rotation_deg,
            ),
            BuilderTab::Mesh => {
                rasterize_mesh_gradient_preview(self.preview_size, &self.mesh_points, self.mesh_background, false)
            }
            BuilderTab::Freeform => super::super::paint::rasterize_freeform_preview(
                self.preview_size,
                &self.mesh_points,
                self.mesh_background,
                self.spread_percent,
                self.freeform_hsl,
                &super::super::paint::FieldShape {
                    point_spreads: &self.point_spreads,
                    softness_percent: self.softness_percent,
                },
            ),
        }
        .and_then(|image| self.image_adjustments.apply(image));
        let Some(image) = image else {
            self.export_status = Some("Preview not ready to export".into());
            cx.notify();
            return;
        };
        let directory = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
        let path = cx.prompt_for_new_path(&directory, Some("gradient.png"));
        self.export_status = Some("Choose a location for your PNG".into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match path.await {
                Ok(Ok(Some(path))) => {
                    cx.background_executor()
                        .spawn(async move {
                            super::super::paint::save_preview_png(&image, &path)
                                .map(|_| format!("Saved {}", path.file_name().unwrap_or_default().to_string_lossy()))
                        })
                        .await
                }
                Ok(Ok(None)) => Ok("Export canceled".into()),
                Ok(Err(error)) => Err(error),
                Err(error) => Err(anyhow::anyhow!(error)),
            };
            let _ = this.update(cx, |this, cx| {
                this.export_status = Some(result.unwrap_or_else(|error| format!("Export failed: {error}")));
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn cached_preview_image(&self) -> Option<Arc<RenderImage>> {
        self.preview_image_cache.as_ref().map(|(_, image)| image.clone())
    }

    pub(super) fn points_visible(&self) -> bool {
        if self.selected_tab == BuilderTab::Freeform {
            self.freeform_controls_visible
        } else {
            self.mesh_controls_visible
        }
    }

    fn sync_visibility_button(&self, cx: &mut Context<Self>) {
        self.visibility_button.update(cx, |button, cx| button.set_data(self.points_visible(), cx));
    }

    pub(super) fn copy_gradient_css(&self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(self.gradient_spec(cx)));
    }

    pub(super) fn toggle_mesh_controls(&mut self, cx: &mut Context<Self>) {
        if self.selected_tab == BuilderTab::Freeform {
            self.freeform_controls_visible = !self.freeform_controls_visible;
        } else if self.selected_tab == BuilderTab::Mesh {
            self.mesh_controls_visible = !self.mesh_controls_visible;
        } else {
            return;
        }
        self.sync_visibility_button(cx);
        self.active_mesh_drag = None;
        self.preview_image_cache = None;
        let _ = self.ensure_preview_image_cache(self.preview_size, cx);
        cx.notify();
    }

    pub(super) fn begin_mesh_drag(&mut self, point_index: usize, cx: &mut Context<Self>) {
        self.selected_mesh_point = Some(point_index);
        self.sync_point_spread(cx);
        self.mesh_color_target = MeshColorTarget::Point(point_index);
        self.active_mesh_drag = Some(point_index);
        self.color_picker_open = false;
        cx.notify();
    }

    pub(super) fn handle_mesh_drag_move(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        let Some(point_index) = self.active_mesh_drag else {
            return;
        };
        let Some(bounds) = self.mesh_preview_bounds else {
            return;
        };
        let width = bounds.size.width.as_f32();
        let height = bounds.size.height.as_f32();
        if width <= 0.0 || height <= 0.0 {
            return;
        }

        let u = ((position.x - bounds.origin.x).as_f32() / width).clamp(0.0, 1.0);
        let v = ((position.y - bounds.origin.y).as_f32() / height).clamp(0.0, 1.0);
        let (u, v) = self.constrained_mesh_position(point_index, u, v);
        let Some(point) = self.mesh_points.get_mut(point_index) else {
            return;
        };
        if (point.u - u).abs() <= f32::EPSILON && (point.v - v).abs() <= f32::EPSILON {
            return;
        }
        point.u = u;
        point.v = v;
        let _ = self.ensure_preview_image_cache(self.preview_size, cx);
        cx.notify();
    }

    pub(super) fn finish_mesh_drag(&mut self, cx: &mut Context<Self>) {
        if self.active_mesh_drag.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn constrained_mesh_position(&self, point_index: usize, proposed_u: f32, proposed_v: f32) -> (f32, f32) {
        if self.selected_tab == BuilderTab::Freeform {
            return (proposed_u.clamp(0.0, 1.0), proposed_v.clamp(0.0, 1.0));
        }
        let Some(point) = self.mesh_points.get(point_index).copied() else {
            return (proposed_u.clamp(0.0, 1.0), proposed_v.clamp(0.0, 1.0));
        };
        let Some((rows, cols)) = mesh_dimensions(&self.mesh_points) else {
            return (proposed_u.clamp(0.0, 1.0), proposed_v.clamp(0.0, 1.0));
        };

        let row = point.row as usize;
        let col = point.col as usize;
        let mut u = proposed_u.clamp(0.0, 1.0);
        let mut v = proposed_v.clamp(0.0, 1.0);

        if col == 0 {
            let right = self.mesh_points[mesh_point_index(row, col + 1, cols)].u - MESH_POINT_GAP;
            u = u.clamp(0.0, right.max(0.0));
        } else if col + 1 == cols {
            let left = self.mesh_points[mesh_point_index(row, col - 1, cols)].u + MESH_POINT_GAP;
            u = u.clamp(left.min(1.0), 1.0);
        } else {
            let left = self.mesh_points[mesh_point_index(row, col - 1, cols)].u + MESH_POINT_GAP;
            let right = self.mesh_points[mesh_point_index(row, col + 1, cols)].u - MESH_POINT_GAP;
            u = u.clamp(left.min(right), left.max(right));
        }

        if row == 0 {
            let bottom = self.mesh_points[mesh_point_index(row + 1, col, cols)].v - MESH_POINT_GAP;
            v = v.clamp(0.0, bottom.max(0.0));
        } else if row + 1 == rows {
            let top = self.mesh_points[mesh_point_index(row - 1, col, cols)].v + MESH_POINT_GAP;
            v = v.clamp(top.min(1.0), 1.0);
        } else {
            let top = self.mesh_points[mesh_point_index(row - 1, col, cols)].v + MESH_POINT_GAP;
            let bottom = self.mesh_points[mesh_point_index(row + 1, col, cols)].v - MESH_POINT_GAP;
            v = v.clamp(top.min(bottom), top.max(bottom));
        }

        (u, v)
    }
}
