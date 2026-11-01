use super::*;
use gpui::{AnyElement, Corners, CustomGeometry, ImageSource, IntoElement, canvas, div, img, prelude::*};
use xgpui_shader::MeshGradient;

#[derive(Default)]
pub(super) struct Cache {
    paint: Option<Arc<MeshGradient>>,
    #[cfg(target_os = "macos")]
    wide: Option<(Arc<MeshGradient>, Arc<super::mesh_p3::Paint>)>,
    #[cfg(target_os = "macos")]
    pipeline: Arc<super::mesh_p3::Pipeline>,
    guides: Option<(Vec<MeshPoint>, Size<Pixels>, Option<Arc<RenderImage>>)>,
}
impl Cache {
    pub(super) fn element(&mut self, points: &[MeshPoint], extent: Size<Pixels>, background: gpui::Hsla, guides: bool, wide: bool) -> AnyElement {
        let result = super::super::paint::shader_mesh_grid(points).and_then(|grid| {
            if self.paint.as_ref().is_none_or(|p| p.grid() != &grid) {
                let next = match &self.paint {
                    Some(old) => old.with_grid(grid),
                    None => MeshGradient::new(grid, 32),
                }?;
                self.paint = Some(Arc::new(next));
            }
            Ok(self.paint.as_ref().unwrap().clone())
        });
        let paint = match result {
            Ok(paint) => paint,
            Err(error) => return div().p_3().child(error.to_string()).into_any_element(),
        };
        let mut content: Arc<dyn gpui::CustomContent> = paint.clone();
        #[cfg(target_os = "macos")]
        if wide {
            if self.wide.as_ref().is_none_or(|(old, _)| !Arc::ptr_eq(old, &paint)) {
                self.wide = Some((paint.clone(), Arc::new(super::mesh_p3::Paint::new(paint.clone(), self.pipeline.clone()))));
            }
            content = self.wide.as_ref().unwrap().1.clone();
        }
        #[cfg(not(target_os = "macos"))]
        let _ = wide;
        let guide_image = if guides {
            if self.guides.as_ref().is_none_or(|(old, size, _)| old != points || *size != extent) {
                self.guides = Some((points.to_vec(), extent, super::super::paint::rasterize_mesh_guides(extent, points)));
            }
            self.guides.as_ref().and_then(|(_, _, image)| image.clone())
        } else { None };
        div().size_full().relative().bg(background)
            .child(canvas(|_, _, _| (), move |bounds, _, window, _| {
                window.paint_custom(CustomGeometry::RoundedRect { bounds, corner_radii: Corners::default() }, content.clone());
            }).size_full())
            .when_some(guide_image, |this, image| this.child(img(ImageSource::Render(image)).absolute().inset_0().size_full()))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mesh_adapter_preserves_points_and_cache_reuses_unchanged_paint() {
        let points = default_mesh_points(MeshGridPreset::SampleThreeByFour);
        let mut shuffled = points.clone();
        shuffled.reverse();
        let grid = super::super::super::paint::shader_mesh_grid(&shuffled).unwrap();
        let (rows, cols) = super::super::super::paint::mesh_dimensions(&points).unwrap();
        assert_eq!((grid.height(), grid.width()), (rows as u32, cols as u32));
        for (index, p) in grid.points().iter().enumerate() {
            let original = points.iter().find(|p| p.row as usize == index / cols && p.col as usize == index % cols).unwrap();
            assert_eq!(p.position, [original.u, original.v]);
            let c = original.color.to_rgb();
            assert_eq!(p.color, [c.r, c.g, c.b]);
        }
        let mut cache = Cache::default();
        let extent = size(px(200.), px(150.));
        let _ = cache.element(&points, extent, gpui::black(), true, false);
        let first = cache.paint.as_ref().unwrap().clone();
        let _ = cache.element(&points, extent, gpui::white(), false, false);
        assert!(Arc::ptr_eq(&first, cache.paint.as_ref().unwrap()));
        let mut changed = points;
        changed[0].u += 0.01;
        let _ = cache.element(&changed, extent, gpui::white(), false, false);
        assert!(!Arc::ptr_eq(&first, cache.paint.as_ref().unwrap()));
        changed[0].color.a = 0.5;
        assert!(super::super::super::paint::shader_mesh_grid(&changed).is_err());
    }
    #[cfg(feature = "test-support")]
    #[test]
    fn mesh_renderer_switch_preserves_editing_state_and_freeform_cpu_path() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.handle_top_tabs_event(&TabsEvent::Activate { tab_id: "mesh".into(), label: "Mesh".into() }, cx);
        });
        let selector = view.update(cx, |view, _| view.mesh_renderer_selector.clone());
        let points = view.update(cx, |view, _| view.mesh_points.clone());
        for id in ["shader", "cpu", "shader"] {
            selector.update(cx, |_, cx| cx.emit(SelectorEvent::Change { item_id: id.into(), label: id.into() }));
            cx.run_until_parked();
            view.update(cx, |view, cx| {
                assert_eq!(view.mesh_points, points);
                assert_eq!(view.mesh_shader_enabled, id == "shader");
                assert_eq!(view.uses_render_preview(cx), id == "cpu");
                assert_eq!(view.adjustments_supported(), id == "cpu");
            });
        }
        view.update(cx, |view, cx| {
            view.handle_top_tabs_event(&TabsEvent::Activate { tab_id: "freeform".into(), label: "Freeform".into() }, cx);
            assert!(view.uses_render_preview(cx));
            assert!(view.adjustments_supported());
        });
    }
}
