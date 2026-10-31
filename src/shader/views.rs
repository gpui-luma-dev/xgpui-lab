use super::*;
use xgpui_shader::{painted, Paint, WindowPaintExt, WindowShaderExt};

impl Render for ShaderPage {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_choices(_cx);
        if self.active_tab == "Shadertoy: Seascape" {
            self.sea_pane.update(_cx, |pane, _| pane.activate());
            return div().size_full().child(self.sea_pane.clone()).into_any_element();
        }
        if self.active_tab == "Shader Depot: Grid" {
            self.grid_pane.update(_cx, |pane, _| pane.activate());
            return div().size_full().child(self.grid_pane.clone()).into_any_element();
        }
        if self.requires_sdr() && window.color_status().effective() != gpui::WindowColorMode::Default {
            return div().p_4().child("Preparing SDR presentation for captured effects…").into_any_element();
        }
        if self.active_tab == "Brushes" {
            return div()
                .id("brush-gallery-scroll")
                .size_full()
                .overflow_y_scroll()
                .p_4()
                .child(gallery::gallery(self, _cx))
                .into_any_element();
        }
        if self.active_tab == "Layer Effects" {
            return div()
                .id("layer-gallery-scroll")
                .size_full()
                .overflow_y_scroll()
                .p_4()
                .child(gallery::layers(self, _cx))
                .into_any_element();
        }
        if self.active_tab == "Mesh Editor" {
            return div().size_full().child(self.mesh_pane.clone()).into_any_element();
        }
        if matches!(self.active_tab.as_ref(), "Shadows" | "Borders") {
            self.shadow_pane.update(_cx, |page, _| page.borders = self.active_tab == "Borders");
            return div().size_full().child(self.shadow_pane.clone()).into_any_element();
        }
        if self.active_tab == "Typography" {
            if window.color_status().effective() != gpui::WindowColorMode::Default {
                return div().p_4().child("Preparing SDR presentation for typography effects…").into_any_element();
            }
            return div().size_full().child(self.typography.clone()).into_any_element();
        }
        let now = Instant::now();
        if self.animated {
            self.time += now.duration_since(self.last_frame).as_secs_f32().min(0.1);
            window.request_animation_frame();
        }
        self.last_frame = now;
        let actions: &[Action] = match self.active_tab.as_ref() {
            "Mesh Editor" => &[Action::Point, Action::Reset, Action::Opacity, Action::Clip],
            "WGSL Paints" => &[Action::Animate, Action::Opacity, Action::Clip],
            "Instancing" => &[Action::Mode, Action::Count, Action::Animate, Action::Opacity, Action::Clip],
            _ => &[
                Action::Effect,
                Action::Stop,
                Action::Add,
                Action::Duplicate,
                Action::Remove,
                Action::Reset,
                Action::Animate,
                Action::Opacity,
                Action::Clip,
                Action::Native,
            ],
        };
        let parameters: &[Parameter] = match self.active_tab.as_ref() {
            "Mesh Editor" => &[
                Parameter::X,
                Parameter::Y,
                Parameter::MeshRed,
                Parameter::MeshGreen,
                Parameter::MeshBlue,
                Parameter::U,
                Parameter::V,
            ],
            "WGSL Paints" => &[Parameter::Progress, Parameter::ShaderAlpha],
            "Instancing" => &[],
            _ => &[
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
            ],
        };
        let preview = match self.active_tab.as_ref() {
            "WGSL Paints" => self.wgsl(),
            "Instancing" => self.instances(),
            "Gradient Paints" => self.coverage(true),
            "Mesh Editor" => self.coverage(false),
            _ => self.gradients(),
        };
        div()
            .id("shader-page-scroll")
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .text_color(self.look.chrome().body_text)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .children(
                        self.buttons
                            .iter()
                            .filter(|(action, _)| {
                                actions.contains(action)
                                    && !(self.active_tab == "Instancing"
                                        && ((*action == Action::Count && self.mode != 1)
                                            || (*action == Action::Clip && self.mode != 2)))
                            })
                            .map(|(_, button)| button.clone()),
                    )
                    .children(
                        self.choices
                            .iter()
                            .filter(|(action, _)| {
                                actions.contains(action)
                                    && !(*action == Action::Count && self.active_tab == "Instancing" && self.mode != 1)
                            })
                            .map(|(_, choice)| choice.clone()),
                    ),
            )
            .child(self.status())
            .child(
                div().flex().flex_wrap().gap_4().children(
                    self.sliders
                        .iter()
                        .filter(|(parameter, _)| parameters.contains(parameter) && self.parameter_visible(*parameter))
                        .map(|(parameter, slider)| {
                            div()
                                .w(px(180.))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(format!("{}: {:.2}", parameter.config().0, self.value(*parameter)))
                                .child(slider.clone())
                        }),
                ),
            )
            .child(match preview {
                Ok(view) => view,
                Err(error) => div().child(format!("Shader lab unavailable: {error:#}")).into_any_element(),
            })
            .into_any_element()
    }
}

