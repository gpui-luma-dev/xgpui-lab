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
    sliders: Vec<(Parameter, Slider)>,
}

impl ColorPage {
    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let buttons = [
            (Action::Source, "Next source space"),
            (Action::Destination, "Next destination space"),
            (Action::Preset, "Next color pair"),
            (Action::Mapping, "Toggle clipping / chroma"),
            (Action::Alpha, "Toggle endpoint alpha"),
            (Action::Hue, "Next hue policy"),
            (Action::Model, "Next interpolation"),
            (Action::Samples, "Next sample count"),
            (Action::Reset, "Reset input"),
        ]
        .map(|(action, label)| {
            (
                action,
                radix::Button::new(format!("color-lab-{action:?}"))
                    .look(look.as_ref())
                    .outline()
                    .size(radix::ButtonSize::One)
                    .label(label)
                    .on_click(cx, move |this: &mut Self, cx| this.action(action, cx))
                    .spawn(cx),
            )
        })
        .to_vec();
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
        Self {
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
            sliders,
        }
    }

    fn action(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::Source => self.source_index = (self.source_index + 1) % 3,
            Action::Destination => self.destination_index = (self.destination_index + 1) % 3,
            Action::Preset => self.preset = (self.preset + 1) % reference::PRESETS.len(),
            Action::Mapping => {
                self.policy = if self.policy == GamutMapping::Clip {
                    GamutMapping::ReduceChroma
                } else {
                    GamutMapping::Clip
                }
            }
            Action::Alpha => self.translucent = !self.translucent,
            Action::Hue => self.hue_index = (self.hue_index + 1) % HUES.len(),
            Action::Model => self.model_index = (self.model_index + 1) % 3,
            Action::Samples => self.sample_count_index = (self.sample_count_index + 1) % 3,
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
    use gpui_luma::controls::button::ButtonEvent;

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
                page.buttons.iter().find(|(action, _)| *action == Action::Source).unwrap().1.clone(),
                page.sliders.iter().find(|(parameter, _)| *parameter == Parameter::Red).unwrap().1.clone(),
                page.buttons.iter().find(|(action, _)| *action == Action::Preset).unwrap().1.clone(),
            )
        });
        source.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        red.update(cx, |slider, cx| {
            slider.set_thumb_position(slider.primary_thumb_id(), 0.9, cx);
        });
        preset.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
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
