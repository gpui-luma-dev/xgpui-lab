//! App-only import of Christian Helgeson's Grid Shader (BSD-3-Clause).
//! Copyright 2019 WebGPU Samples Contributors; see licenses/ShaderDepot-BSD-3-Clause.txt.
use super::*;
use xgpui_shader::{
    shader_paint,
    instanced::{InstanceField, ShaderInstance},
    painted, WindowShaderExt,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GridInstance {
    pub grid_dimensions: f32,
    pub cell_origin_x: f32,
    pub cell_origin_y: f32,
    pub line_width: f32,
}
impl Default for GridInstance {
    fn default() -> Self {
        Self { grid_dimensions: 10., cell_origin_x: 0., cell_origin_y: 0., line_width: 1. }
    }
}
impl ShaderInstance for GridInstance {
    const FIELDS: &'static [InstanceField] = &[
        InstanceField { name: "grid_dimensions", offset: std::mem::offset_of!(Self, grid_dimensions), components: 1 },
        InstanceField { name: "cell_origin_x", offset: std::mem::offset_of!(Self, cell_origin_x), components: 1 },
        InstanceField { name: "cell_origin_y", offset: std::mem::offset_of!(Self, cell_origin_y), components: 1 },
        InstanceField { name: "line_width", offset: std::mem::offset_of!(Self, line_width), components: 1 },
    ];
}
pub(crate) struct GridPrograms {
    pub shader_paint: shader_paint::ShaderProgram,
    pub instanced: ShaderProgram<GridInstance>,
}
impl GridPrograms {
    pub fn new() -> anyhow::Result<Self> {
        let common = include_str!("grid.wgsl");
        Ok(Self {
            shader_paint: shader_paint::ShaderProgram::new(&format!(
                "{common}\nfn paint(input: PaintInput) -> vec4<f32> {{ return grid_color(input.uv, parameters); }}"
            ))?,
            instanced: ShaderProgram::new(&format!(
                "{common}\nfn shade(input: ShaderInput, p: Parameters) -> vec4<f32> {{ return grid_color(input.uv, p); }}"
            ))?,
        })
    }
    pub fn paint(&self, p: GridInstance) -> anyhow::Result<shader_paint::ShaderPaint> {
        self.shader_paint.paint(
            shader_paint::ShaderParameters::new()
                .float("grid_dimensions", p.grid_dimensions)
                .float("cell_origin_x", p.cell_origin_x)
                .float("cell_origin_y", p.cell_origin_y)
                .float("line_width", p.line_width),
        )
    }
}