impl ShaderPage {
    fn parameter_visible(&self, parameter: Parameter) -> bool {
        let effect = Self::EFFECTS[self.effect_index];
        match parameter {
            Parameter::Angle => effect != Effect::Radial,
            Parameter::CenterX | Parameter::CenterY => effect != Effect::Linear,
            Parameter::RadiusX | Parameter::RadiusY => matches!(effect, Effect::Radial | Effect::Diamond),
            _ => true,
        }
    }

    fn status(&self) -> String {
        let state = format!(
            "{} · Opacity {} · Clipping {}",
            if self.animated { "Animating" } else { "Paused" },
            if self.half_opacity { "50%" } else { "100%" },
            self.clipped
        );
        match self.active_tab.as_ref() {
            "Mesh Editor" => format!("paint-lab · 3×3 Hermite mesh · Point {} of 9 · {state}", self.selected_point + 1),
            "WGSL Paints" => format!("paint-lab portable wash + shader-examples minimal WGSL · {state}"),
            "Instancing" => format!(
                "shader-examples · {} · {} instances · {state}",
                ["Glow", "Particles", "Composition"][self.mode],
                if self.mode == 0 {
                    100
                } else if self.mode == 1 {
                    [100, 1000, 5000, 10000][self.count_index]
                } else {
                    3
                }
            ),
            _ => {
                let s = &self.settings[self.effect_index];
                format!(
                    "paint-lab · {} · Stop {} of {} · Native coverage {} · {state}",
                    Self::EFFECTS[self.effect_index].label(),
                    s.selected + 1,
                    s.stops.len(),
                    self.native
                )
            }
        }
    }

    fn gradients(&self) -> anyhow::Result<AnyElement> {
        let settings = &self.settings[self.effect_index];
        let paint = if self.native {
            Paint::Native(white().into())
        } else {
            self.producer()?.into()
        };
        Ok(div()
            .flex()
            .flex_col()
            .gap_3()
            .child("Analytic gradient producer · linear, radial, conic and diamond")
            .child(
                div()
                    .id("analytic-preview")
                    .w_full()
                    .h(px(280.))
                    .overflow_hidden()
                    .opacity(if self.half_opacity { 0.5 } else { 1. })
                    .child(
                        painted()
                            .w(px(640.))
                            .h(px(280.))
                            .rounded_2xl()
                            .bg_paint(paint)
                            .when(self.clipped, |view| view.ml(px(-120.))),
                    ),
            )
            .child(div().flex().gap_2().children(settings.stops.iter().enumerate().map(|(index, stop)| {
                let [r, g, b, a] = stop.color;
                div()
                    .flex_1()
                    .h(px(56.))
                    .bg(Rgba { r, g, b, a })
                    .border_2()
                    .border_color(if index == settings.selected {
                        white()
                    } else {
                        transparent_black()
                    })
                    .child(format!("{:.2}", stop.offset))
            })))
            .into_any_element())
    }

