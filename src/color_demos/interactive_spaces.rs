//! A retained, interactive successor to the original Color Spaces reference page.
use std::sync::Arc;

use gpui::{prelude::*, *};
use gpui_luma::{
    controls::{slider::Slider, textfield::TextField},
};
use gpui_luma_color::{
    color_field::{ColorFieldEvent, ColorFieldState},
    color_slider::color_spec::Hsv,
};
use gpui_luma_look_radix::{self as radix, Look};
use xgpui_color::{EncodedRgb, RgbSpace};

use crate::theme::ColorVizLookExt;

const CHANNELS: [&str; 3] = ["Red", "Green", "Blue"];
const INITIAL: [f64; 3] = [0.86, 0.32, 0.58];

pub(crate) struct InteractiveSpaces {
    look: Arc<Look>,
    rgb: [f64; 3],
    hsv: Hsv,
    field: Entity<ColorFieldState>,
    inputs: Vec<TextField>,
    sliders: Vec<Slider>,
    saturation: Slider,
    _subscriptions: Vec<Subscription>,
}

/// One physical color, represented in three coordinate systems. No reinterpretation
/// of RGB triples and no gamut mapping is needed for an sRGB source in Display P3.
struct Coordinates {
    p3: [f64; 3],
    oklch: [f64; 3],
}
impl Coordinates {
    fn new(rgb: [f64; 3]) -> Result<Self, xgpui_color::ColorError> {
        let source = EncodedRgb::new(RgbSpace::Srgb, rgb)?.decode()?;
        Ok(Self {
            p3: source.convert(RgbSpace::DisplayP3)?.encode()?.components(),
            oklch: source.to_oklab()?.to_oklch()?.components(),
        })
    }
}

fn rgba(rgb: [f64; 3]) -> Rgba {
    Rgba { r: rgb[0] as f32, g: rgb[1] as f32, b: rgb[2] as f32, a: 1. }
}
fn hex(rgb: [f64; 3]) -> String {
    let [r, g, b] = rgb.map(|value| (value.clamp(0., 1.) * 255.).round() as u8);
    format!("#{r:02X}{g:02X}{b:02X}")
}

impl InteractiveSpaces {
    pub(crate) fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let hsv = Hsv::from_rgba(rgba(INITIAL));
        let field = cx.new(|_| {
            ColorFieldState::hue_value("interactive-spaces-field", hsv, 16.)
                .raster_image()
                .rounded(px(6.))
                .inside_field()
        });
        let subscription = cx.subscribe(&field, |this: &mut Self, _, event, cx| {
            if let ColorFieldEvent::Change(hsv) | ColorFieldEvent::Release(hsv) = event {
                this.hsv = *hsv;
                let color: Rgba = hsv.to_hsla_ext().into();
                this.rgb = [color.r as f64, color.g as f64, color.b as f64];
                this.sync_controls(None, cx);
                cx.notify();
            }
        });
        let inputs = CHANNELS
            .iter()
            .enumerate()
            .map(|(index, channel)| {
                radix::TextField::new(format!("interactive-spaces-input-{index}"))
                    .look(&look)
                    .surface()
                    .size(radix::TextFieldSize::Two)
                    .placeholder(format!("{channel} 0–1"))
                    .value(format!("{:.3}", INITIAL[index]))
                    .full_width(true)
                    .select_all_on_tab_focus(true)
                    .validator(Arc::new(|text| {
                        text.trim().parse::<f64>().is_ok_and(|v| v.is_finite() && (0. ..=1.).contains(&v))
                    }))
                    .on_value_changed(cx, move |this: &mut Self, value, cx| {
                        // Preserve intermediate edits, including an empty field. Only
                        // finite, bounded values enter the shared color state.
                        if let Ok(value) = value.trim().parse::<f64>()
                            && value.is_finite()
                            && (0. ..=1.).contains(&value)
                        {
                            this.set_channel(index, value, Some(index), cx);
                        }
                    })
                    .on_focus_changed(cx, move |this: &mut Self, focused, cx| {
                        if !focused {
                            this.sync_input(index, cx);
                        }
                    })
                    .on_submit(cx, move |this: &mut Self, _, cx| this.sync_input(index, cx))
                    .spawn(cx)
            })
            .collect();
        let sliders = (0..3)
            .map(|index| {
                radix::Slider::new(format!("interactive-spaces-slider-{index}"))
                    .look(&look)
                    .size(radix::SliderSize::One)
                    .range(0. ..1.)
                    .step(0.001)
                    .value(INITIAL[index])
                    .on_value_changed(cx, move |this: &mut Self, _, value, cx| {
                        this.set_channel(index, value as f64, None, cx);
                    })
                    .spawn(cx)
            })
            .collect();
        let saturation = radix::Slider::new("interactive-spaces-saturation")
            .look(&look)
            .size(radix::SliderSize::One)
            .range(0. ..1.)
            .step(0.001)
            .value(hsv.s as f64)
            .on_value_changed(cx, |this: &mut Self, _, value, cx| {
                this.hsv.s = value;
                let color: Rgba = this.hsv.to_hsla_ext().into();
                this.rgb = [color.r as f64, color.g as f64, color.b as f64];
                this.field.update(cx, |field, cx| field.set_hsv(this.hsv, cx));
                this.sync_controls(None, cx);
                cx.notify();
            })
            .spawn(cx);
        Self { look, rgb: INITIAL, hsv, field, inputs, sliders, saturation, _subscriptions: vec![subscription] }
    }

    fn set_channel(&mut self, index: usize, value: f64, editing: Option<usize>, cx: &mut Context<Self>) {
        self.rgb[index] = value.clamp(0., 1.);
        // Retain hue at black/gray so the field remains useful at either endpoint.
        let mut hsv = Hsv::from_rgba(rgba(self.rgb));
        if hsv.s <= f32::EPSILON {
            hsv.h = self.hsv.h;
        }
        self.hsv = hsv;
        self.field.update(cx, |field, cx| field.set_hsv(hsv, cx));
        self.sync_controls(editing, cx);
        cx.notify();
    }

    fn sync_input(&self, index: usize, cx: &mut Context<Self>) {
        self.inputs[index].update(cx, |input, cx| input.set_value(format!("{:.3}", self.rgb[index]), cx));
    }

    fn sync_controls(&self, editing: Option<usize>, cx: &mut Context<Self>) {
        self.saturation.update(cx, |slider, cx| slider.set_value(self.hsv.s, cx));
        for index in 0..3 {
            if editing != Some(index) {
                self.sync_input(index, cx);
            }
            self.sliders[index].update(cx, |slider, cx| slider.set_value(self.rgb[index] as f32, cx));
        }
    }

    fn panel(&self) -> Div {
        let chrome = self.look.chrome();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .bg(chrome.panel_background)
            .border_1()
            .border_color(chrome.border)
    }

    fn caption(&self, text: impl Into<SharedString>) -> Div {
        div().text_sm().text_color(self.look.chrome().muted_text).child(text.into())
    }

    fn card(
        &self,
        title: &'static str,
        description: &'static str,
        labels: [&str; 3],
        values: [String; 3],
        notation: String,
    ) -> Div {
        self.panel()
            .flex_1()
            .min_w(px(225.))
            .flex_basis(px(265.))
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(self.caption(description))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_center()
                    .child(div().flex_1().min_w(px(70.)).h(px(94.)).rounded_md().bg(rgba(self.rgb)))
                    .child(div().w(px(100.)).flex().flex_col().gap_2().child(self.caption(notation)).children(
                        labels.into_iter().zip(values).map(|(label, value)| {
                            div().flex().justify_between().text_sm().child(label.to_owned()).child(value)
                        }),
                    )),
            )
    }
}

