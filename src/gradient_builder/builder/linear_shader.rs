//! Shader choices for the existing first-tab preview. No separate editor state.
use super::*;
use gpui::{AnyElement, CustomGeometry, Corners, IntoElement, Window, canvas, div, prelude::*};
use xgpui_shader::{
    Gradient, GradientStop,
    brush::{Brush, BrushStop},
};

#[derive(Clone, PartialEq)]
struct Key {
    renderer: PreviewRenderer,
    kind: GradientType,
    stops: Vec<(f32, gpui::Hsla)>,
    angle: f32,
    size: Size<Pixels>,
}
#[derive(Clone)]
pub(super) enum Material {
    Paint(Arc<Gradient>),
    Brush(Brush),
}
#[derive(Default)]
pub(super) struct Cache(Option<(Key, Result<Material, String>)>);
impl Cache {
    pub(super) fn get(
        &mut self,
        renderer: PreviewRenderer,
        kind: GradientType,
        stops: &[(f32, gpui::Hsla)],
        angle: f32,
        size: Size<Pixels>,
        neutral: bool,
    ) -> Result<Material, String> {
        if !neutral {
            return Err("Shader options do not support Saturation/Vibrance adjustments yet. Reset them or select an existing renderer.".into());
        }
        if size.width <= px(0.) || size.height <= px(0.) {
            return Err("Waiting for preview dimensions.".into());
        }
        let key = Key { renderer, kind, stops: stops.to_vec(), angle, size };
        if self.0.as_ref().is_none_or(|(old, _)| old != &key) {
            // Preserve the original editor's normalized-axis projection on non-square bounds.
            let theta = angle.to_radians();
            let linear_angle =
                (theta.sin() / size.width.as_f32()).atan2(theta.cos() / size.height.as_f32()).to_degrees();
            let rgba = |color: gpui::Hsla| {
                let c = color.to_rgb();
                [c.r, c.g, c.b, color.a]
            };
            let sorted = super::super::paint::sorted_stops(stops);
            let material = match renderer {
                PreviewRenderer::ShaderPaint => {
                    let stops = sorted.iter().map(|(p, c)| GradientStop::new(*p, rgba(*c))).collect();
                    let paint = match kind {
                        GradientType::Radial => Gradient::radial([0.5; 2], [std::f32::consts::FRAC_1_SQRT_2; 2], stops),
                        GradientType::Conic => Gradient::conic([0.5; 2], angle, stops),
                        GradientType::Linear => Gradient::linear(linear_angle, stops),
                    };
                    paint.map(|p| Material::Paint(Arc::new(p))).map_err(|e| e.to_string())
                }
                PreviewRenderer::ShaderBrush if kind == GradientType::Conic => {
                    Err("Conic gradients use Shader paint; xgpui's main-pass Brush has no conic constructor.".into())
                }
                PreviewRenderer::ShaderBrush if stops.len() > 3 => {
                    Err("Shader brush supports at most 3 stops: its 128-byte per-draw payload (for WebGL2 portability) uses 48 bytes for three RGBA colors and 80 bytes for stop positions, geometry, corner radii, border widths, and options. This is a data-layout limit, not a GPU gradient limit. Select Shader paint for more stops.".into())
                }
                PreviewRenderer::ShaderBrush => {
                    let stops = sorted.iter().map(|(p, c)| BrushStop::new(*p, rgba(*c))).collect();
                    let brush = match kind {
                        GradientType::Radial => Brush::radial([0.5; 2], [std::f32::consts::FRAC_1_SQRT_2; 2], stops),
                        GradientType::Linear => Brush::linear(linear_angle, stops),
                        GradientType::Conic => unreachable!("conic brushes are rejected above"),
                    };
                    brush.map(Material::Brush).map_err(|e| e.to_string())
                }
                _ => Err("Select a shader renderer.".into()),
            };
            self.0 = Some((key, material));
        }
        self.0.as_ref().unwrap().1.clone()
    }
}
pub(super) fn element(material: Result<Material, String>) -> AnyElement {
    match material {
        Ok(material) => canvas(
            |_, _, _| (),
            move |bounds, _, window: &mut Window, _| match &material {
                Material::Paint(content) => window.paint_custom(
                    CustomGeometry::RoundedRect { bounds, corner_radii: Corners::default() },
                    content.clone(),
                ),
                Material::Brush(brush) => {
                    if let Err(error) = brush.paint_background(window, bounds, Corners::default()) {
                        tracing::error!("Gradient shader brush: {error:#}");
                    }
                }
            },
        )
        .size_full()
        .into_any_element(),
        Err(error) => div().p_3().child(error).into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linear_shader_choices_cache_input_and_report_limits() {
        let mut cache = Cache::default();
        let mut stops = vec![(0., gpui::hsla(0., 1., 0.5, 1.)), (1., gpui::hsla(0.6, 1., 0.5, 0.5))];
        let extent = size(px(160.), px(90.));
        let Material::Paint(first) =
            cache.get(PreviewRenderer::ShaderPaint, GradientType::Linear, &stops, 37., extent, true).unwrap()
        else {
            panic!("paint expected")
        };
        let Material::Paint(second) =
            cache.get(PreviewRenderer::ShaderPaint, GradientType::Linear, &stops, 37., extent, true).unwrap()
        else {
            panic!("paint expected")
        };
        assert!(Arc::ptr_eq(&first, &second));
        let Material::Paint(radial) =
            cache.get(PreviewRenderer::ShaderPaint, GradientType::Radial, &stops, 37., extent, true).unwrap()
        else {
            panic!("paint expected")
        };
        assert!(!Arc::ptr_eq(&first, &radial), "switching types must replace the cached geometry");
        assert_eq!(
            radial.kind(),
            xgpui_shader::GradientKind::Radial { center: [0.5; 2], radius: [std::f32::consts::FRAC_1_SQRT_2; 2] }
        );
        assert!(cache.get(PreviewRenderer::ShaderBrush, GradientType::Radial, &stops, 37., extent, true).is_ok());
        assert!(cache.get(PreviewRenderer::ShaderBrush, GradientType::Linear, &stops, 37., extent, true).is_ok());
        stops.insert(1, (0.3, gpui::white()));
        stops.insert(2, (0.7, gpui::black()));
        assert!(cache.get(PreviewRenderer::ShaderPaint, GradientType::Linear, &stops, 37., extent, true).is_ok());
        assert!(cache.get(PreviewRenderer::ShaderBrush, GradientType::Linear, &stops, 37., extent, true).is_err());
        assert!(cache.get(PreviewRenderer::ShaderPaint, GradientType::Radial, &stops, 37., extent, true).is_ok());
        assert!(cache.get(PreviewRenderer::ShaderBrush, GradientType::Radial, &stops, 37., extent, true).is_err());
        let Material::Paint(conic) =
            cache.get(PreviewRenderer::ShaderPaint, GradientType::Conic, &stops, 37., extent, true).unwrap()
        else {
            panic!("paint expected")
        };
        assert_eq!(conic.kind(), xgpui_shader::GradientKind::Conic { center: [0.5; 2], angle: 37. });
        assert_eq!(conic.stops().len(), stops.len());
        assert!(cache.get(PreviewRenderer::ShaderBrush, GradientType::Conic, &stops, 37., extent, true).is_err());
        assert!(cache.get(PreviewRenderer::ShaderPaint, GradientType::Linear, &stops, 37., extent, false).is_err());
        assert_eq!(stops.len(), 4);
    }
    #[cfg(feature = "test-support")]
    #[test]
    fn renderer_switching_preserves_existing_linear_controls() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.rotation_deg = 37.;
            cx.notify();
        });
        cx.run_until_parked();
        let original = view.update(cx, |view, cx| view.preview_stops(cx));
        let selector = cx.update(|_, cx| view.read(cx).renderer_selector.clone());
        let types = cx.update(|_, cx| view.read(cx).type_selector.clone());
        for (type_id, kind) in [
            ("linear", GradientType::Linear),
            ("radial", GradientType::Radial),
            ("conic", GradientType::Conic),
            ("linear", GradientType::Linear),
        ] {
            types.update(cx, |_, cx| cx.emit(SelectorEvent::Change { item_id: type_id.into(), label: type_id.into() }));
            cx.run_until_parked();
            for id in ["shader_paint", "shader_brush", "render", "render_async", "quads"] {
                selector.update(cx, |_, cx| cx.emit(SelectorEvent::Change { item_id: id.into(), label: id.into() }));
                cx.run_until_parked();
                view.update(cx, |view, cx| {
                    assert_eq!(view.rotation_deg, 37.);
                    assert_eq!(view.preview_stops(cx), original);
                    assert_eq!(view.selected_tab, BuilderTab::Gradients);
                    assert_eq!(view.gradient_type, kind);
                    if view.preview_renderer.is_shader() {
                        assert!(!view.uses_render_preview(cx));
                    }
                });
            }
        }
    }
}