    fn coverage(&self, brushes: bool) -> anyhow::Result<AnyElement> {
        let producer = self.producer()?;
        let paint = if self.native && brushes {
            Paint::Native(white().into())
        } else {
            producer.into()
        };
        let mut cards = div().flex().flex_wrap().gap_4();
        for (index, label) in ["Rounded background", "Dashed border", "Curved path fill"].into_iter().enumerate() {
            let mut tile = painted().w(px(280.)).h(px(210.));
            match index {
                0 => {
                    tile = tile
                        .rounded_3xl()
                        .bg_paint(paint.clone())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child("Normal GPUI children")
                }
                1 => tile = tile.rounded_3xl().border_8().border_dashed().border_paint(paint.clone()),
                _ => {
                    let paint = paint.clone();
                    tile = tile.child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                if let Ok(path) = shape(bounds) {
                                    window.paint_path_with_paint(path, paint.clone());
                                }
                            },
                        )
                        .size_full(),
                    );
                }
            }
            cards = cards.child(
                div().flex().flex_col().gap_2().child(label).child(
                    div()
                        .id(format!("shader-coverage-{index}"))
                        .w(px(if self.clipped { 200. } else { 280. }))
                        .h(px(210.))
                        .overflow_hidden()
                        .opacity(if self.half_opacity { 0.5 } else { 1. })
                        .child(tile),
                ),
            );
        }
        let view = div().flex().flex_col().gap_4().child(cards);
        Ok(view.into_any_element())
    }

    fn wgsl(&self) -> anyhow::Result<AnyElement> {
        let programs = self.programs.as_ref().map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let wash = programs.wash.paint(
            xgpui_shader::shader_paint::ShaderParameters::new()
                .float("progress", self.progress + if self.animated { 0.25 * self.time.sin() } else { 0. })
                .vec3("tint", [0.95, 0.15, 0.35])
                .float("alpha", self.shader_alpha),
        )?;
        let program = programs.minimal.clone();
        let phase = self.time * 2.;
        let alpha = self.shader_alpha;
        Ok(div()
            .flex()
            .flex_col()
            .gap_3()
            .child("Portable WGSL wash · reflected progress, tint and alpha parameters")
            .child(
                div()
                    .w_full()
                    .h(px(180.))
                    .overflow_hidden()
                    .opacity(if self.half_opacity { 0.5 } else { 1. })
                    .child(
                        painted()
                            .w(px(640.))
                            .h(px(180.))
                            .rounded_2xl()
                            .bg_paint(wash)
                            .when(self.clipped, |view| view.ml(px(-120.))),
                    ),
            )
            .child("Minimal typed fragment shader · shader-examples/minimal.wgsl")
            .child(
                div()
                    .w_full()
                    .h(px(180.))
                    .overflow_hidden()
                    .opacity(if self.half_opacity { 0.5 } else { 1. })
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                if let Err(error) = window.paint_custom_shader(
                                    bounds,
                                    &program,
                                    EffectInstance { color: [0.25, 0.65, 1., alpha], effect: [12., phase, 1., 0.] },
                                ) {
                                    eprintln!("Minimal shader paint failed: {error:#}");
                                }
                            },
                        )
                        .w(px(640.))
                        .h(px(180.))
                        .when(self.clipped, |view| view.ml(px(-120.))),
                    ),
            )
            .into_any_element())
    }

    fn instances(&self) -> anyhow::Result<AnyElement> {
        let programs = self.programs.as_ref().map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let program = match self.mode {
            0 => programs.glow.clone(),
            1 => programs.particles.clone(),
            _ => programs.rounded.clone(),
        };
        let mode = self.mode;
        let count = if mode == 0 {
            100
        } else {
            [100, 1000, 5000, 10000][self.count_index]
        };
        let time = self.time;
        let clipped = self.clipped;
        Ok(div()
            .flex()
            .flex_col()
            .gap_3()
            .child("Typed main-pass instances · native draws preserve GPUI clipping, opacity and order")
            .child("Instance counts describe the workload. Use a GPU capture to measure submissions or GPU time.")
            .child(
                div()
                    .w_full()
                    .h(px(380.))
                    .overflow_hidden()
                    .bg(rgb(0x121c32))
                    .opacity(if self.half_opacity { 0.5 } else { 1. })
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                let result = if mode == 2 {
                                    paint_composition(bounds, &program, clipped, window)
                                } else {
                                    paint_instances(bounds, &program, mode, count, time, window)
                                };
                                if let Err(error) = result {
                                    eprintln!("Instanced shader paint failed: {error:#}");
                                }
                            },
                        )
                        .size_full(),
                    ),
            )
            .into_any_element())
    }
}

