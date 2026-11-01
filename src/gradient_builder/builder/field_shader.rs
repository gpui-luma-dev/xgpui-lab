//! Adapter between editor state and the app's promotable GPU field primitive.
use super::*;
use super::super::color_field::{ColorField, Output};
use gpui::{AnyElement, IntoElement, div, prelude::*};
#[derive(Default)]
pub(super) struct Cache {
    #[cfg(target_os = "macos")]
    pipeline: Arc<super::super::color_field::PipelineCache>,
    #[cfg(target_os = "macos")]
    cached: Option<(ColorField, Output, Arc<super::super::color_field::Paint>)>,
}
impl Cache {
    pub(super) fn element(&mut self, field: ColorField, wide: bool) -> AnyElement {
        #[cfg(target_os = "macos")]
        {
            use gpui::{Corners, CustomGeometry, canvas};
            let output = if wide { Output::DisplayP3 } else { Output::Srgb };
            if self.cached.as_ref().is_none_or(|(old, mode, _)| old != &field || *mode != output) {
                let paint = match super::super::color_field::Paint::new(field.clone(), output, self.pipeline.clone()) {
                    Ok(paint) => Arc::new(paint),
                    Err(error) => return div().p_3().child(error.to_string()).into_any_element(),
                };
                self.cached = Some((field, output, paint));
            }
            let paint = self.cached.as_ref().unwrap().2.clone();
            return canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    window.paint_custom(
                        CustomGeometry::RoundedRect { bounds, corner_radii: Corners::default() },
                        paint.clone(),
                    );
                },
            )
            .size_full()
            .into_any_element();
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (field, wide);
            div().p_3().child("GPU field prototype requires native Metal; select CPU.").into_any_element()
        }
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    #[test]
    fn renderer_and_mixing_switches_keep_the_same_edited_field() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.handle_top_tabs_event(
                &TabsEvent::Activate { tab_id: "freeform".into(), label: "Freeform".into() },
                cx,
            );
            view.point_transforms[0] = [75., 0.4];
            view.point_spreads[0] = 65.;
        });
        let (renderer, mixing) =
            view.update(cx, |view, _| (view.freeform_renderer_selector.clone(), view.freeform_mixing_selector.clone()));
        let original = view.update(cx, |view, _| view.mesh_points.clone());
        for method in ["normalized", "ordered"] {
            mixing.update(cx, |_, cx| cx.emit(SelectorEvent::Change { item_id: method.into(), label: method.into() }));
            cx.run_until_parked();
            for id in ["gpu", "cpu"] {
                renderer.update(cx, |_, cx| cx.emit(SelectorEvent::Change { item_id: id.into(), label: id.into() }));
                cx.run_until_parked();
                view.update(cx, |view, cx| {
                    assert_eq!(view.mesh_points, original);
                    assert_eq!(view.point_transforms[0], [75., 0.4]);
                    assert_eq!(view.point_spreads[0], 65.);
                    assert_eq!(view.uses_render_preview(cx), id == "cpu");
                    assert_eq!(view.adjustments_supported(), id == "cpu");
                    assert_eq!(
                        view.freeform_mixing == super::super::super::color_field::Mixing::Normalized,
                        method == "normalized"
                    );
                });
            }
        }
    }
}
