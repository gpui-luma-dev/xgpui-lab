//! Embedded paint-lab and shader-examples models, with Luma-owned controls.
mod editor;
pub(crate) mod grid;
pub(crate) mod seascape;
mod views;
mod typography;
mod gallery;
mod mesh;
mod hermite;
mod mesh_tangent;
mod shadows;
pub(crate) mod benchmarks;

use std::{sync::Arc, time::Instant};
use gpui::{prelude::*, *};
use gpui_luma::{
    controls::{button::Button, slider::Slider},
    prelude::HasPresenter,
};
use gpui_luma_look_radix::{self as radix, Look};
use xgpui_shader::{ControlPoint, Gradient, GradientStop, MeshGradient, MeshGrid};
use xgpui_shader::instanced::{ShaderProgram, presets::*};
use crate::theme::ColorVizLookExt;
use editor::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effect {
    Mesh,
    Linear,
    Radial,
    Conic,
    Diamond,
}
impl Effect {
    fn label(self) -> &'static str {
        match self {
            Self::Mesh => "Mesh",
            Self::Linear => "Linear",
            Self::Radial => "Radial",
            Self::Conic => "Conic",
            Self::Diamond => "Diamond",
        }
    }
}

fn gradient_stops() -> Vec<GradientStop> {
    vec![
        GradientStop::new(0., [1., 0.12, 0.18, 1.]),
        GradientStop::new(0.25, [1., 0.7, 0.12, 1.]),
        GradientStop::new(0.5, [0.25, 0.25, 1., 1.]),
        GradientStop::new(0.75, [0.05, 0.85, 0.75, 1.]),
        GradientStop::new(1., [1., 0.12, 0.18, 1.]),
    ]
}