impl Render for InteractiveSpaces {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let chrome = self.look.chrome();
        let comparison = match Coordinates::new(self.rgb) {
            Ok(coordinates) => div()
                .flex()
                .flex_wrap()
                .gap_4()
                .child(self.card(
                    "sRGB",
                    "Standard RGB for web and general use",
                    ["R", "G", "B"],
                    self.rgb.map(|v| format!("{v:.3}")),
                    hex(self.rgb),
                ))
                .child(self.card(
                    "Display P3",
                    "Wide gamut for modern displays",
                    ["R", "G", "B"],
                    coordinates.p3.map(|v| format!("{v:.3}")),
                    "Encoded P3".into(),
                ))
                .child(self.card(
                    "OKLCH",
                    "Perceptual color space",
                    ["L", "C", "H"],
                    [
                        format!("{:.3}", coordinates.oklch[0]),
                        format!("{:.3}", coordinates.oklch[1]),
                        format!("{:.1}°", coordinates.oklch[2]),
                    ],
                    "D65".into(),
                )),
            Err(error) => div().child(format!("Color conversion unavailable: {error}")),
        };
        div().id("interactive-color-spaces").size_full().overflow_y_scroll()
            .bg(chrome.content_background).text_color(chrome.body_text)
            .child(div().w_full().max_w(px(1240.)).mx_auto().p_6().flex().flex_col().gap_5()
                .child(self.caption("xgpui-color  /  Interactive Color Spaces"))
                .child(div().flex().flex_col().gap_2()
                    .child(div().text_3xl().font_weight(FontWeight::SEMIBOLD).child("Interactive Color Spaces"))
                    .child(self.caption("Explore color models and their representations")))
                .child(self.panel().gap_4().p_5()
                    .child(div().flex().flex_col().gap_1()
                        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Color space comparison"))
                        .child(self.caption("One selected color, represented in sRGB, Display P3, and OKLCH")))
                    .child(div().debug_selector(|| "interactive-spaces-field-bounds".into()).w_full().h(px(190.)).flex_shrink_0().child(self.field.clone()))
                    .child(self.caption(format!("Drag to change hue and brightness · saturation {:.0}%", self.hsv.s * 100.)))
                    .child(div().flex().flex_wrap().items_center().gap_3()
                        .child(self.caption("Saturation"))
                        .child(div().w(px(200.)).child(self.saturation.clone()))
                        .child(self.caption("Use arrow keys for precise adjustments")))
                    .child(comparison)
                    .child(div().flex().flex_wrap().gap_4()
                        .child(self.panel().flex_grow(2.).flex_basis(px(440.)).min_w(px(240.))
                            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Color parameters"))
                            .child(self.caption("Adjust the color using encoded sRGB values (0–1)"))
                            .child(div().flex().flex_wrap().gap_4().children((0..3).map(|index| {
                                div().flex_1().min_w(px(100.)).flex().flex_col().gap_2()
                                    .child(CHANNELS[index]).child(self.inputs[index].clone()).child(self.sliders[index].clone())
                            }))))
                        .child(self.panel().flex_1().flex_basis(px(265.)).min_w(px(225.))
                            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Color preview"))
                            .child(self.caption("The selected color"))
                            .child(div().w_full().h(px(80.)).rounded_md().bg(rgba(self.rgb)))
                            .child(self.caption(hex(self.rgb)))))
                    .child(self.caption("The swatches show the same color. Display P3 coordinates are converted from sRGB; OKLCH describes lightness, chroma, and hue."))))
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::{Coordinates, INITIAL, InteractiveSpaces};
    use gpui::{TestAppContext, point, px};
    use xgpui_color::{EncodedRgb, RgbSpace};
    use gpui_luma::controls::textfield::TextFieldEvent;

