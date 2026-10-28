use super::*;
use gpui::{AnyElement, IntoElement, ParentElement, Render, Rgba, Styled, Window, div, prelude::*, px, rgb};
use xgpui_shader::{Gradient, GradientStop, painted};
use crate::theme::ColorVizLookExt;

fn preview(color: LinearRgb, policy: GamutMapping, alpha: f64) -> anyhow::Result<[f32; 4]> {
    let [r, g, b] = color.map_gamut(RgbSpace::Srgb, policy)?.encode()?.components();
    Ok([r as f32, g as f32, b as f32, alpha as f32])
}

fn swatch(color: [f32; 4]) -> AnyElement {
    let [r, g, b, a] = color;
    div().w_full().h(px(64.)).bg(Rgba { r, g, b, a }).into_any_element()
}

fn row(label: impl Into<SharedString>, colors: &[[f32; 4]], outside: &[bool]) -> anyhow::Result<AnyElement> {
    // Same display path as extensions-web/color.rs: shader interpolation between CPU-mapped samples.
    let display = if colors.len() <= 32 {
        let stops = colors
            .iter()
            .enumerate()
            .map(|(i, color)| GradientStop::new(i as f32 / (colors.len() - 1) as f32, *color))
            .collect();
        let paint = Arc::new(Gradient::linear(90., stops)?);
        painted().w_full().h(px(64.)).rounded_md().bg_paint(paint).into_any_element()
    } else {
        // The shader producer accepts at most 32 stops. Preserve every CPU sample
        // at higher resolutions instead of silently truncating the reference.
        div()
            .flex()
            .w_full()
            .h(px(64.))
            .children(colors.iter().map(|&[r, g, b, a]| div().flex_1().min_w_0().h_full().bg(Rgba { r, g, b, a })))
            .into_any_element()
    };
    Ok(div()
        .flex()
        .flex_col()
        .gap_2()
        .child(label.into())
        .child(display)
        .child(
            div().flex().h(px(5.)).w_full().children(
                outside
                    .iter()
                    .map(|outside| div().flex_1().h_full().bg(rgb(if *outside { 0xfbbf24 } else { 0x475569 }))),
            ),
        )
        .into_any_element())
}

fn triple(values: [f64; 3]) -> String {
    format!("[{:.6}, {:.6}, {:.6}]", values[0], values[1], values[2])
}

impl Render for ColorPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_choices(_cx);
        let actions: &[Action] = match self.active_tab.as_ref() {
            "Color Spaces" => &[Action::Source, Action::Reset],
            "Conversions" => &[Action::Source, Action::Destination, Action::Reset],
            "Interpolation" => &[Action::Preset, Action::Mapping, Action::Alpha, Action::Hue],
            "Gamut Mapping" => &[Action::Source, Action::Destination, Action::Preset, Action::Alpha, Action::Reset],
            _ => &[
                Action::Destination,
                Action::Preset,
                Action::Mapping,
                Action::Alpha,
                Action::Model,
                Action::Hue,
                Action::Samples,
                Action::Reset,
            ],
        };
        let parameters: &[Parameter] = match self.active_tab.as_ref() {
            "Color Spaces" | "Conversions" | "Gamut Mapping" => &[Parameter::Red, Parameter::Green, Parameter::Blue],
            "CPU Gradients" => &[Parameter::Position, Parameter::AxisX, Parameter::AxisY],
            _ => &[],
        };
        let result = match self.active_tab.as_ref() {
            "Color Spaces" => self.spaces(),
            "Conversions" => self.conversions(),
            "Interpolation" => self.interpolation(),
            "Gamut Mapping" => self.mapping(),
            _ => self.cpu_gradients(),
        };
        div()
            .id("color-lab-scroll")
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
                                    && !(*action == Action::Hue
                                        && self.active_tab == "CPU Gradients"
                                        && self.model_index != 2)
                            })
                            .map(|(_, button)| button.clone()),
                    )
                    .children(
                        self.choices
                            .iter()
                            .filter(|(action, _)| {
                                actions.contains(action)
                                    && !(*action == Action::Hue
                                        && self.active_tab == "CPU Gradients"
                                        && self.model_index != 2)
                            })
                            .map(|(_, choice)| choice.clone()),
                    )
                    .when(actions.contains(&Action::Alpha), |row| row.child(self.transparency.clone())),
            )
            .child(div().flex().flex_wrap().gap_4().children(
                self.sliders.iter().filter(|(parameter, _)| parameters.contains(parameter)).map(
                    |(parameter, slider)| {
                        let value = match parameter {
                            Parameter::Red => self.input[0],
                            Parameter::Green => self.input[1],
                            Parameter::Blue => self.input[2],
                            Parameter::Position => self.position,
                            Parameter::AxisX => self.axis[0],
                            Parameter::AxisY => self.axis[1],
                        };
                        div()
                            .w(px(200.))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(format!("{}: {value:.3}", parameter.config().0))
                            .child(slider.clone())
                    },
                ),
            ))
            .child("CPU color reference · mapped sRGB previews · numerical conversions retain extended values")
            .child(match result {
                Ok(view) => view,
                Err(error) => div().child(format!("Color lab unavailable: {error:#}")).into_any_element(),
            })
    }
}