#[derive(Clone, Copy)]
struct Settings {
    grid: GridInstance,
    pulse_width: f32,
    pulse_speed: f32,
    pulse: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self { grid: GridInstance::default(), pulse_width: 2., pulse_speed: 10., pulse: true }
    }
}
impl Settings {
    fn frame(self, time: f32) -> GridInstance {
        let mut grid = self.grid;
        if self.pulse {
            // Upstream main.ts and utils/interpolate.ts: cosine interpolation.
            let phase = time - time / 10. * (self.pulse_speed + 10.);
            let mix = (1. - (phase * std::f32::consts::PI).cos()) * 0.5;
            grid.line_width = grid.line_width * (1. - mix) + self.pulse_width * mix;
        }
        grid
    }
    fn values(self) -> [f32; 6] {
        [
            self.grid.grid_dimensions,
            self.grid.cell_origin_x,
            self.grid.cell_origin_y,
            self.grid.line_width,
            self.pulse_width,
            self.pulse_speed,
        ]
    }
    fn edit(&mut self, index: usize, value: f32) {
        match index {
            0 => self.grid.grid_dimensions = value.round(),
            1 => self.grid.cell_origin_x = value,
            2 => self.grid.cell_origin_y = value,
            3 => self.grid.line_width = value,
            4 => self.pulse_width = value,
            _ => self.pulse_speed = value,
        }
    }
}
const CONTROLS: [(&str, f32, f32, f32); 6] = [
    ("Grid dimensions", 1., 100., 1.),
    ("Cell origin X", -1., 1., 0.1),
    ("Cell origin Y", -1., 1., 0.1),
    ("Line width", 1., 8., 0.1),
    ("Pulse width", 2., 9., 0.1),
    ("Pulse speed", 0., 100., 10.),
];
pub struct GridPane {
    look: Arc<Look>,
    programs: anyhow::Result<GridPrograms>,
    settings: Settings,
    sliders: Vec<Slider>,
    buttons: Vec<Entity<Button>>,
    active: bool,
    time: f32,
    last: Instant,
}
impl GridPane {
    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let sliders = CONTROLS
            .iter()
            .enumerate()
            .map(|(i, (_, min, max, step))| {
                radix::Slider::new(format!("grid-{i}"))
                    .look(look.as_ref())
                    .range(*min..*max)
                    .step(*step)
                    .on_value_changed(cx, move |this: &mut Self, _, value, cx| {
                        this.settings.edit(i, value);
                        cx.notify();
                    })
                    .spawn(cx)
            })
            .collect();
        let buttons = ["Toggle pulse", "Reset", "Original source · Christian Helgeson"]
            .iter()
            .enumerate()
            .map(|(i, label)| {
                radix::Button::new(format!("grid-action-{i}"))
                    .look(look.as_ref())
                    .outline()
                    .label(*label)
                    .on_click(cx, move |this: &mut Self, cx| {
                        match i {
                            0 => {
                                this.settings.pulse = !this.settings.pulse;
                                this.last = Instant::now();
                            }
                            1 => {
                                this.settings = Settings::default();
                                this.time = 0.;
                                this.sync(cx);
                            }
                            _ => cx.open_url("https://github.com/cmhhelgeson/WGSL_Shader_Depot"),
                        }
                        cx.notify();
                    })
                    .spawn(cx)
            })
            .collect();
        let pane = Self {
            look,
            programs: GridPrograms::new(),
            settings: Settings::default(),
            sliders,
            buttons,
            active: false,
            time: 0.,
            last: Instant::now(),
        };
        pane.sync(cx);
        pane
    }
    fn sync(&self, cx: &mut Context<Self>) {
        for (slider, value) in self.sliders.iter().zip(self.settings.values()) {
            slider.update(cx, |slider, cx| slider.set_value(value, cx));
        }
    }
    pub fn activate(&mut self) {
        if !self.active {
            self.last = Instant::now();
        }
        self.active = true;
    }
    pub fn suspend(&mut self) {
        self.active = false;
    }
    fn preview(&self, p: GridInstance) -> anyhow::Result<AnyElement> {
        let programs = self.programs.as_ref().map_err(|e| anyhow::anyhow!("{e:#}"))?;
        let paint = programs.paint(p)?;
        let typed = programs.instanced.clone();
        Ok(div()
            .flex()
            .flex_wrap()
            .gap_4()
            .child(
                div()
                    .flex_1()
                    .min_w(px(280.))
                    .child("Shader paint · declarative paint")
                    .child(painted().w_full().h(px(280.)).bg_paint(paint)),
            )
            .child(
                div().flex_1().min_w(px(280.)).child("Instanced shader · typed main-pass instance").child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            if let Err(e) = window.paint_custom_shader(bounds, &typed, p) {
                                tracing::error!("Grid: {e:#}");
                            }
                        },
                    )
                    .w_full()
                    .h(px(280.)),
                ),
            )
            .into_any_element())
    }
}
impl Render for GridPane {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        if self.active && self.settings.pulse {
            self.time += now.duration_since(self.last).as_secs_f32().min(0.1);
            window.request_animation_frame();
        }
        self.last = now;
        let p = self.settings.frame(self.time);
        div().id("shader-depot-grid").size_full().overflow_y_scroll().p_4().flex().flex_col().gap_4()
            .text_color(self.look.chrome().body_text)
            .child("Shader Depot: Grid · Christian Helgeson")
            .child(div().flex().gap_2().children(self.buttons.iter().cloned()))
            .child(format!("Pulse {} · effective line width {:.2} · 16-byte instance", if self.settings.pulse { "on" } else { "off" }, p.line_width))
            .child(div().flex().flex_wrap().gap_4().children(self.sliders.iter().enumerate().map(|(i, slider)| {
                div().w(px(180.)).flex().flex_col().gap_2()
                    .child(format!("{}: {:.1}", CONTROLS[i].0, self.settings.values()[i])).child(slider.clone())
            })))
            .child(match self.preview(p) { Ok(view) => view, Err(e) => div().child(format!("Grid shader error: {e:#}")).into_any_element() })
            .child("Adapted from WGSL Shader Depot · Copyright 2019 WebGPU Samples Contributors · BSD-3-Clause (licenses/ShaderDepot-BSD-3-Clause.txt)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn public_apis_accept_grid_and_validate_layout_and_parameters() {
        assert_eq!(std::mem::size_of::<GridInstance>(), 16);
        assert_eq!(GridInstance::FIELDS.iter().map(|f| f.offset).collect::<Vec<_>>(), [0, 4, 8, 12]);
        let programs = GridPrograms::new().unwrap();
        programs.paint(GridInstance::default()).unwrap();
        assert!(programs.shader_paint.paint(shader_paint::ShaderParameters::new()).is_err());
        assert!(programs.paint(GridInstance { line_width: f32::NAN, ..Default::default() }).is_err());
        let wrong = include_str!("grid.wgsl").replace("grid_dimensions: f32", "dimensions: f32");
        assert!(
            ShaderProgram::<GridInstance>::new(&format!(
                "{wrong}\nfn shade(i: ShaderInput, p: Parameters) -> vec4<f32> {{ return grid_color(i.uv, p); }}"
            ))
            .is_err()
        );
    }
    #[::core::prelude::v1::test]
    fn pulse_matches_upstream_endpoints_and_pause() {
        let mut s = Settings::default();
        assert_eq!(s.frame(0.).line_width, 1.);
        assert_eq!(s.frame(1.).line_width, 2.);
        s.pulse = false;
        assert_eq!(s.frame(1.).line_width, 1.);
        s.edit(0, 23.4);
        assert_eq!(s.grid.grid_dimensions, 23.);
    }
    #[cfg(feature = "test-support")]
    #[::core::prelude::v1::test]
    fn controls_edit_reset_and_suspend_retained_grid_state() {
        use gpui_luma::controls::button::ButtonEvent;
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (pane, cx) = app.add_window_view(|_, cx| GridPane::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        let slider = cx.update(|_, cx| pane.read(cx).sliders[0].clone());
        slider.update(cx, |slider, cx| slider.set_thumb_position(slider.primary_thumb_id(), 0.5, cx));
        cx.run_until_parked();
        cx.update(|_, cx| assert_eq!(pane.read(cx).settings.grid.grid_dimensions, 51.));
        let toggle = cx.update(|_, cx| pane.read(cx).buttons[0].clone());
        toggle.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        cx.update(|_, cx| assert!(!pane.read(cx).settings.pulse));
        let reset = cx.update(|_, cx| pane.read(cx).buttons[1].clone());
        reset.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        cx.run_until_parked();
        pane.update(cx, |pane, _| {
            assert_eq!(pane.settings.values(), Settings::default().values());
            assert!(pane.settings.pulse);
            pane.activate();
            assert!(pane.active);
            pane.suspend();
            assert!(!pane.active);
            assert!(pane.settings.pulse, "suspension preserves the pulse preference");
        });
    }
}