fn initial_mesh() -> anyhow::Result<Vec<ControlPoint>> {
    Ok(MeshGrid::regular(
        3,
        3,
        vec![
            [1., 0.15, 0.12],
            [1., 0.65, 0.1],
            [0.3, 0.3, 1.],
            [0.95, 0.2, 0.6],
            [0.5, 0.8, 0.8],
            [0.1, 0.6, 1.],
            [0.45, 0.1, 0.8],
            [0.1, 0.9, 0.6],
            [0.1, 0.3, 0.8],
        ],
    )?
    .points()
    .to_vec())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Effect,
    Stop,
    Add,
    Duplicate,
    Remove,
    Point,
    Reset,
    Animate,
    Opacity,
    Clip,
    Native,
    Mode,
    Count,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parameter {
    Angle,
    CenterX,
    CenterY,
    RadiusX,
    RadiusY,
    Offset,
    Red,
    Green,
    Blue,
    Alpha,
    X,
    Y,
    MeshRed,
    MeshGreen,
    MeshBlue,
    U,
    V,
    Progress,
    ShaderAlpha,
}
impl Parameter {
    fn config(self) -> (&'static str, f32, f32) {
        match self {
            Self::Angle => ("Angle", 0., 360.),
            Self::CenterX => ("Center X", 0., 1.),
            Self::CenterY => ("Center Y", 0., 1.),
            Self::RadiusX => ("Radius X", 0.01, 2.),
            Self::RadiusY => ("Radius Y", 0.01, 2.),
            Self::Offset => ("Stop position", 0., 1.),
            Self::Red | Self::MeshRed => ("Red", 0., 1.),
            Self::Green | Self::MeshGreen => ("Green", 0., 1.),
            Self::Blue | Self::MeshBlue => ("Blue", 0., 1.),
            Self::Alpha | Self::ShaderAlpha => ("Alpha", 0., 1.),
            Self::X => ("Point X", 0., 1.),
            Self::Y => ("Point Y", 0., 1.),
            Self::U => ("U tangent X", -1., 1.),
            Self::V => ("V tangent Y", -1., 1.),
            Self::Progress => ("Progress", -1., 1.),
        }
    }
}

struct Programs {
    wash: xgpui_shader::experimental::ShaderProgram,
    minimal: ShaderProgram<EffectInstance>,
    glow: ShaderProgram<EffectInstance>,
    particles: ShaderProgram<EffectInstance>,
    rounded: ShaderProgram<EffectInstance>,
}
impl Programs {
    fn new() -> anyhow::Result<Self> {
        Ok(Self {
            wash: xgpui_shader::experimental::ShaderProgram::new_v1(include_str!("shader/portable-wash.wgsl"))?,
            minimal: ShaderProgram::new(include_str!("shader/minimal.wgsl"))?,
            glow: ShaderProgram::new(PULSE_GLOW)?,
            particles: ShaderProgram::new(PARTICLE)?,
            rounded: ShaderProgram::new(ROUNDED_BOX)?,
        })
    }
}

pub struct ShaderPage {
    look: Arc<Look>,
    typography: Entity<typography::TypographyDemo>,
    mesh_pane: Entity<mesh::MeshPane>,
    shadow_pane: Entity<shadows::ShadowPane>,
    grid_pane: Entity<grid::GridPane>,
    sea_pane: Entity<seascape::SeaPane>,
    gallery_buttons: Vec<(&'static str, Entity<Button>)>,
    gallery_choices: Vec<(&'static str, crate::lab_controls::Choice)>,
    gallery_labels: std::cell::RefCell<std::collections::HashMap<&'static str, String>>,
    freeform_step: u8,
    backdrop_changed: bool,
    backdrop_moved: bool,
    backdrop_disabled: bool,
    direct_paths: bool,
    layer_blend_step: usize,
    layer_blur_step: usize,
    layer_blur_full_resolution: bool,
    layer_filter_step: usize,
    active_tab: SharedString,
    effect_index: usize,
    settings: Vec<Settings>,
    mesh: anyhow::Result<Vec<ControlPoint>>,
    selected_point: usize,
    programs: anyhow::Result<Programs>,
    buttons: Vec<(Action, Entity<Button>)>,
    choices: Vec<(Action, crate::lab_controls::Choice)>,
    sliders: Vec<(Parameter, Slider)>,
    animated: bool,
    time: f32,
    last_frame: Instant,
    half_opacity: bool,
    clipped: bool,
    native: bool,
    mode: usize,
    count_index: usize,
    progress: f32,
    shader_alpha: f32,
}

impl ShaderPage {
    const EFFECTS: [Effect; 4] = [Effect::Linear, Effect::Radial, Effect::Conic, Effect::Diamond];

    fn init_choices(&mut self, cx: &mut Context<Self>) {
        self.choices = [Action::Effect, Action::Stop, Action::Point, Action::Mode, Action::Count]
            .into_iter()
            .map(|action| {
                let (label, options, selected) = self.choice_options(action);
                let choice = crate::lab_controls::Choice::new(
                    format!("shader-{action:?}"),
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
            Action::Effect => {
                ("Gradient type", Self::EFFECTS.iter().map(|e| e.label().to_string()).collect(), self.effect_index)
            }
            Action::Stop => (
                "Selected stop",
                (0..self.settings[self.effect_index].stops.len()).map(|i| format!("Stop {}", i + 1)).collect(),
                self.settings[self.effect_index].selected,
            ),
            Action::Point => ("Mesh point", (1..=9).map(|i| format!("Point {i}")).collect(), self.selected_point),
            Action::Mode => ("Example", crate::lab_controls::labels(&["Glow", "Particles", "Rounded"]), self.mode),
            Action::Count => {
                ("Workload", crate::lab_controls::labels(&["100", "1000", "5000", "10000"]), self.count_index)
            }
            _ => unreachable!("action has no selector"),
        }
    }
    fn select_choice(&mut self, action: Action, index: usize, cx: &mut Context<Self>) {
        match action {
            Action::Effect => {
                self.freeze();
                self.effect_index = index;
            }
            Action::Stop => {
                self.freeze();
                self.settings[self.effect_index].selected = index;
            }
            Action::Point => {
                self.selected_point = index;
            }
            Action::Mode => {
                self.mode = index;
            }
            Action::Count => {
                self.count_index = index;
            }
            _ => unreachable!("action has no selector"),
        }
        self.sync_sliders(cx);
        cx.notify();
    }

    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let buttons = [
            (Action::Add, "Add stop"),
            (Action::Duplicate, "Duplicate stop"),
            (Action::Remove, "Remove stop"),
            (Action::Reset, "Reset"),
            (Action::Animate, "Animate / Pause"),
            (Action::Opacity, "Toggle opacity"),
            (Action::Clip, "Toggle clipping"),
            (Action::Native, "Toggle native coverage"),
        ]
        .map(|(action, label)| {
            let button = radix::Button::new(format!("shader-{action:?}"))
                .look(look.as_ref())
                .outline()
                .label(label)
                .on_click(cx, move |this: &mut Self, cx| this.action(action, cx))
                .spawn(cx);
            (action, button)
        })
        .to_vec();
        let sliders = [
            Parameter::Angle,
            Parameter::CenterX,
            Parameter::CenterY,
            Parameter::RadiusX,
            Parameter::RadiusY,
            Parameter::Offset,
            Parameter::Red,
            Parameter::Green,
            Parameter::Blue,
            Parameter::Alpha,
            Parameter::X,
            Parameter::Y,
            Parameter::MeshRed,
            Parameter::MeshGreen,
            Parameter::MeshBlue,
            Parameter::U,
            Parameter::V,
            Parameter::Progress,
            Parameter::ShaderAlpha,
        ]
        .map(|parameter| {
            let (_, min, max) = parameter.config();
            let slider = radix::Slider::new(format!("shader-{parameter:?}"))
                .look(look.as_ref())
                .size(radix::SliderSize::One)
                .range(min..max)
                .step(if parameter == Parameter::Angle { 1. } else { 0.01 })
                .on_value_changed(cx, move |this: &mut Self, _, value, cx| this.edit(parameter, value, cx))
                .spawn(cx);
            (parameter, slider)
        })
        .to_vec();
        let typography = cx.new(|cx| typography::TypographyDemo::new(look.clone(), cx));
        let gallery_buttons = [
            ("backdrop-source", "Swap stripe colors"),
            ("backdrop-foreground", "Move marker"),
            ("backdrop-toggle", "Toggle blur"),
            ("layer-blur-quality", "Toggle blur quality"),
            ("path-route", "Toggle direct / native"),
        ]
        .into_iter()
        .map(|(id, label)| {
            let button = radix::Button::new(format!("gallery-{id}"))
                .look(look.as_ref())
                .outline()
                .label(label)
                .on_click(cx, move |this: &mut Self, cx| {
                    match id {
                        "backdrop-source" => this.backdrop_changed = !this.backdrop_changed,
                        "backdrop-foreground" => this.backdrop_moved = !this.backdrop_moved,
                        "backdrop-toggle" => this.backdrop_disabled = !this.backdrop_disabled,

                        "layer-blur-quality" => this.layer_blur_full_resolution = !this.layer_blur_full_resolution,
                        _ => this.direct_paths = !this.direct_paths,
                    }
                    cx.notify();
                })
                .spawn(cx);
            (id, button)
        })
        .collect();
        let gallery_choices = vec![
            (
                "freeform-points",
                crate::lab_controls::Choice::new(
                    "gallery-freeform-points",
                    "Freeform points",
                    &look,
                    crate::lab_controls::labels(&["4", "8", "16"]),
                    0,
                    cx,
                    |this: &mut Self, index, cx| {
                        this.freeform_step = index as _;
                        cx.notify();
                    },
                ),
            ),
            (
                "layer-filter-cycle",
                crate::lab_controls::Choice::new(
                    "gallery-layer-filter-cycle",
                    "Filter",
                    &look,
                    crate::lab_controls::labels(&["Invert 100%", "Invert 50%", "Saturation 2"]),
                    0,
                    cx,
                    |this: &mut Self, index, cx| {
                        this.layer_filter_step = index as _;
                        cx.notify();
                    },
                ),
            ),
            (
                "layer-blend-cycle",
                crate::lab_controls::Choice::new(
                    "gallery-layer-blend-cycle",
                    "Blend",
                    &look,
                    crate::lab_controls::labels(&[
                        "Normal",
                        "Multiply",
                        "Screen",
                        "Overlay",
                        "Color dodge",
                        "Soft light",
                        "Linear burn",
                    ]),
                    0,
                    cx,
                    |this: &mut Self, index, cx| {
                        this.layer_blend_step = index as _;
                        cx.notify();
                    },
                ),
            ),
            (
                "layer-blur-cycle",
                crate::lab_controls::Choice::new(
                    "gallery-layer-blur-cycle",
                    "Subtree blur",
                    &look,
                    crate::lab_controls::labels(&["0 px", "4 px", "8 px", "16 px"]),
                    0,
                    cx,
                    |this: &mut Self, index, cx| {
                        this.layer_blur_step = index as _;
                        cx.notify();
                    },
                ),
            ),
        ];
        let mesh_pane = cx.new(|cx| mesh::MeshPane::new(look.clone(), cx));
        let sea_pane = cx.new(|cx| seascape::SeaPane::new(look.clone(), cx));
        let grid_pane = cx.new(|cx| grid::GridPane::new(look.clone(), cx));
        let shadow_pane = cx.new(|cx| shadows::ShadowPane::new(look.clone(), cx));
        let mut page = Self {
            mesh_pane,
            shadow_pane,
            grid_pane,
            sea_pane,
            gallery_buttons,
            gallery_choices,
            gallery_labels: Default::default(),
            freeform_step: 0,
            backdrop_changed: false,
            backdrop_moved: false,
            backdrop_disabled: false,
            direct_paths: true,
            layer_blend_step: 0,
            layer_blur_step: 0,
            layer_blur_full_resolution: false,
            layer_filter_step: 0,
            typography,
            look,
            active_tab: "Gradients".into(),
            effect_index: 1,
            settings: Self::EFFECTS.map(Settings::new).to_vec(),
            mesh: initial_mesh(),
            selected_point: 0,
            programs: Programs::new(),
            buttons,
            choices: Vec::new(),
            sliders,
            animated: false,
            time: 0.,
            last_frame: Instant::now(),
            half_opacity: false,
            clipped: false,
            native: false,
            mode: 0,
            count_index: 0,
            progress: 0.,
            shader_alpha: 1.,
        };
        page.init_choices(cx);
        page.sync_sliders(cx);
        page
    }

    pub fn activate(&mut self, tab: SharedString, cx: &mut Context<Self>) {
        if self.active_tab != tab {
            self.grid_pane.update(cx, |pane, _| pane.suspend());
            self.sea_pane.update(cx, |pane, _| pane.suspend());
            self.typography.update(cx, |demo, _| demo.suspend());
            self.shadow_pane.update(cx, |page, _| page.animated = false);
        }
        if tab == "Shader Depot: Grid" {
            self.grid_pane.update(cx, |pane, _| pane.activate());
        }
        if tab == "Shadertoy: Seascape" {
            self.sea_pane.update(cx, |pane, _| pane.activate());
        }
        self.active_tab = tab;
        self.animated = false;
        self.last_frame = Instant::now();
        self.sync_sliders(cx);
        cx.notify();
    }

    pub fn requires_sdr(&self) -> bool {
        matches!(self.active_tab.as_ref(), "Typography" | "Brushes" | "Layer Effects")
    }

    pub fn suspend(&mut self, cx: &mut Context<Self>) {
        self.animated = false;
        self.grid_pane.update(cx, |pane, _| pane.suspend());
        self.sea_pane.update(cx, |pane, _| pane.suspend());
        self.typography.update(cx, |demo, _| demo.suspend());
        self.shadow_pane.update(cx, |page, _| page.animated = false);
    }

    fn gallery_button(&self, id: &'static str, label: impl Into<String>, cx: &mut Context<Self>) -> gpui::AnyElement {
        if let Some((_, choice)) = self.gallery_choices.iter().find(|(key, _)| *key == id) {
            return choice.clone().into_any_element();
        }
        let button = self.gallery_buttons.iter().find(|(key, _)| *key == id).unwrap().1.clone();
        let label = label.into();
        let changed = self.gallery_labels.borrow().get(id) != Some(&label);
        if changed {
            self.gallery_labels.borrow_mut().insert(id, label.clone());
            button.update(cx, |button, cx| button.set_label(label, cx));
        }
        button.into_any_element()
    }
    fn value(&self, parameter: Parameter) -> f32 {
        let s = &self.settings[self.effect_index];
        let stop = s.stops[s.selected];
        let point = self.mesh.as_ref().ok().and_then(|points| points.get(self.selected_point));
        match parameter {
            Parameter::Angle => s.angle,
            Parameter::CenterX => s.center[0],
            Parameter::CenterY => s.center[1],
            Parameter::RadiusX => s.radius[0],
            Parameter::RadiusY => s.radius[1],
            Parameter::Offset => stop.offset,
            Parameter::Red => stop.color[0],
            Parameter::Green => stop.color[1],
            Parameter::Blue => stop.color[2],
            Parameter::Alpha => stop.color[3],
            Parameter::X => point.map_or(0., |p| p.position[0]),
            Parameter::Y => point.map_or(0., |p| p.position[1]),
            Parameter::MeshRed => point.map_or(0., |p| p.color[0]),
            Parameter::MeshGreen => point.map_or(0., |p| p.color[1]),
            Parameter::MeshBlue => point.map_or(0., |p| p.color[2]),
            Parameter::U => point.map_or(0., |p| p.u_tangent[0]),
            Parameter::V => point.map_or(0., |p| p.v_tangent[1]),
            Parameter::Progress => self.progress,
            Parameter::ShaderAlpha => self.shader_alpha,
        }
    }

    fn sync_sliders(&mut self, cx: &mut Context<Self>) {
        for (parameter, slider) in &self.sliders {
            let value = self.value(*parameter);
            slider.update(cx, |slider, cx| slider.set_value(value, cx));
        }
    }

    fn freeze(&mut self) {
        self.settings[self.effect_index].freeze(Self::EFFECTS[self.effect_index], self.time);
        self.time = 0.;
        self.animated = false;
    }

    fn action(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::Effect | Action::Stop | Action::Point | Action::Mode | Action::Count => {
                unreachable!("selector setting")
            }
            Action::Animate => {
                self.animated = !self.animated;
                self.last_frame = Instant::now();
            }
            Action::Opacity => self.half_opacity = !self.half_opacity,
            Action::Clip => self.clipped = !self.clipped,
            Action::Native => self.native = !self.native,
            _ => {
                self.freeze();
                let s = &mut self.settings[self.effect_index];
                match action {
                    Action::Add => s.add(),
                    Action::Duplicate => s.duplicate(),
                    Action::Remove => s.remove(),
                    Action::Reset if self.active_tab == "Mesh Editor" => self.mesh = initial_mesh(),
                    Action::Reset => *s = Settings::new(Self::EFFECTS[self.effect_index]),
                    _ => {}
                }
            }
        }
        self.sync_sliders(cx);
        cx.notify();
    }

    fn edit(&mut self, parameter: Parameter, value: f32, cx: &mut Context<Self>) {
        if (self.value(parameter) - value).abs() < 0.0001 {
            return;
        }
        self.freeze();
        let s = &mut self.settings[self.effect_index];
        let stop = &mut s.stops[s.selected];
        match parameter {
            Parameter::Angle => s.angle = value,
            Parameter::CenterX => s.center[0] = value,
            Parameter::CenterY => s.center[1] = value,
            Parameter::RadiusX => s.radius[0] = value,
            Parameter::RadiusY => s.radius[1] = value,
            Parameter::Offset => s.move_stop(value),
            Parameter::Red => stop.color[0] = value,
            Parameter::Green => stop.color[1] = value,
            Parameter::Blue => stop.color[2] = value,
            Parameter::Alpha => stop.color[3] = value,
            Parameter::Progress => self.progress = value,
            Parameter::ShaderAlpha => self.shader_alpha = value,
            _ => {
                if let Ok(points) = &mut self.mesh {
                    let point = &mut points[self.selected_point];
                    match parameter {
                        Parameter::X => point.position[0] = value,
                        Parameter::Y => point.position[1] = value,
                        Parameter::MeshRed => point.color[0] = value,
                        Parameter::MeshGreen => point.color[1] = value,
                        Parameter::MeshBlue => point.color[2] = value,
                        Parameter::U => point.u_tangent[0] = value,
                        Parameter::V => point.v_tangent[1] = value,
                        _ => {}
                    }
                }
            }
        }
        self.sync_sliders(cx);
        cx.notify();
    }

    fn producer(&self) -> anyhow::Result<Arc<dyn CustomContent>> {
        let effect = if self.active_tab == "Mesh Editor" {
            Effect::Mesh
        } else {
            Self::EFFECTS[self.effect_index]
        };
        if effect == Effect::Mesh {
            let points = self.mesh.as_ref().map_err(|e| anyhow::anyhow!("{e:#}"))?;
            return Ok(Arc::new(MeshGradient::new(MeshGrid::new(3, 3, points.clone())?, 32)?));
        }
        let effect = Self::EFFECTS[self.effect_index];
        let settings = &self.settings[self.effect_index];
        let kind = settings.kind(effect, self.time).ok_or_else(|| anyhow::anyhow!("missing analytic gradient"))?;
        Ok(Arc::new(Gradient::new(kind, settings.stops.clone())?))
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::{ShaderPage, Action, Parameter};
    use gpui::TestAppContext;
    use gpui_luma::controls::button::ButtonEvent;

    #[test]
    fn stop_selector_tracks_edits_and_retains_each_gradient_selection() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| ShaderPage::new(crate::theme::default_look(), cx));
        let choose = |action, index, cx: &mut gpui::VisualTestContext| {
            let choice =
                cx.update(|_, cx| page.read(cx).choices.iter().find(|(key, _)| *key == action).unwrap().1.clone());
            cx.update(|_, cx| choice.select(index, cx));
            cx.run_until_parked();
        };
        choose(Action::Effect, 3, cx);
        page.update(cx, |page, cx| {
            page.action(Action::Add, cx);
            page.action(Action::Add, cx);
            page.sync_choices(cx);
        });
        let last = cx.update(|_, cx| page.read(cx).settings[3].stops.len() - 1);
        choose(Action::Stop, last, cx);
        page.update(cx, |page, cx| {
            assert_eq!(page.settings[3].selected, last);
            page.action(Action::Remove, cx);
            page.sync_choices(cx);
            let selected = page.settings[3].selected;
            let choice = &page.choices.iter().find(|(key, _)| *key == Action::Stop).unwrap().1;
            assert_eq!(choice.selector.read(cx).selected_id().unwrap().as_ref(), selected.to_string());
            assert_eq!(selected, last - 1);
        });
        choose(Action::Effect, 0, cx);
        choose(Action::Effect, 3, cx);
        cx.update(|_, cx| assert_eq!(page.read(cx).settings[3].selected, last - 1));
    }

    #[test]
    fn shader_tabs_render_and_luma_controls_edit_retained_lab_state() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (page, cx) = app.add_window_view(|_, cx| ShaderPage::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        cx.update(|_, cx| assert!(page.read(cx).programs.is_ok(), "lab WGSL must validate"));
        for tab in [
            "Gradients",
            "Mesh Editor",
            "Gradient Paints",
            "WGSL Paints",
            "Shader Depot: Grid",
            "Shadertoy: Seascape",
            "Instancing",
            "Typography",
            "Brushes",
            "Layer Effects",
            "Shadows",
            "Borders",
        ] {
            page.update(cx, |page, cx| page.activate(tab.into(), cx));
            cx.run_until_parked();
        }
        page.update(cx, |page, cx| page.activate("Mesh Editor".into(), cx));
        let (next_point, x_slider) = cx.update(|_, cx| {
            let page = page.read(cx);
            (
                page.choices.iter().find(|(action, _)| *action == Action::Point).unwrap().1.clone(),
                page.sliders.iter().find(|(parameter, _)| *parameter == Parameter::X).unwrap().1.clone(),
            )
        });
        cx.update(|_, cx| next_point.select(1, cx));
        cx.run_until_parked();
        x_slider.update(cx, |slider, cx| {
            slider.set_thumb_position(slider.primary_thumb_id(), 0.6, cx);
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert_eq!(page.selected_point, 1);
            assert!((page.mesh.as_ref().unwrap()[1].position[0] - 0.6).abs() < 0.001);
            assert!(page.producer().is_ok());
        });
        // Changing another tab must retain the edited mesh and separate analytic settings.
        page.update(cx, |page, cx| page.activate("Gradients".into(), cx));
        let add = cx
            .update(|_, cx| page.read(cx).buttons.iter().find(|(action, _)| *action == Action::Add).unwrap().1.clone());
        add.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert_eq!(page.settings[1].stops.len(), 6);
            assert_eq!(page.settings[0].stops.len(), 5);
            assert!((page.mesh.as_ref().unwrap()[1].position[0] - 0.6).abs() < 0.001);
            assert!(page.producer().is_ok());
        });
    }
}

fn initial_mesh_grid() -> anyhow::Result<MeshGrid> {
    MeshGrid::new(3, 3, initial_mesh()?)
}