    #[test]
    fn comparison_coordinates_preserve_the_selected_physical_color() {
        for rgb in [INITIAL, [0., 0., 0.], [1., 1., 1.], [1., 0., 0.], [0.12, 0.45, 0.91]] {
            let coordinates = Coordinates::new(rgb).unwrap();
            let p3_roundtrip = EncodedRgb::new(RgbSpace::DisplayP3, coordinates.p3)
                .unwrap()
                .decode()
                .unwrap()
                .convert(RgbSpace::Srgb)
                .unwrap()
                .encode()
                .unwrap()
                .components();
            let [l, c, h] = coordinates.oklch;
            let perceptual_roundtrip = xgpui_color::Oklch::new(l, c, h)
                .unwrap()
                .to_oklab()
                .unwrap()
                .to_linear(RgbSpace::Srgb)
                .unwrap()
                .encode()
                .unwrap()
                .components();
            for ((p3, oklch), source) in p3_roundtrip.into_iter().zip(perceptual_roundtrip).zip(rgb) {
                assert!((p3 - source).abs() < 1e-5);
                assert!((oklch - source).abs() < 1e-5);
            }
        }
        assert_ne!(Coordinates::new(INITIAL).unwrap().p3, INITIAL, "P3 must convert rather than reinterpret RGB");
    }

    #[test]
    fn rgb_inputs_sliders_and_field_share_state_and_reject_invalid_edits() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| InteractiveSpaces::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        let input = cx.update(|_, cx| page.read(cx).inputs[0].clone());
        for (text, expected) in [("0.125", 0.125), ("NaN", 0.125), ("1.2", 0.125), ("", 0.125)] {
            input.update(cx, |input, cx| {
                input.set_value(text, cx);
                cx.emit(TextFieldEvent::Change { value: text.into() });
            });
            cx.run_until_parked();
            cx.update(|_, cx| assert_eq!(page.read(cx).rgb[0], expected));
        }
        input.update(cx, |_, cx| cx.emit(TextFieldEvent::FocusChanged { focused: false }));
        cx.run_until_parked();
        cx.update(|_, cx| assert_eq!(input.read(cx).value(), "0.125"));
        let slider = cx.update(|_, cx| page.read(cx).sliders[1].clone());
        slider.update(cx, |slider, cx| slider.set_thumb_position(slider.primary_thumb_id(), 0.7, cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert!((page.rgb[1] - 0.7).abs() < 0.001);
            assert_eq!(page.inputs[1].read(cx).value(), "0.700");
        });
        let bounds = cx.debug_bounds("interactive-spaces-field-bounds").unwrap();
        assert!(bounds.size.width > px(400.));
        assert_eq!(bounds.size.height, px(190.));
        cx.simulate_click(
            point(bounds.left() + bounds.size.width * 0.25, bounds.top() + bounds.size.height * 0.25),
            Default::default(),
        );
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert!((page.hsv.h - 90.).abs() < 10.);
            assert!((page.hsv.v - 0.75).abs() < 0.1);
            for (index, input) in page.inputs.iter().enumerate() {
                assert_eq!(input.read(cx).value(), format!("{:.3}", page.rgb[index]));
            }
        });
        let saturation = cx.update(|_, cx| page.read(cx).saturation.clone());
        saturation.update(cx, |slider, cx| slider.set_thumb_position(slider.primary_thumb_id(), 0., cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let rgb = page.read(cx).rgb;
            assert!((rgb[0] - rgb[1]).abs() < 1e-6 && (rgb[1] - rgb[2]).abs() < 1e-6);
        });
    }
}
