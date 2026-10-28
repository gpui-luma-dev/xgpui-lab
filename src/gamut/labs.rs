//! Embedded adaptations of xgpui/apps/gamut-lab/{main,color_main}.rs.
use super::*;
use gpui::{AnyElement, IntoElement, rgba, rgb};
use xgpui_gamut::{
    color::{ColorInterpolation, EncodedRgb, HueInterpolation, LinearRgba},
    instanced::WideGradient,
};

fn strip(colors: &[[f32; 4]], p3: bool) -> anyhow::Result<AnyElement> {
    let cells = colors
        .iter()
        .map(|&[r, g, b, a]| {
            let cell = div().flex_1().min_w_0().h_full();
            if p3 {
                let [r, g, b] =
                    EncodedRgb::new(RgbSpace::DisplayP3, [r as f64, g as f64, b as f64])?.decode()?.components();
                let color = Arc::new(WideColor::linear_p3([r as f32, g as f32, b as f32, a])?);
                Ok(cell
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                window.paint_custom(
                                    CustomGeometry::RoundedRect { bounds, corner_radii: Default::default() },
                                    color.clone(),
                                );
                            },
                        )
                        .size_full(),
                    )
                    .into_any_element())
            } else {
                Ok(cell.bg(Rgba { r, g, b, a }).into_any_element())
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(div().flex().w_full().h(px(48.0)).children(cells).into_any_element())
}

