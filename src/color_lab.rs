//! Color-lab reference comparisons and xgpui-color CPU API exploration.
mod views;

use std::sync::Arc;
use gpui::{Context, Entity, SharedString};
use gpui_luma::{
    controls::{button::Button, slider::Slider},
    prelude::HasPresenter,
};
use gpui_luma_look_radix::{self as radix, Look};
use xgpui_color::{
    ColorInterpolation, EncodedRgb, GamutMapping, HueInterpolation, LinearGradient, LinearRgb, LinearRgba, RgbSpace,
};
use crate::gamut::reference;

const SPACES: [RgbSpace; 3] = [RgbSpace::Srgb, RgbSpace::DisplayP3, RgbSpace::Rec2020];
const HUES: [HueInterpolation; 4] = [
    HueInterpolation::Shorter,
    HueInterpolation::Longer,
    HueInterpolation::Increasing,
    HueInterpolation::Decreasing,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Source,
    Destination,
    Preset,
    Mapping,
    Alpha,
    Hue,
    Model,
    Samples,
    Reset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parameter {
    Red,
    Green,
    Blue,
    Position,
    AxisX,
    AxisY,
}
impl Parameter {
    fn config(self) -> (&'static str, f32, f32, f32) {
        match self {
            Self::Red => ("Encoded red", 0., 1., 0.2),
            Self::Green => ("Encoded green", 0., 1., 0.5),
            Self::Blue => ("Encoded blue", 0., 1., 0.8),
            Self::Position => ("Sample position t", -0.25, 1.25, 0.5),
            Self::AxisX => ("Axis end X", 0., 640., 320.),
            Self::AxisY => ("Axis end Y", 0., 320., 0.),
        }
    }
}

pub struct ColorPage {
    look: Arc<Look>,
    pub active_tab: SharedString,
    input: [f64; 3],
    source_index: usize,
    destination_index: usize,
    preset: usize,
    policy: GamutMapping,
    translucent: bool,
    hue_index: usize,
    model_index: usize,
    sample_count_index: usize,
    position: f64,
    axis: [f64; 2],
    buttons: Vec<(Action, Entity<Button>)>,
    choices: Vec<(Action, crate::lab_controls::Choice)>,
    sliders: Vec<(Parameter, Slider)>,
    transparency: gpui_luma::controls::switch::Switch,
}

impl ColorPage {
    fn init_choices(&mut self, cx: &mut Context<Self>) {
        self.choices =
            [Action::Source, Action::Destination, Action::Preset, Action::Hue, Action::Model, Action::Samples]
                .into_iter()
                .map(|action| {
                    let (label, options, selected) = self.choice_options(action);
                    let choice = crate::lab_controls::Choice::new(
                        format!("color_lab-{action:?}"),
                        label,
                        &self.look,
                        options,
                        selected,
                        cx,
                        move |this: &mut Self, index, cx| this.select_choice(action, index, cx),
                    );
                    (action, choice)
                })
                .collect();
    }
    fn sync_choices(&self, cx: &mut gpui::App) {
        for (action, choice) in &self.choices {
            let (_, options, selected) = self.choice_options(*action);
            choice.sync(options, selected, cx);
        }
    }
    fn choice_options(&self, action: Action) -> (&'static str, Vec<String>, usize) {
        match action {
            Action::Source => ("Source space", SPACES.iter().map(|s| format!("{s:?}")).collect(), self.source_index),
            Action::Destination => {
                ("Destination space", SPACES.iter().map(|s| format!("{s:?}")).collect(), self.destination_index)
            }
            Action::Preset => ("Color pair", reference::PRESETS.iter().map(|s| s.to_string()).collect(), self.preset),
            Action::Hue => ("Hue policy", HUES.iter().map(|s| format!("{s:?}")).collect(), self.hue_index),
            Action::Model => {
                ("Interpolation", crate::lab_controls::labels(&["Linear RGB", "Oklab", "Oklch"]), self.model_index)
            }
            Action::Samples => {
                ("Sample count", crate::lab_controls::labels(&["32", "64", "128"]), self.sample_count_index)
            }
            _ => unreachable!("action has no selector"),
        }
    }
    fn select_choice(&mut self, action: Action, index: usize, cx: &mut Context<Self>) {
        match action {
            Action::Source => {
                self.source_index = index;
            }
            Action::Destination => {
                self.destination_index = index;
            }
            Action::Preset => {
                self.preset = index;
            }
            Action::Hue => {
                self.hue_index = index;
            }
            Action::Model => {
                self.model_index = index;
            }
            Action::Samples => {
                self.sample_count_index = index;
            }
            _ => unreachable!("action has no selector"),
        }

        cx.notify();
    }

    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let buttons = [(Action::Mapping, "Toggle clipping / chroma"), (Action::Reset, "Reset input")]
            .map(|(action, label)| {
                (
                    action,
                    radix::Button::new(format!("color-lab-{action:?}"))
                        .look(look.as_ref())
                        .outline()
                        .label(label)
                        .on_click(cx, move |this: &mut Self, cx| this.action(action, cx))
                        .spawn(cx),
                )
            })
            .to_vec();
        let transparency = radix::Switch::new("color-lab-transparency")
            .look(look.as_ref())
            .label("Endpoint transparency")
            .on_value_changed(cx, |this: &mut Self, on, cx| {
                this.translucent = on;
                cx.notify();
            })
            .spawn(cx);
        let sliders = [
            Parameter::Red,
            Parameter::Green,
            Parameter::Blue,
            Parameter::Position,
            Parameter::AxisX,
            Parameter::AxisY,
        ]
        .map(|parameter| {
            let (_, min, max, value) = parameter.config();
            (
                parameter,
                radix::Slider::new(format!("color-lab-{parameter:?}"))
                    .look(look.as_ref())
                    .size(radix::SliderSize::One)
                    .range(min..max)
                    .step(if matches!(parameter, Parameter::AxisX | Parameter::AxisY) {
                        1.
                    } else {
                        0.01
                    })
                    .value(value as f64)
                    .on_value_changed(cx, move |this: &mut Self, _, value, cx| {
                        match parameter {
                            Parameter::Red => this.input[0] = value as f64,
                            Parameter::Green => this.input[1] = value as f64,
                            Parameter::Blue => this.input[2] = value as f64,
                            Parameter::Position => this.position = value as f64,
                            Parameter::AxisX => this.axis[0] = value as f64,
                            Parameter::AxisY => this.axis[1] = value as f64,
                        }
                        cx.notify();
                    })
                    .spawn(cx),
            )
        })
        .to_vec();
        let mut page = Self {
            look,
            active_tab: "Color Spaces".into(),
            input: [0.2, 0.5, 0.8],
            source_index: 0,
            destination_index: 1,
            preset: 0,
            policy: GamutMapping::Clip,
            translucent: false,
            hue_index: 0,
            model_index: 1,
            sample_count_index: 0,
            position: 0.5,
            axis: [320., 0.],
            buttons,
            choices: Vec::new(),
            transparency,
            sliders,
        };
        page.init_choices(cx);
        page
    }

    fn action(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::Source | Action::Destination | Action::Preset | Action::Hue | Action::Model | Action::Samples => {
                unreachable!("selector setting")
            }
            Action::Mapping => {
                self.policy = if self.policy == GamutMapping::Clip {
                    GamutMapping::ReduceChroma
                } else {
                    GamutMapping::Clip
                }
            }
            Action::Alpha => self.translucent = !self.translucent,
            Action::Reset => {
                self.input = [0.2, 0.5, 0.8];
                self.position = 0.5;
                self.axis = [320., 0.];
                for (parameter, slider) in &self.sliders {
                    slider.update(cx, |slider, cx| slider.set_value(parameter.config().3, cx));
                }
            }
        }
        cx.notify();
    }

    fn input_color(&self) -> Result<LinearRgb, xgpui_color::ColorError> {
        EncodedRgb::new(SPACES[self.source_index], self.input)?.decode()
    }

    fn model(&self) -> ColorInterpolation {
        match self.model_index {
            0 => ColorInterpolation::LinearRgb,
            1 => ColorInterpolation::Oklab,
            _ => ColorInterpolation::Oklch(HUES[self.hue_index]),
        }
    }

    fn endpoints(&self) -> Result<[LinearRgba; 2], xgpui_color::ColorError> {
        let (a, b) = match self.preset {
            0 => ([0., 0., 1.], [1., 1., 0.]),
            1 => ([1., 0., 0.], [0., 1., 0.]),
            _ => ([1., 0.18, 0.], [0.45, 0., 1.]),
        };
        Ok([
            LinearRgba::new(LinearRgb::new(RgbSpace::DisplayP3, a)?, 1.)?,
            LinearRgba::new(LinearRgb::new(RgbSpace::DisplayP3, b)?, if self.translucent { 0.25 } else { 1. })?,
        ])
    }

    fn gradient(&self, model: ColorInterpolation) -> Result<LinearGradient, xgpui_color::ColorError> {
        LinearGradient::new([0., 0.], self.axis, self.endpoints()?, SPACES[self.destination_index], model)
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::{Action, ColorPage, Parameter, SPACES};
    use gpui::TestAppContext;

    #[test]
    fn color_tabs_render_and_luma_controls_retain_independent_lab_state() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (page, cx) = app.add_window_view(|_, cx| ColorPage::new(crate::theme::default_look(), cx));
        for tab in ["Color Spaces", "Conversions", "Interpolation", "Gamut Mapping", "CPU Gradients"] {
            page.update(cx, |page, cx| {
                page.active_tab = tab.into();
                cx.notify();
            });
            cx.run_until_parked();
        }
        let (source, red, preset) = cx.update(|_, cx| {
            let page = page.read(cx);
            (
                page.choices.iter().find(|(action, _)| *action == Action::Source).unwrap().1.clone(),
                page.sliders.iter().find(|(parameter, _)| *parameter == Parameter::Red).unwrap().1.clone(),
                page.choices.iter().find(|(action, _)| *action == Action::Preset).unwrap().1.clone(),
            )
        });
        let transparency = cx.update(|_, cx| page.read(cx).transparency.clone());
        transparency.update(cx, |switch, cx| {
            switch.set_data(true, cx);
            cx.emit(gpui_luma::controls::switch::SwitchEvent::Change { on: true });
        });
        cx.update(|_, cx| source.select(1, cx));
        red.update(cx, |slider, cx| {
            slider.set_thumb_position(slider.primary_thumb_id(), 0.9, cx);
        });
        cx.update(|_, cx| preset.select(1, cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert!(page.translucent);
            assert!(*page.transparency.read(cx).data());
            assert_eq!(page.source_index, 1);
            assert_eq!(page.preset, 1);
            assert!((page.input[0] - 0.9).abs() < 0.001);
            let roundtrip = page
                .input_color()
                .unwrap()
                .convert(SPACES[2])
                .unwrap()
                .convert(SPACES[page.source_index])
                .unwrap()
                .encode()
                .unwrap()
                .components();
            for (actual, expected) in roundtrip.into_iter().zip(page.input) {
                assert!((actual - expected).abs() < 1e-6);
            }
        });
    }

    #[test]
    fn cpu_gradient_projects_geometry_extends_endpoints_and_handles_coincident_axis() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (page, cx) = app.add_window_view(|_, cx| ColorPage::new(crate::theme::default_look(), cx));
        page.update(cx, |page, _| {
            page.translucent = true;
            page.axis = [120., 80.];
            let gradient = page.gradient(page.model()).unwrap();
            let first = gradient.sample_at(0.).unwrap();
            let last = gradient.sample_at(1.).unwrap();
            assert_eq!(gradient.sample([-120., -80.]).unwrap(), first);
            assert_eq!(gradient.sample([240., 160.]).unwrap(), last);
            assert!((gradient.sample([60., 40.]).unwrap().alpha() - 0.625).abs() < 1e-12);
            for count in 0..3 {
                page.sample_count_index = count;
                assert!(page.cpu_gradients().is_ok(), "all CPU sample counts must render");
            }
            page.axis = [0., 0.];
            assert_eq!(page.gradient(page.model()).unwrap().sample([0., 0.]).unwrap(), last);
        });
    }
}