impl ColorPage {
    fn spaces(&self) -> anyhow::Result<AnyElement> {
        let mut view = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(format!("Same encoded components {} interpreted in three RGB spaces", triple(self.input)));
        for space in SPACES {
            let linear = EncodedRgb::new(space, self.input)?.decode()?;
            view = view.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(format!(
                        "{space:?} · linear RGB {} · XYZ D65 {}",
                        triple(linear.components()),
                        triple(linear.to_xyz()?.components())
                    ))
                    .child(swatch(preview(linear, GamutMapping::ReduceChroma, 1.)?)),
            );
        }
        let selected = SPACES[self.source_index];
        view = view.child(format!("{selected:?} · linear RGB → XYZ D65 matrix"));
        for values in selected.linear_to_xyz_matrix() {
            view = view.child(triple(values));
        }
        Ok(view.child("sRGB and Display P3 share a transfer function; Rec.2020 uses its SDR transfer. All three use a D65 white point. Previews use chroma reduction into sRGB.").into_any_element())
    }

    fn conversions(&self) -> anyhow::Result<AnyElement> {
        let source = SPACES[self.source_index];
        let destination = SPACES[self.destination_index];
        let linear = self.input_color()?;
        let converted = linear.convert(destination)?;
        let lab = linear.to_oklab()?;
        let roundtrip = converted.convert(source)?.encode()?.components();
        let error = roundtrip.into_iter().zip(self.input).map(|(a, b)| (a - b).abs()).fold(0., f64::max);
        Ok(div()
            .flex()
            .flex_col()
            .gap_3()
            .child(format!("{source:?} → {destination:?} · conversion without clipping"))
            .child(format!("Encoded input: {}", triple(self.input)))
            .child(format!("Decoded source: {}", triple(linear.components())))
            .child(format!("XYZ D65: {}", triple(linear.to_xyz()?.components())))
            .child(format!("OkLab: {}", triple(lab.components())))
            .child(format!("OkLCh (hue in degrees): {}", triple(lab.to_oklch()?.components())))
            .child(format!(
                "Destination linear: {} · In gamut: {}",
                triple(converted.components()),
                converted.is_in_gamut()
            ))
            .child(format!("Destination encoded: {}", triple(converted.encode()?.components())))
            .child(format!("Source round trip: {} · Max encoded error: {error:.3e}", triple(roundtrip)))
            .child("sRGB preview of the converted color (chroma reduction for display only)")
            .child(swatch(preview(converted, GamutMapping::ReduceChroma, 1.)?))
            .into_any_element())
    }

    fn interpolation(&self) -> anyhow::Result<AnyElement> {
        let rows = reference::rows(self.preset, self.policy, self.translucent)?;
        let mut view = div().flex().flex_col().gap_4().child(format!(
            "{} · {:?} · End alpha {} · OkLCh hue {:?}",
            reference::PRESETS[self.preset],
            self.policy,
            if self.translucent { "25%" } else { "100%" },
            HUES[self.hue_index]
        ));
        for (index, sample) in rows.iter().enumerate() {
            if index != 3 {
                view = view.child(row(sample.label, &sample.rgba, &sample.outside)?);
            } else {
                let gradient = LinearGradient::new(
                    [0., 0.],
                    [1., 0.],
                    self.endpoints()?,
                    RgbSpace::Srgb,
                    ColorInterpolation::Oklch(HUES[self.hue_index]),
                )?;
                let mut colors = Vec::new();
                let mut outside = Vec::new();
                for i in 0..reference::SAMPLE_COUNT {
                    let color = gradient.sample_at(i as f64 / (reference::SAMPLE_COUNT - 1) as f64)?;
                    outside.push(!color.rgb().is_in_gamut());
                    colors.push(preview(color.rgb(), self.policy, color.alpha())?);
                }
                view = view.child(row(format!("OkLCh · {:?} hue", HUES[self.hue_index]), &colors, &outside)?);
            }
        }
        Ok(view.child("32 CPU reference samples per row. Amber marks source samples outside sRGB before mapping. Display paints interpolate between the mapped samples; color interpolation uses premultiplied components.").into_any_element())
    }

    fn mapping(&self) -> anyhow::Result<AnyElement> {
        let destination = SPACES[self.destination_index];
        let input = self.input_color()?;
        let extended = input.convert(destination)?;
        let mut view = div().flex().flex_col().gap_4().child(format!(
            "{:?} → {destination:?} · Unmapped linear RGB {} · In gamut: {}",
            SPACES[self.source_index],
            triple(extended.components()),
            extended.is_in_gamut()
        ));
        for policy in [GamutMapping::Clip, GamutMapping::ReduceChroma] {
            let mapped = input.map_gamut(destination, policy)?;
            view = view
                .child(format!("{policy:?} · {destination:?} linear {}", triple(mapped.components())))
                .child(swatch(preview(mapped, GamutMapping::ReduceChroma, 1.)?));
        }
        view =
            view.child(format!("Shared color-lab reference: {} · sRGB destination", reference::PRESETS[self.preset]));
        let clip = reference::rows(self.preset, GamutMapping::Clip, self.translucent)?;
        let reduced = reference::rows(self.preset, GamutMapping::ReduceChroma, self.translucent)?;
        for (clip, reduced) in clip.iter().zip(&reduced) {
            view = view.child(row(format!("{} · Clip", clip.label), &clip.rgba, &clip.outside)?).child(row(
                format!("{} · Reduce chroma", reduced.label),
                &reduced.rgba,
                &reduced.outside,
            )?);
        }
        Ok(view.into_any_element())
    }

    pub(super) fn cpu_gradients(&self) -> anyhow::Result<AnyElement> {
        let destination = SPACES[self.destination_index];
        let gradient = self.gradient(self.model())?;
        let count = [32, 64, 128][self.sample_count_index];
        let sample_position = [self.axis[0] * self.position, self.axis[1] * self.position];
        let sample = gradient.sample(sample_position)?;
        let mapped = sample.map_gamut(destination, self.policy)?;
        let mut colors = Vec::new();
        let mut outside = Vec::new();
        for i in 0..count {
            let t = i as f64 / (count - 1) as f64;
            let color = gradient.sample([self.axis[0] * t, self.axis[1] * t])?;
            outside.push(!color.rgb().is_in_gamut());
            colors.push(preview(color.map_gamut(destination, self.policy)?.rgb(), self.policy, color.alpha())?);
        }
        Ok(div().flex().flex_col().gap_3()
            .child(format!("{} · {:?} · Output {destination:?} · {:?} · {count} samples", reference::PRESETS[self.preset], self.model(), self.policy))
            .child(format!("Axis [0, 0] → [{:.1}, {:.1}] · Sample point [{:.2}, {:.2}] · t = {:.3}", self.axis[0],self.axis[1],sample_position[0],sample_position[1],self.position))
            .child(row("CPU-sampled gradient, mapped into the selected output space then previewed in sRGB", &colors,&outside)?)
            .child(format!("Unmapped sample (linear {destination:?}): {} · Alpha {:.4}", triple(sample.rgb().components()),sample.alpha()))
            .child(format!("Mapped sample: {} · In gamut: {}", triple(mapped.rgb().components()),mapped.rgb().is_in_gamut()))
            .child(format!("Premultiplied output: {:?}", mapped.premultiplied()))
            .child(swatch(preview(mapped.rgb(),self.policy,mapped.alpha())?))
            .child("Outside-axis positions extend endpoint colors. A coincident axis selects the second endpoint. Sampling is CPU reference math; these previews do not measure GPU performance.").into_any_element())
    }
}
