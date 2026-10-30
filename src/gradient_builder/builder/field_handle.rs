//! On-canvas rotation and area-preserving stretch for the selected freeform field.
use super::*;
use gpui::{AnyElement, IntoElement, MouseButton, PathBuilder, Window, canvas, div, point, prelude::*};
const ARM: f32 = 0.18;

pub(super) fn endpoint(p: MeshPoint, transform: [f32; 2]) -> [f32; 2] {
    let angle = transform[0].to_radians();
    let length = ARM * transform[1].sqrt();
    [p.u + angle.cos() * length, p.v + angle.sin() * length]
}
pub(super) fn from_offset(dx: f32, dy: f32, previous_angle: f32) -> [f32; 2] {
    let distance = dx.hypot(dy);
    let angle = if distance < 0.0001 { previous_angle } else { dy.atan2(dx).to_degrees().rem_euclid(360.) };
    [angle, (distance / ARM).powi(2).clamp(1. / 9., 9.)]
}
impl GradientBuilder {
    pub(super) fn drag_field_handle(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        let Some(index) = self.active_field_drag else { return; };
        let Some(bounds) = self.mesh_preview_bounds else { return; };
        if self.selected_tab != BuilderTab::Freeform || bounds.size.width <= px(0.) || bounds.size.height <= px(0.) { return; }
        let Some(p) = self.mesh_points.get(index) else { return; };
        let dx = (position.x - bounds.origin.x).as_f32() / bounds.size.width.as_f32() - p.u;
        let dy = (position.y - bounds.origin.y).as_f32() / bounds.size.height.as_f32() - p.v;
        let Some(transform) = self.point_transforms.get_mut(index) else { return; };
        *transform = from_offset(dx, dy, transform[0]);
        self.preview_image_cache = None;
        let _ = self.ensure_preview_image_cache(self.preview_size, cx);
        cx.notify();
    }
    pub(super) fn field_handle(&self, extent: Size<Pixels>, cx: &Context<Self>) -> Option<AnyElement> {
        if self.selected_tab != BuilderTab::Freeform || !self.points_visible() { return None; }
        let index = self.selected_mesh_point?;
        let p = *self.mesh_points.get(index)?;
        let transform = self.point_transforms.get(index).copied().unwrap_or([0., 1.]);
        let end = endpoint(p, transform);
        let builder = cx.entity();
        Some(div().debug_selector(|| "freeform-angle-stretch-vector".into()).absolute().inset_0()
            .child(canvas(|_, _, _| (), move |bounds, _, window: &mut Window, _| {
                let start = bounds.origin + point(bounds.size.width * p.u, bounds.size.height * p.v);
                let finish = bounds.origin + point(bounds.size.width * end[0], bounds.size.height * end[1]);
                let dx = (finish.x - start.x).as_f32();
                let dy = (finish.y - start.y).as_f32();
                let angle = dy.atan2(dx);
                for (width, color) in [(4., gpui::hsla(0., 0., 0., 0.7)), (2., gpui::white())] {
                    let mut path = PathBuilder::stroke(px(width));
                    path.move_to(start);
                    path.line_to(finish);
                    for offset in [-0.5_f32, 0.5] {
                        path.move_to(finish - point(px(10. * (angle + offset).cos()), px(10. * (angle + offset).sin())));
                        path.line_to(finish);
                    }
                    if let Ok(path) = path.build() { window.paint_path(path, color); }
                }
            }).size_full())
            .child(div().id("freeform-angle-stretch-handle")
                .debug_selector(|| "freeform-angle-stretch-handle".into())
                .absolute().left(extent.width * end[0]).top(extent.height * end[1])
                .ml(px(-12.)).mt(px(-12.)).size(px(24.)).rounded(px(12.))
                .bg(gpui::hsla(0., 0., 0., 0.7)).border_2().border_color(gpui::white()).cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    builder.update(cx, |this, cx| {
                        this.active_mesh_drag = None;
                        this.active_field_drag = Some(index);
                        this.selected_mesh_point = Some(index);
                        this.mesh_color_target = MeshColorTarget::Point(index);
                        this.sync_point_spread(cx);
                        this.color_picker_open = false;
                        cx.notify();
                    });
                })
                .on_click(|_, _, cx| cx.stop_propagation()))
            .into_any_element())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_drag_roundtrips_rotation_and_stretch() {
        let p = MeshPoint { row: 0, col: 0, u: 0.4, v: 0.5, color: gpui::white() };
        for angle in [0., 45., 90., 180., 270., 359.] {
            for stretch in [1. / 9., 0.25, 0.5, 1., 2., 9.] {
                let end = endpoint(p, [angle, stretch]);
                let actual = from_offset(end[0] - p.u, end[1] - p.v, 0.);
                let error = (actual[0] - angle + 180.).rem_euclid(360.) - 180.;
                assert!(error.abs() < 0.001);
                assert!((actual[1] - stretch).abs() < 0.001);
            }
        }
        assert_eq!(from_offset(0., 0., 75.), [75., 1. / 9.]);
    }
}

#[cfg(all(test, feature = "test-support"))]
mod interaction_tests {
    use super::*;
    #[test]
    fn dragging_endpoint_rotates_field_without_moving_center() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.handle_top_tabs_event(&TabsEvent::Activate { tab_id: "freeform".into(), label: "Freeform".into() }, cx);
            view.begin_mesh_drag(1, cx);
            view.finish_mesh_drag(cx);
        });
        cx.run_until_parked();
        let endpoint = cx.debug_bounds("freeform-angle-stretch-handle").expect("selected endpoint").center();
        let (center, bounds) = view.update(cx, |view, _| (view.mesh_points[1], view.mesh_preview_bounds.unwrap()));
        cx.simulate_mouse_down(endpoint, MouseButton::Left, Default::default());
        cx.run_until_parked();
        view.update(cx, |view, _| {
            assert_eq!(view.active_field_drag, Some(1));
            assert_eq!(view.selected_mesh_point, Some(1));
        });
        let target = bounds.origin + point(bounds.size.width * (center.u + 0.25), bounds.size.height * (center.v + 0.25));
        cx.simulate_mouse_move(target, MouseButton::Left, Default::default());
        cx.run_until_parked();
        view.update(cx, |view, _| {
            assert_eq!(view.selected_mesh_point, Some(1), "selection during drag");
            assert_eq!(view.active_field_drag, Some(1), "active endpoint during drag");
            assert!(view.freeform_controls_visible);
        });
        assert!(cx.debug_bounds("freeform-angle-stretch-handle").is_some(), "endpoint remains visible during drag");
        assert!(cx.debug_bounds("freeform-angle-stretch-vector").is_some(), "vector remains visible during drag");
        view.update(cx, |view, _| {
            assert_eq!(view.selected_mesh_point, Some(1));
            assert_eq!(view.active_field_drag, Some(1));
        });
        cx.simulate_mouse_up(target, MouseButton::Left, Default::default());
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            assert_eq!(view.mesh_points[1], center);
            assert!((view.point_transforms[1][0] - 45.).abs() < 0.01);
            assert!(view.point_transforms[1][1] > 3.);
            assert_eq!(view.active_field_drag, None);
            let saved = view.point_transforms.clone();
            view.handle_top_tabs_event(&TabsEvent::Activate { tab_id: "mesh".into(), label: "Mesh".into() }, cx);
            view.handle_top_tabs_event(&TabsEvent::Activate { tab_id: "freeform".into(), label: "Freeform".into() }, cx);
            assert_eq!(view.point_transforms, saved);
            view.deselect_freeform_point(cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("freeform-angle-stretch-handle").is_none());
    }
}