fn random(seed: u32) -> f32 {
    let mut value = seed.wrapping_add(0x9e3779b9);
    value = (value ^ (value >> 16)).wrapping_mul(0x85ebca6b);
    value = (value ^ (value >> 13)).wrapping_mul(0xc2b2ae35);
    ((value ^ (value >> 16)) >> 8) as f32 / 16777216.
}

fn paint_instances(
    bounds: Bounds<Pixels>,
    program: &ShaderProgram<EffectInstance>,
    mode: usize,
    count: usize,
    t: f32,
    window: &mut Window,
) -> anyhow::Result<()> {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    for i in 0..count {
        let fi = i as f32;
        let (x, y, w, h, tint) = if mode == 0 {
            let w = width / 10.;
            let h = height / 10.;
            ((i % 10) as f32 * w, (i / 10) as f32 * h, w, h, [0.3 + 0.2 * (fi * 0.3).sin(), 0.55, 1., 1.])
        } else {
            let x = ((random(i as u32) + t * 0.025) % 1.) * width;
            let y = ((random((i as u32).wrapping_add(123456)) - t * 0.06).rem_euclid(1.)) * height;
            (x - 6., y - 6., 12., 12., [0.4 + 0.4 * (fi * 0.7).sin(), 0.65, 1., 0.8])
        };
        window.paint_custom_shader(
            Bounds::new(bounds.origin + point(px(x), px(y)), size(px(w), px(h))),
            program,
            EffectInstance { color: tint, effect: [12., t * 2. + fi * 0.1, 1., 0.] },
        )?;
    }
    Ok(())
}

fn shape(bounds: Bounds<Pixels>) -> anyhow::Result<Path<Pixels>> {
    let mut p = PathBuilder::fill();
    let w = bounds.size.width;
    let h = bounds.size.height;
    let o = bounds.origin;
    p.move_to(o + point(w * 0.5, h * 0.0));
    p.line_to(o + point(w * 0.67, h * 0.33));
    p.curve_to(o + point(w * 1.0, h * 0.5), o + point(w * 1.0, h * 0.1));
    p.line_to(o + point(w * 0.72, h * 0.68));
    p.line_to(o + point(w * 0.8, h * 1.0));
    p.line_to(o + point(w * 0.5, h * 0.8));
    p.line_to(o + point(w * 0.2, h * 1.0));
    p.line_to(o + point(w * 0.28, h * 0.68));
    p.line_to(o + point(w * 0.0, h * 0.5));
    p.line_to(o + point(w * 0.33, h * 0.33));
    p.close();
    p.build()
}

fn paint_composition(
    bounds: Bounds<Pixels>,
    program: &ShaderProgram<EffectInstance>,
    clipped: bool,
    window: &mut Window,
) -> anyhow::Result<()> {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let clip = Bounds::new(
        bounds.origin + point(px(width * 0.12), px(height * 0.16)),
        size(px(width * 0.72), px(height * 0.60)),
    );
    let geometry = |x, y, w, h| {
        Bounds::new(
            clip.origin + point(clip.size.width * x, clip.size.height * y),
            size(clip.size.width * w, clip.size.height * h),
        )
    };
    window.paint_quad(fill(clip, rgb(0x1c2b46)));
    window.with_content_mask(clipped.then_some(ContentMask { bounds: clip }), |window| -> anyhow::Result<()> {
        let effect = |color| EffectInstance { color, effect: [12., 0., 1., 0.] };
        window.paint_custom_shader(geometry(0.04, 0.12, 0.56, 0.76), program, effect([1., 0.2, 0.3, 0.65]))?;
        window.paint_quad(fill(geometry(0.36, -0.12, 0.025, 1.40), rgb(0xffdf80)));
        window.paint_custom_shader(geometry(0.30, 0.25, 0.56, 0.96), program, effect([0.15, 0.9, 0.55, 0.65]))?;
        window.paint_custom_shader(geometry(0.56, -0.10, 0.64, 0.90), program, effect([0.3, 0.5, 1., 0.65]))?;
        Ok(())
    })?;
    window.paint_quad(fill(clip, transparent_black()).border_widths(px(2.)).border_color(rgb(0x67e8f9)));
    Ok(())
}
