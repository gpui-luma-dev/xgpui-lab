use std::sync::Arc;
use gpui::{prelude::*, *};
use gpui_luma_look_radix::Look;
use xgpui_color::GamutMapping;
use xgpui_shader::WindowShaderExt;
use super::chromaticity_shader::{self, Parameters};
use crate::{
    lab_controls::{Choice, labels},
    theme::ColorVizLookExt,
};
use super::chromaticity_model as model;

pub(crate) struct ChromaticityLab {
    look: Arc<Look>,
    source: usize,
    patch: usize,
    simulated: usize,
    luminance: usize,
    mapping: GamutMapping,
    fill: bool,
    boundary: Arc<Vec<[f64; 2]>>,
    cells: Arc<Vec<model::Cell>>,
    error: Option<String>,
    choices: Vec<Choice>,
    shader: Result<xgpui_shader::instanced::ShaderProgram<Parameters>, String>,
}
impl ChromaticityLab {
    pub(crate) fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let choices = vec![
            Choice::new(
                "chromaticity-source",
                "Source",
                &look,
                labels(&model::NAMES),
                0,
                cx,
                |this: &mut Self, value, cx| {
                    this.source = value;
                    cx.notify();
                },
            ),
            Choice::new(
                "chromaticity-pixel",
                "Pixel",
                &look,
                model::PATCHES.iter().map(|p| p.0.to_owned()).collect(),
                0,
                cx,
                |this: &mut Self, value, cx| {
                    this.patch = value;
                    cx.notify();
                },
            ),
            Choice::new(
                "chromaticity-simulated",
                "Simulated gamut",
                &look,
                labels(&model::NAMES),
                1,
                cx,
                |this: &mut Self, value, cx| {
                    this.simulated = value;
                    this.rebuild();
                    cx.notify();
                },
            ),
            Choice::new(
                "chromaticity-luminance",
                "Fill Y",
                &look,
                labels(&["0.05", "0.18", "0.50"]),
                1,
                cx,
                |this: &mut Self, value, cx| {
                    this.luminance = value;
                    this.rebuild();
                    cx.notify();
                },
            ),
            Choice::new(
                "chromaticity-mapping",
                "Mapping",
                &look,
                labels(&["Clip", "Reduce chroma"]),
                0,
                cx,
                |this: &mut Self, value, cx| {
                    this.mapping = if value == 0 {
                        GamutMapping::Clip
                    } else {
                        GamutMapping::ReduceChroma
                    };
                    this.rebuild();
                    cx.notify();
                },
            ),
            Choice::new(
                "chromaticity-fill",
                "Fill",
                &look,
                labels(&["Visible", "Hidden"]),
                0,
                cx,
                |this: &mut Self, value, cx| {
                    this.fill = value == 0;
                    cx.notify();
                },
            ),
        ];
        let mut this = Self {
            look,
            source: 0,
            patch: 0,
            simulated: 1,
            luminance: 1,
            mapping: GamutMapping::Clip,
            fill: true,
            boundary: Arc::new(model::locus()),
            cells: Arc::new(Vec::new()),
            error: None,
            choices,
            shader: chromaticity_shader::program().map_err(|error| error.to_string()),
        };
        this.rebuild();
        this
    }
    fn rebuild(&mut self) {
        match model::cells(
            model::SPACES[self.simulated],
            model::LUMINANCE[self.luminance],
            self.mapping,
            &self.boundary,
        ) {
            Ok(cells) => {
                self.cells = Arc::new(cells);
                self.error = None;
            }
            Err(error) => {
                self.cells = Arc::new(Vec::new());
                self.error = Some(error.to_string());
            }
        }
    }
    fn chart(&self) -> AnyElement {
        let program = match &self.shader {
            Ok(program) => program.clone(),
            Err(error) => {
                return div().p_4().child(format!("Chromaticity shader unavailable: {error}")).into_any_element();
            }
        };
        let selected = model::source_xy(model::SPACES[self.source], model::PATCHES[self.patch].1);
        let point = selected.unwrap_or([0., 0.]);
        let parameters = Parameters {
            simulated: self.simulated as f32,
            luminance: model::LUMINANCE[self.luminance] as f32,
            reduce_chroma: if self.mapping == GamutMapping::ReduceChroma {
                1.
            } else {
                0.
            },
            show_fill: if self.fill { 1. } else { 0. },
            source_x: point[0] as f32,
            source_y: point[1] as f32,
            source_visible: if selected.is_some() { 1. } else { 0. },
            overlays: 1.,
        };
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                if let Err(error) = window.paint_custom_shader(bounds, &program, parameters) {
                    tracing::error!("Chromaticity chart: {error:#}");
                }
            },
        )
        .w_full()
        // Preserve the 48 px chart inset and enlarge the plotted area by 50%.
        .h(px(48. + (420. - 48.) * 1.5))
        .flex_shrink_0()
        .into_any_element()
    }
}
impl Render for ChromaticityLab {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let chrome = self.look.chrome();
        let outside = self.cells.iter().filter(|cell| cell.outside).count();
        let fraction = if self.cells.is_empty() {
            0.
        } else {
            100. * outside as f64 / self.cells.len() as f64
        };
        div().id("chromaticity-lab-scroll").size_full().overflow_y_scroll().p_5().flex().flex_col().gap_4()
            .bg(chrome.content_background).text_color(chrome.body_text)
            .child(div().text_size(px(22.)).child("Chromaticity & Gamut"))
            .child("Keep the pixel fixed and change its interpretation. Compare reference gamut boundaries, luminance, and mapping.")
            .child(div().flex().flex_wrap().gap_3().children(self.choices.clone()))
            .child(div().w_full().rounded_md().child(self.chart()))
            .child(div().flex().flex_wrap().gap_4().children(model::NAMES.into_iter().zip(model::COLORS).map(|(name, color)| {
                div().flex().items_center().gap_2().child(div().size(px(10.)).bg(rgb(color))).child(name)
            })))
            .child(format!("Sampled area outside {} at Y = {}: {fraction:.1}% · D65: white cross · Source: gold cross",
                model::NAMES[self.simulated], model::LUMINANCE[self.luminance]))
            .when_some(self.error.clone(), |view, error| view.child(error))
            .child(div().text_size(px(12.)).text_color(chrome.muted_text)
                .child("CIE 1931 2° spectral locus: CIE 2019, DOI 10.25039/CIE.DS.mifmy4x4, CC BY-SA 4.0. The plotted locus retains that license. Fill colors are an sRGB preview, even on a wide-gamut window. Hatching describes the selected simulated gamut, not a measurement of your monitor."))
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::{ChromaticityLab, GamutMapping};
    use gpui::TestAppContext;

    #[test]
    fn native_selectors_update_simulation_and_source_independently() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(|_, cx| ChromaticityLab::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        let (choices, outside) = cx.update(|_, cx| {
            let lab = view.read(cx);
            (lab.choices.clone(), lab.cells.iter().filter(|cell| cell.outside).count())
        });
        cx.update(|_, cx| choices[2].select(0, cx));
        cx.run_until_parked();
        cx.update(|_, cx| assert!(view.read(cx).cells.iter().filter(|cell| cell.outside).count() > outside));
        cx.update(|_, cx| choices[0].select(3, cx));
        cx.update(|_, cx| choices[1].select(5, cx));
        cx.update(|_, cx| choices[4].select(1, cx));
        cx.update(|_, cx| choices[5].select(1, cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let lab = view.read(cx);
            assert_eq!((lab.source, lab.patch, lab.simulated), (3, 5, 0));
            assert_eq!(lab.mapping, GamutMapping::ReduceChroma);
            assert!(!lab.fill);
            assert!(lab.error.is_none());
        });
    }
}