impl GamutPage {
    pub(super) fn lab_content(&self, wide: bool, _window: &mut Window, _cx: &mut Context<Self>) -> AnyElement {
        self.sync_choices(_cx);
        let presentation = self.active_tab == "Presentation";
        let actions: &[Action] = if presentation {
            &[
                Action::Presentation,
                Action::Overlay,
                Action::Opacity,
                Action::Order,
                Action::Background,
                Action::Route,
            ]
        } else if self.active_tab == "Gamut Mapping" {
            &[Action::Preset, Action::Alpha]
        } else {
            &[Action::Preset, Action::Mapping, Action::Alpha]
        };
        let mut content = div().flex().flex_col().gap_4().child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .children(
                    self.controls
                        .iter()
                        .filter(|(action, _)| actions.contains(action))
                        .map(|(_, button)| button.clone()),
                )
                .children(
                    self.choices
                        .iter()
                        .filter(|(action, _)| actions.contains(action))
                        .map(|(_, choice)| choice.clone()),
                )
                .children(
                    self.switches
                        .iter()
                        .filter(|(action, _)| actions.contains(action))
                        .map(|(_, switch)| switch.clone()),
                ),
        );
        let result = match self.active_tab.as_ref() {
            "Wide Gradients" => self.gradients(wide),
            "Gamut Mapping" => self.mapping(wide),
            "Presentation" => self.presentation(wide),
            _ => Ok(div().into_any_element()),
        };
        if !presentation {
            content = content.child(format!(
                "{} · {} · End alpha: {}",
                reference::PRESETS[self.preset],
                if self.active_tab == "Gamut Mapping" {
                    "Clip vs ReduceChroma".into()
                } else {
                    format!("{:?}", self.policy)
                },
                if self.translucent { "25%" } else { "100%" }
            ));
        }
        match result {
            Ok(view) => content.child(view).into_any_element(),
            Err(error) => {
                self.paint_errors.borrow_mut().push(format!("{error:#}"));
                content.child(format!("Lab unavailable: {error:#}")).into_any_element()
            }
        }
    }

    fn gradients(&self, wide: bool) -> anyhow::Result<AnyElement> {
        // Same endpoint presets and mapping policies as color-lab.
        let srgb = reference::rows(self.preset, self.policy, self.translucent)?;
        let p3 = reference::rows_in_space(self.preset, self.policy, self.translucent, RgbSpace::DisplayP3)?;
        let mut view =
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child("Wide gradients · gamut-lab native instanced path")
                .child(if wide {
                    "Native P3 gradients above their sRGB reference samples"
                } else {
                    "sRGB reference previews — P3 output disabled or unavailable"
                });
        for (index, (srgb, p3)) in srgb.iter().zip(&p3).enumerate() {
            let mut row = div().flex().flex_col().gap_2().child(srgb.label);
            if wide && index > 0 {
                let painter = self.painter.as_ref().map_err(|error| anyhow::anyhow!("{error:#}"))?;
                let decode = |rgba: [f32; 4]| -> anyhow::Result<LinearRgba> {
                    Ok(LinearRgba::new(
                        EncodedRgb::new(RgbSpace::DisplayP3, [rgba[0] as f64, rgba[1] as f64, rgba[2] as f64])?
                            .decode()?,
                        rgba[3] as f64,
                    )?)
                };
                let model = match index {
                    1 => ColorInterpolation::LinearRgb,
                    2 => ColorInterpolation::Oklab,
                    _ => ColorInterpolation::Oklch(HueInterpolation::Shorter),
                };
                let gradient = WideGradient::linear(
                    [0.0, 0.0],
                    [1.0, 0.0],
                    [decode(p3.rgba[0])?, decode(p3.rgba[reference::SAMPLE_COUNT - 1])?],
                    model,
                    self.policy,
                )?;
                let painter = painter.clone();
                row = row.child(
                    div().w_full().h(px(48.0)).child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                if let Err(error) = painter.paint_gradient(window, bounds, px(0.0), &gradient) {
                                    eprintln!("Gamut gradient paint failed: {error:#}");
                                }
                            },
                        )
                        .size_full(),
                    ),
                );
            } else {
                row = row.child(strip(if wide { &p3.rgba } else { &srgb.rgba }, wide)?);
            }
            view = view.child(row.child("sRGB reference (32 samples)").child(strip(&srgb.rgba, false)?));
        }
        Ok(view.into_any_element())
    }

    fn mapping(&self, wide: bool) -> anyhow::Result<AnyElement> {
        let mut view=div().flex().flex_col().gap_3()
            .child("Display P3 output mapping · native shader versus CPU reference")
            .child("Color → Gamut Mapping explores numeric RGB conversions and sRGB references. This page validates P3 shader output and compares it with the mapped P3 reference.");
        for policy in [GamutMapping::Clip, GamutMapping::ReduceChroma] {
            let rows = reference::rows_in_space(self.preset, policy, self.translucent, RgbSpace::DisplayP3)?;
            for (index, row) in rows.iter().enumerate().skip(1) {
                view = view.child(format!(
                    "{} · {policy:?} · {} source samples outside Display P3",
                    row.label,
                    row.outside.iter().filter(|v| **v).count()
                ));
                if wide {
                    let decode = |rgba: [f32; 4]| -> anyhow::Result<LinearRgba> {
                        Ok(LinearRgba::new(
                            EncodedRgb::new(RgbSpace::DisplayP3, [rgba[0] as f64, rgba[1] as f64, rgba[2] as f64])?
                                .decode()?,
                            rgba[3] as f64,
                        )?)
                    };
                    let model = match index {
                        1 => ColorInterpolation::LinearRgb,
                        2 => ColorInterpolation::Oklab,
                        _ => ColorInterpolation::Oklch(HueInterpolation::Shorter),
                    };
                    let gradient = WideGradient::linear(
                        [0., 0.],
                        [1., 0.],
                        [decode(row.rgba[0])?, decode(*row.rgba.last().unwrap())?],
                        model,
                        policy,
                    )?;
                    let painter = self.painter.as_ref().map_err(|e| anyhow::anyhow!("{e:#}"))?.clone();
                    let paint_errors = self.paint_errors.clone();
                    view = view
                        .child(
                            div().h(px(48.)).child(
                                canvas(
                                    |_, _, _| (),
                                    move |bounds, _, window, _| {
                                        if let Err(error) = painter.paint_gradient(window, bounds, px(0.), &gradient) {
                                            paint_errors.borrow_mut().push(format!("P3 mapping: {error:#}"));
                                            eprintln!("P3 mapping: {error:#}");
                                        }
                                    },
                                )
                                .size_full(),
                            ),
                        )
                        .child("CPU P3 reference samples")
                        .child(strip(&row.rgba, true)?);
                } else {
                    let colors = row
                        .rgba
                        .iter()
                        .map(|rgba| -> anyhow::Result<[f32; 4]> {
                            let rgb =
                                EncodedRgb::new(RgbSpace::DisplayP3, [rgba[0] as f64, rgba[1] as f64, rgba[2] as f64])?
                                    .decode()?
                                    .map_gamut(RgbSpace::Srgb, GamutMapping::ReduceChroma)?
                                    .encode()?
                                    .components();
                            Ok([rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, rgba[3]])
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    view = view
                        .child("P3 → sRGB preview; enable P3 presentation to validate native output")
                        .child(strip(&colors, false)?);
                }
            }
        }
        Ok(view.into_any_element())
    }

    fn presentation(&self, wide: bool) -> anyhow::Result<AnyElement> {
        let opacity = if self.half_opacity { 0.5 } else { 1.0 };
        let surface = |left: f32, top: f32, source: [f32; 4], fallback| -> anyhow::Result<AnyElement> {
            let color = Arc::new(WideColor::linear_p3(source)?);
            let route = self.route;
            let painter = if wide && route > 0 {
                Some(self.painter.as_ref().map_err(|error| anyhow::anyhow!("{error:#}"))?.clone())
            } else {
                None
            };
            let paint_errors = self.paint_errors.clone();
            let endpoints = [source, [source[2], source[0], source[1], source[3]]].map(|rgba| {
                LinearRgba::new(
                    LinearRgb::new(RgbSpace::DisplayP3, [rgba[0] as f64, rgba[1] as f64, rgba[2] as f64]).unwrap(),
                    rgba[3] as f64,
                )
                .unwrap()
            });
            let gradient = WideGradient::linear(
                [0., 0.],
                [1., 0.],
                endpoints,
                ColorInterpolation::Oklab,
                GamutMapping::ReduceChroma,
            )?;
            Ok(div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(360.0))
                .h(px(220.0))
                .opacity(opacity)
                .when(!wide, |view| view.bg(rgb(fallback)))
                .when(wide, |view| {
                    view.child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                if route > 0
                                    && let Some(painter) = &painter
                                {
                                    let result = if route == 2 {
                                        painter.paint_gradient(window, bounds, px(0.), &gradient)
                                    } else {
                                        painter.paint_color(window, bounds, px(0.), &color)
                                    };
                                    if let Err(error) = result {
                                        paint_errors.borrow_mut().push(format!("P3 comparison route: {error:#}"));
                                        eprintln!("P3 comparison route: {error:#}");
                                    }
                                } else {
                                    window.paint_custom(
                                        CustomGeometry::RoundedRect { bounds, corner_radii: Default::default() },
                                        color.clone(),
                                    );
                                }
                            },
                        )
                        .size_full(),
                    )
                })
                .into_any_element())
        };
        let red = surface(25.0, 25.0, [1.0, 0.0, 0.0, 1.0], 0xff0000)?;
        let green = surface(200.0, 155.0, [0.0, 1.0, 0.0, 1.0], 0x00ff00)?;
        let (first, second) = if self.reverse { (green, red) } else { (red, green) };
        let scene = div()
            .relative()
            .w(px(600.0))
            .h(px(400.0))
            .overflow_hidden()
            .bg(rgb(if self.dark { 0x101010 } else { 0xf0f0f0 }))
            .child(first)
            .when(self.overlay, |view| {
                view.child(
                    div()
                        .absolute()
                        .left(px(90.0))
                        .top(px(110.0))
                        .w(px(420.0))
                        .p_4()
                        .bg(rgba(0xffffff80))
                        .text_color(rgb(0))
                        .child("sRGB overlay between P3 surfaces"),
                )
            })
            .child(second)
            .when(self.overlay, |view| {
                view.child(
                    div()
                        .absolute()
                        .left(px(300.0))
                        .top(px(270.0))
                        .p_3()
                        .bg(rgba(0x172554d0))
                        .text_color(rgb(0xffffff))
                        .child("sRGB overlay above both surfaces"),
                )
            });
        Ok(div()
            .flex()
            .flex_col()
            .gap_3()
            .child("Presentation · gamut-lab ordered compositing")
            .child(format!(
                "Route: {}",
                ["WideColor texture", "Instanced P3 solid", "Instanced P3 OkLab gradient"][self.route]
            ))
            .child(format!(
                "Output: {} · Overlays: {} · Opacity: {opacity} · Reversed: {}",
                if wide { "P3" } else { "sRGB baseline" },
                self.overlay,
                self.reverse
            ))
            .child(div().id("gamut-composition-scroll").overflow_x_scroll().child(scene))
            .into_any_element())
    }
}
