//! Interactive source-space comparisons using Luma controls and xgpui-color.
use std::sync::Arc;
use super::source_field::{self, Coordinates, FieldSettings, NAMES, rgba, hex};
use super::hv_field_shader::{NativeColors, Parameters};

use gpui::{
    App, Context, Div, Entity, FontWeight, IntoElement, Render, Rgba, SharedString, Subscription, Window, div,
    prelude::*, px, canvas,
};
use gpui_luma::{
    controls::{slider::Slider, switch::Switch, textfield::TextField},
    infra::ElementExt,
    prelude::HasPresenter,
    hstack, vstack,
};
use gpui_luma_color::{
    color_field::{ColorFieldBackgroundPainter, ColorFieldEvent, ColorFieldState},
    color_slider::color_spec::Hsv,
    color_slider::{
        ChannelDelegate, ColorSliderBuilder, ColorSliderDomainRenderer, GradientDelegate, primary_slider_value,
        refresh_color_slider,
    },
};
use gpui_luma_look_radix::{self as radix, Look};

use crate::theme::{ColorVizLookExt, TextSize, TypographyExt};

const CHANNELS: [&str; 3] = ["Red", "Green", "Blue"];
const INITIAL: [f64; 3] = [0.86, 0.32, 0.58];
const COLOR_FIELD_HEIGHT: f32 = 300.;
const CHANNEL_GRADIENT_SEGMENTS: usize = 16;

pub(crate) struct InteractiveSpaces {
    look: Arc<Look>,
    field_settings: FieldSettings,
    source_choice: crate::lab_controls::Choice,
    rgb: [f64; 3],
    hsv: Hsv,
    field: Entity<ColorFieldState>,
    channels: [RgbChannelControls; 3],
    saturation: Slider,
    saturation_track: Arc<ColorSliderDomainRenderer>,
    native_colors: Option<NativeColors>,
    gpu_painter: Option<ColorFieldBackgroundPainter>,
    cpu_painter: Option<ColorFieldBackgroundPainter>,
    presentation_pending: bool,
    presentation_epoch: u64,
    presentation_attempted: Option<gpui::WindowColorMode>,
    previous_presentation: Option<gpui::WindowColorMode>,
    host_window: Option<gpui::AnyWindowHandle>,
    gpu_toggle: Switch,
    contour_toggle: Switch,
    gpu_error: Option<String>,
    presentation_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

fn rgb_slider_delegate(
    colors: Option<&NativeColors>,
    rgb: [f64; 3],
    source: usize,
    channel: usize,
) -> Arc<dyn gpui_luma_color::color_slider::ColorSliderDelegate> {
    if let Some(colors) = colors {
        return colors.track(source, rgb, channel);
    }
    let samples = (0..=CHANNEL_GRADIENT_SEGMENTS)
        .map(|step| {
            let mut sample = rgb;
            sample[channel] = step as f64 / CHANNEL_GRADIENT_SEGMENTS as f64;
            let color =
                source_field::srgb_reference(sample, source_field::SPACES[source]).unwrap_or_else(|_| rgba(sample));
            gpui_luma::color::ColorValue::srgb(color.r, color.g, color.b, 1.)
        })
        .collect();
    Arc::new(GradientDelegate::from_colors(samples).expect("valid channel gradient"))
}

struct RgbChannelControls {
    input: TextField,
    slider: Slider,
    track: Arc<ColorSliderDomainRenderer>,
}

fn parse_channel(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|value| value.is_finite() && (0. ..=1.).contains(value))
}

impl RgbChannelControls {
    fn new(
        index: usize,
        look: &Look,
        colors: Option<&NativeColors>,
        cx: &mut Context<InteractiveSpaces>,
    ) -> (Self, Subscription) {
        let input = radix::TextField::new(format!("interactive-spaces-input-{index}"))
            .look(look)
            .surface()
            .size(radix::TextFieldSize::Two)
            .placeholder(format!("{} 0–1", CHANNELS[index]))
            .value(format!("{:.3}", INITIAL[index]))
            .full_width(true)
            .select_all_on_tab_focus(true)
            .validator(Arc::new(|text| parse_channel(text).is_some()))
            .on_value_changed(cx, move |page, text, cx| {
                if let Some(value) = parse_channel(text) {
                    page.set_channel(index, value, Some(index), cx);
                }
            })
            .on_focus_changed(cx, move |page, focused, cx| {
                if !focused {
                    page.sync_input(index, cx);
                }
            })
            .on_submit(cx, move |page, _, cx| page.sync_input(index, cx))
            .spawn(cx);
        let builder = ColorSliderBuilder::new(
            format!("interactive-spaces-slider-{index}"),
            INITIAL[index] as f32,
            rgb_slider_delegate(colors, INITIAL, 0, index),
        )
        .horizontal()
        .range(0. ..1.)
        .step(0.001)
        .size(gpui_luma::theme::ControlSize::Sm)
        .thumb_small();
        let track = builder.domain_renderer();
        let slider = builder.spawn(cx);
        let subscription = cx.subscribe(&slider, move |page, _, event, cx| {
            if let Some(value) = primary_slider_value(event) {
                page.set_channel(index, value as f64, None, cx);
            }
        });
        (Self { input, slider, track }, subscription)
    }

    fn refresh_track(&self, colors: Option<&NativeColors>, rgb: [f64; 3], source: usize, index: usize, cx: &mut App) {
        self.track.set_delegate(rgb_slider_delegate(colors, rgb, source, index));
        refresh_color_slider(&self.slider, cx);
    }
}

impl InteractiveSpaces {
    pub(crate) fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let field_settings = FieldSettings::new(0);
        let hsv = Hsv::from_rgba(rgba(INITIAL));
        field_settings.set_saturation(hsv.s);
        let (native_colors, mut gpu_error) = match NativeColors::new() {
            Ok(colors) => (Some(colors), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let gpu_painter = native_colors.as_ref().map(|colors| colors.field_painter(field_settings.clone()));
        let cpu_painter = native_colors.as_ref().and_then(|colors| {
            match super::native_cpu_field::painter(field_settings.clone(), colors.clone()) {
                Ok(painter) => Some(painter),
                Err(error) => {
                    gpu_error = Some(error.to_string());
                    None
                }
            }
        });
        let field = cx.new(|_| {
            let field = ColorFieldState::hue_saturation_lightness("interactive-spaces-field", hsv, 16.)
                .with_model(field_settings.model())
                .raster_image()
                .rounded(px(0.))
                .no_border()
                .inside_field();
            if let (Some(painter), Some(colors)) = (&gpu_painter, &native_colors) {
                field
                    .background_painter(painter.clone())
                    .thumb_painter(colors.thumb_painter(field_settings.clone()))
            } else {
                field
            }
        });
        let subscription = cx.subscribe(&field, |this: &mut Self, _, event, cx| {
            if let ColorFieldEvent::Change(hsv) | ColorFieldEvent::Release(hsv) = event {
                this.accept_hsv(*hsv, cx);
            }
        });
        let mut subscriptions = vec![subscription];
        let channels = std::array::from_fn(|index| {
            let (controls, subscription) = RgbChannelControls::new(index, &look, native_colors.as_ref(), cx);
            subscriptions.push(subscription);
            controls
        });
        let saturation_builder =
            ColorSliderBuilder::channel("interactive-spaces-saturation", hsv.s, hsv, Hsv::SATURATION)
                .expect("valid HSV saturation channel")
                .horizontal()
                .step(0.001)
                .size(gpui_luma::theme::ControlSize::Sm)
                .thumb_small();
        let saturation_track = saturation_builder.domain_renderer();
        if let Some(colors) = &native_colors {
            saturation_track.set_delegate(colors.saturation_track(0, hsv));
        }
        let saturation = saturation_builder.spawn(cx);
        subscriptions.push(cx.subscribe(&saturation, |this: &mut Self, _, event, cx| {
            if let Some(value) = primary_slider_value(event) {
                this.hsv.s = value;
                this.accept_hsv(this.hsv, cx);
                this.field.update(cx, |field, cx| field.set_hsv(this.hsv, cx));
            }
        }));
        let gpu_toggle = radix::Switch::new("interactive-spaces-gpu")
            .look(&look)
            .label("GPU field")
            .checked(gpu_painter.is_some())
            .enabled(gpu_painter.is_some())
            .on_value_changed(cx, |this: &mut Self, on, cx| {
                this.install_field_painter(on, cx);
                cx.notify();
            })
            .spawn(cx);
        let contour_toggle = radix::Switch::new("interactive-spaces-contour")
            .look(&look)
            .label("sRGB out of gamut")
            .checked(true)
            .on_value_changed(cx, |this: &mut Self, enabled, cx| {
                this.field_settings.set_contour(enabled);
                this.field.update(cx, |_, cx| cx.notify());
                cx.notify();
            })
            .spawn(cx);
        let source_choice = crate::lab_controls::Choice::new(
            "interactive-spaces-source",
            "Source space",
            &look,
            crate::lab_controls::labels(&NAMES),
            0,
            cx,
            |this: &mut Self, index, cx| this.set_source(index, cx),
        );
        Self {
            look,
            field_settings,
            source_choice,
            rgb: INITIAL,
            hsv,
            field,
            channels,
            saturation,
            saturation_track,
            native_colors,
            gpu_painter,
            cpu_painter,
            presentation_pending: false,
            presentation_epoch: 0,
            presentation_attempted: None,
            previous_presentation: None,
            host_window: None,
            gpu_toggle,
            contour_toggle,
            gpu_error,
            presentation_error: None,
            _subscriptions: subscriptions,
        }
    }

    fn install_field_painter(&self, gpu: bool, cx: &mut Context<Self>) {
        let painter = if gpu {
            self.gpu_painter.clone()
        } else if self.field_settings.source_index() > 0 {
            self.cpu_painter.clone().or_else(|| self.gpu_painter.clone())
        } else {
            None
        };
        self.field.update(cx, |field, cx| field.set_background_painter(painter, cx));
    }

    pub(crate) fn deactivate(&mut self, cx: &mut Context<Self>) {
        self.presentation_epoch += 1;
        self.presentation_pending = false;
        self.presentation_attempted = None;
        if let (Some(handle), Some(previous)) = (self.host_window, self.previous_presentation.take()) {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.set_color_mode(previous));
            });
        }
    }

    fn sync_presentation(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.host_window = Some(window.window_handle());
        let desired = self.field_settings.window_mode();
        if !self.presentation_pending
            && (self.previous_presentation.is_none() || window.color_status().requested() != desired)
            && self.presentation_attempted != Some(desired)
        {
            self.presentation_attempted = Some(desired);
            self.presentation_pending = true;
            let epoch = self.presentation_epoch;
            cx.defer_in(window, move |this, window, cx| {
                if epoch != this.presentation_epoch {
                    return;
                }
                this.previous_presentation.get_or_insert(window.color_status().requested());
                let desired = this.field_settings.window_mode();
                this.presentation_error = match window.set_color_mode(desired) {
                    Ok(status) if status.effective() == desired => None,
                    Ok(_) => Some("Native presentation unavailable: the backend negotiated an SDR fallback".into()),
                    Err(error) => Some(format!("Native presentation unavailable: {error}")),
                };
                this.presentation_pending = false;
                cx.notify();
            });
        }
    }

    fn render_swatch(&self, radius: f32) -> gpui::AnyElement {
        let Some(colors) = self.native_colors.clone() else {
            return div().into_any_element();
        };
        let source = self.field_settings.source_index();
        let rgb = self.rgb;
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                if let Err(error) = colors.paint(bounds, Parameters::solid(source, rgb, radius), window) {
                    tracing::error!("Native color swatch: {error:#}");
                }
            },
        )
        .size_full()
        .into_any_element()
    }

    fn set_source(&mut self, index: usize, cx: &mut Context<Self>) {
        let Ok(rgb) =
            source_field::convert_rgb(self.rgb, self.field_settings.source_space(), source_field::SPACES[index])
        else {
            return;
        };
        self.rgb = rgb;
        self.field_settings.set_source(index);
        self.update_field_selection(cx);
        self.presentation_attempted = None;
        self.presentation_error = None;
        let gpu = *self.gpu_toggle.read(cx).data();
        self.install_field_painter(gpu, cx);
        self.sync_controls(None, cx);
        self.field.update(cx, |field, cx| field.set_model(self.field_settings.model(), cx));
        cx.notify();
    }

    fn accept_hsv(&mut self, hsv: Hsv, cx: &mut Context<Self>) {
        self.hsv = hsv;
        self.field_settings.set_saturation(hsv.s);
        let color: Rgba = hsv.to_hsla_ext().into();
        self.rgb = [color.r as f64, color.g as f64, color.b as f64];
        self.sync_controls(None, cx);
        cx.notify();
    }

    fn set_channel(&mut self, index: usize, value: f64, editing: Option<usize>, cx: &mut Context<Self>) {
        self.rgb[index] = value.clamp(0., 1.);
        self.update_field_selection(cx);
        self.sync_controls(editing, cx);
        cx.notify();
    }

    fn update_field_selection(&mut self, cx: &mut Context<Self>) {
        // Retain hue at black/gray so the field remains useful at either endpoint.
        // Only the bounded picker projection is clipped; RGB coordinates retain the color.
        let mut hsv = Hsv::from_rgba(rgba(self.rgb.map(|value| value.clamp(0., 1.))));
        if hsv.s <= f32::EPSILON {
            hsv.h = self.hsv.h;
        }
        self.hsv = hsv;
        self.field_settings.set_saturation(hsv.s);
        self.field.update(cx, |field, cx| field.set_hsv(hsv, cx));
    }

    fn sync_input(&self, index: usize, cx: &mut Context<Self>) {
        self.channels[index]
            .input
            .update(cx, |input, cx| input.set_value(format!("{:.3}", self.rgb[index]), cx));
    }

    fn sync_controls(&self, editing: Option<usize>, cx: &mut Context<Self>) {
        self.saturation.update(cx, |slider, cx| slider.set_value(self.hsv.s, cx));
        let delegate = if let Some(colors) = &self.native_colors {
            colors.saturation_track(self.field_settings.source_index(), self.hsv)
        } else {
            Arc::new(ChannelDelegate::new(self.hsv, Hsv::SATURATION.into()).expect("valid HSV saturation channel"))
                as Arc<dyn gpui_luma_color::color_slider::ColorSliderDelegate>
        };
        self.saturation_track.set_delegate(delegate);
        refresh_color_slider(&self.saturation, cx);
        for index in 0..3 {
            if editing != Some(index) {
                self.sync_input(index, cx);
            }
            self.channels[index]
                .slider
                .update(cx, |slider, cx| slider.set_value(self.rgb[index].clamp(0., 1.) as f32, cx));
            self.channels[index].refresh_track(
                self.native_colors.as_ref(),
                self.rgb,
                self.field_settings.source_index(),
                index,
                cx,
            );
        }
    }

    fn panel(&self) -> Div {
        let chrome = self.look.chrome();
        vstack! {}
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
        labels: [&str; 3],
        values: [String; 3],
        notation: String,
        disabled: bool,
    ) -> Div {
        let title_style = self.look.typography_scale(TextSize::Base);
        let detail_style = gpui_luma::theme::LumaTextStyle { size: 12., ..self.look.typography_scale(TextSize::Sm) };
        self.panel()
            .p_3()
            .gap_3()
            .flex_1()
            .min_w(px(200.))
            .flex_basis(px(200.))
            .child(
                hstack! {
                    div().size(px(28.)).flex_shrink_0().rounded_sm().child(self.render_swatch(4.)),
                    vstack! {
                        hstack! {
                            div().typography_style(title_style).font_weight(FontWeight::SEMIBOLD).child(title),
                        }
                        .items_center()
                        .gap_2()
                        .when(disabled, |row| row.child(gpui_luma::infra::icon::lucide_icon(
                            lucide_svg_static::Icon::TriangleAlert,
                            self.look.chrome().body_text,
                            14.,
                        ))),
                        self.caption(notation).typography_style(detail_style),
                    },
                }
                .items_center()
                .gap_3(),
            )
            .child(hstack! {}.gap_2().children(labels.into_iter().zip(values).map(|(label, value)| {
                hstack! {
                    self.caption(label.to_owned()).typography_style(detail_style),
                    div().typography_style(detail_style).child(value),
                }
                .gap_1()
            })))
            .when(disabled, |card| card.opacity(0.45))
            .debug_selector(move || format!("color-space-{title}-{}", if disabled { "disabled" } else { "enabled" }))
    }
}

impl InteractiveSpaces {
    fn render_header(&self) -> Div {
        vstack! {
            div().text_3xl().font_weight(FontWeight::SEMIBOLD).child("Color Spaces"),
            self.caption("Explore color models by comparing one selected color in sRGB, Display P3, OKLCH, and Rec2020."),
        }
        .gap_2()
    }

    fn render_field(&self) -> Div {
        let settings = self.field_settings.clone();
        let field = self.field.clone();
        div()
            .debug_selector(|| "interactive-spaces-field-bounds".into())
            .w_full()
            .h(px(COLOR_FIELD_HEIGHT))
            .flex_shrink_0()
            .on_prepaint(move |bounds, _, cx| {
                if settings.resize(bounds.size.width.as_f32()) {
                    field.update(cx, |field, cx| field.set_model(settings.model(), cx));
                }
            })
            .child(self.field.clone())
    }

    fn render_field_controls(&self) -> Div {
        let label = radix::switch_theme(&self.look).resolve(
            false,
            gpui_luma::theme::InteractionState::default(),
            gpui_luma::theme::ControlSize::Md,
        );
        hstack! {
            self.source_choice.clone().into_element()
                .typography_style(label.label_typography)
                .font_family(label.label_font_family)
                .text_color(label.label_color),
            self.gpu_toggle.clone(),
            self.contour_toggle.clone(),
            self.caption("Saturation"),
            div().w(px(200.)).child(self.saturation.clone()),
        }
        .flex_wrap()
        .items_center()
        .gap_3()
    }

    fn render_comparisons(&self, coordinates: &Coordinates) -> Div {
        hstack! {
            self.card("sRGB", ["R", "G", "B"],
                coordinates.srgb.map(|value| format!("{:02X}", (value.clamp(0., 1.) * 255.).round() as u8)),
                "Hex sRGB".into(), coordinates.outside_srgb),
            self.card("Display P3", ["R", "G", "B"],
                coordinates.p3.map(|value| format!("{value:.3}")), "Encoded P3".into(), false),
            self.card("OKLCH", ["L", "C", "H"], [
                format!("{:.3}", coordinates.oklch[0]),
                format!("{:.3}", coordinates.oklch[1]),
                format!("{:.1}°", coordinates.oklch[2]),
            ], "D65".into(), false),
            self.card("Rec2020", ["R", "G", "B"],
                coordinates.rec2020.map(|value| format!("{value:.3}")), "Encoded Rec2020".into(), false),
        }
        .flex_wrap()
        .gap_4()
    }

    fn render_parameters(&self) -> Div {
        self.panel()
            .flex_grow(2.)
            .flex_basis(px(440.))
            .min_w(px(240.))
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Color parameters"))
            .child(self.caption(format!("Adjust encoded {} values (0–1)", NAMES[self.field_settings.source_index()])))
            .child(hstack! {}.flex_wrap().gap_4().children(self.channels.iter().enumerate().map(|(index, channel)| {
                vstack! { CHANNELS[index], channel.input.clone(), channel.slider.clone() }
                    .flex_1()
                    .min_w(px(100.))
                    .gap_2()
            })))
    }

    fn render_preview(&self, srgb_reference: Rgba) -> Div {
        self.panel()
            .flex_1()
            .flex_basis(px(265.))
            .min_w(px(225.))
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Color preview"))
            .child(self.caption("The selected color"))
            .child(div().w_full().h(px(80.)).rounded_md().child(self.render_swatch(6.)))
            .child(self.caption(format!(
                "sRGB reference {}",
                hex([srgb_reference.r as f64, srgb_reference.g as f64, srgb_reference.b as f64])
            )))
    }
}

impl Render for InteractiveSpaces {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_presentation(_window, _cx);
        let chrome = self.look.chrome();
        let coordinates = Coordinates::new(self.rgb, self.field_settings.source_space());
        let preview = coordinates.as_ref().map(|color| color.srgb_reference).unwrap_or_else(|_| rgba(self.rgb));
        let comparison = match coordinates {
            Ok(coordinates) => self.render_comparisons(&coordinates),
            Err(error) => div().child(format!("Color conversion unavailable: {error}")),
        };
        let content = vstack! {}.gap_4()
            .child(self.render_field())
            .child(self.caption(format!("Hue across · value down · saturation {:.0}%", self.hsv.s * 100.)))
            .child(self.render_field_controls())
            .when_some(self.gpu_error.as_ref().or(self.presentation_error.as_ref()), |view, error| view.child(self.caption(format!("Native color rendering unavailable: {error}"))))
            .child(comparison)
            .child(hstack! { self.render_parameters(), self.render_preview(preview) }.flex_wrap().gap_4())
            .child(self.caption("Changing source space preserves the selected color and converts its RGB coordinates. The field and color sliders explore the selected space; display capabilities determine the visible gamut."));

        div()
            .id("interactive-color-spaces")
            .size_full()
            .overflow_y_scroll()
            .bg(chrome.content_background)
            .text_color(chrome.body_text)
            .child(
                vstack! {
                    self.caption("xgpui-color  /  Color Spaces"), self.render_header(), content,
                }
                .w_full()
                .max_w(px(1240.))
                .mx_auto()
                .p_6()
                .gap_5(),
            )
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
            let coordinates = Coordinates::new(rgb, RgbSpace::Srgb).unwrap();
            let p3_roundtrip = EncodedRgb::new(RgbSpace::DisplayP3, coordinates.p3)
                .unwrap()
                .decode()
                .unwrap()
                .convert(RgbSpace::Srgb)
                .unwrap()
                .encode()
                .unwrap()
                .components();
            let rec2020_roundtrip = EncodedRgb::new(RgbSpace::Rec2020, coordinates.rec2020)
                .unwrap()
                .decode()
                .unwrap()
                .convert(RgbSpace::Srgb)
                .unwrap()
                .encode()
                .unwrap()
                .components();
            for (actual, expected) in rec2020_roundtrip.into_iter().zip(rgb) {
                assert!((actual - expected).abs() < 1e-5);
            }
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
        assert_ne!(
            Coordinates::new(INITIAL, RgbSpace::Srgb).unwrap().p3,
            INITIAL,
            "P3 must convert rather than reinterpret RGB"
        );
    }

    #[test]
    fn rgb_inputs_sliders_and_field_share_state_and_reject_invalid_edits() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| InteractiveSpaces::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        let input = cx.update(|_, cx| page.read(cx).channels[0].input.clone());
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
        let slider = cx.update(|_, cx| page.read(cx).channels[1].slider.clone());
        slider.update(cx, |slider, cx| slider.set_thumb_position(slider.primary_thumb_id(), 0.7, cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert!((page.rgb[1] - 0.7).abs() < 0.001);
            assert_eq!(page.channels[1].input.read(cx).value(), "0.700");
        });
        let bounds = cx.debug_bounds("interactive-spaces-field-bounds").unwrap();
        assert!(bounds.size.width > px(400.));
        assert_eq!(bounds.size.height, px(300.));
        cx.simulate_click(
            point(bounds.left() + bounds.size.width * 0.25, bounds.top() + bounds.size.height * 0.25),
            Default::default(),
        );
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert!((page.hsv.h - 90.).abs() < 10.);
            assert!((page.hsv.v - 0.75).abs() < 0.05);
            for (index, channel) in page.channels.iter().enumerate() {
                assert_eq!(channel.input.read(cx).value(), format!("{:.3}", page.rgb[index]));
            }
        });
    }
    #[test]
    fn switching_cpu_gpu_retains_color_and_luma_keyboard_and_drag_behavior() {
        use gpui::{KeyDownEvent, KeyUpEvent, Keystroke, MouseButton};
        use gpui_luma::controls::switch::SwitchEvent;
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|window, cx| {
            window.activate_window();
            InteractiveSpaces::new(crate::theme::default_look(), cx)
        });
        cx.run_until_parked();
        let toggle = cx.update(|_, cx| {
            let page = page.read(cx);
            assert!(page.gpu_painter.is_some());
            assert!(*page.gpu_toggle.read(cx).data());
            page.gpu_toggle.clone()
        });
        for on in [false, true] {
            let before = cx.update(|_, cx| page.read(cx).rgb);
            toggle.update(cx, |toggle, cx| {
                toggle.set_data(on, cx);
                cx.emit(SwitchEvent::Change { on });
            });
            cx.run_until_parked();
            cx.update(|_, cx| assert_eq!(page.read(cx).rgb, before));
            let bounds = cx.debug_bounds("interactive-spaces-field-bounds").unwrap();
            cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Default::default());
            let moved = point(bounds.left() + bounds.size.width * 0.35, bounds.top() + bounds.size.height * 0.4);
            cx.simulate_mouse_move(moved, MouseButton::Left, Default::default());
            cx.simulate_mouse_up(moved, MouseButton::Left, Default::default());
            cx.run_until_parked();
            let hue = cx.update(|_, cx| page.read(cx).hsv.h);
            assert!((hue - 126.).abs() < 10.);
            let keystroke = Keystroke::parse("right").unwrap();
            cx.simulate_event(KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            });
            cx.simulate_event(KeyUpEvent { keystroke });
            cx.run_until_parked();
            cx.update(|_, cx| assert!((page.read(cx).hsv.h - hue - 3.6).abs() < 0.01));
        }
    }
    #[test]
    fn saturation_slider_updates_color_and_srgb_gamut_state() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| InteractiveSpaces::new(crate::theme::default_look(), cx));
        page.update(cx, |page, cx| {
            page.set_source(1, cx);
            for (index, value) in [0., 1., 0.].into_iter().enumerate() {
                page.set_channel(index, value, None, cx);
            }
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("color-space-sRGB-disabled").is_some());
        let saturation = cx.update(|_, cx| page.read(cx).saturation.clone());
        saturation.update(cx, |slider, cx| slider.set_thumb_position(slider.primary_thumb_id(), 0.62, cx));
        cx.run_until_parked();
        let bounds = cx.debug_bounds("interactive-spaces-field-bounds").unwrap();
        cx.simulate_click(
            point(bounds.left() + bounds.size.width / 3., bounds.top() + bounds.size.height * 0.25),
            Default::default(),
        );
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert!((page.hsv.s - 0.62).abs() < 0.001);
            assert!((page.hsv.v - 0.75).abs() < 0.02);
            let field = page.field.read(cx);
            let uv = field.model.uv_from_hsv(&page.hsv);
            let painted: gpui::Rgba = field.model.color_at_uv(&page.hsv, uv).into();
            let selected = super::source_field::srgb_reference(page.rgb, page.field_settings.source_space()).unwrap();
            for (painted, selected) in
                [painted.r, painted.g, painted.b].into_iter().zip([selected.r, selected.g, selected.b])
            {
                assert!((painted - selected).abs() < 1e-5);
            }
        });
        saturation.update(cx, |slider, cx| slider.set_thumb_position(slider.primary_thumb_id(), 0., cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert_eq!(page.hsv.s, 0.);
            assert!((page.rgb[0] - page.rgb[1]).abs() < 1e-6);
            assert!((page.rgb[1] - page.rgb[2]).abs() < 1e-6);
            for (index, channel) in page.channels.iter().enumerate() {
                assert_eq!(channel.input.read(cx).value(), format!("{:.3}", page.rgb[index]));
            }
        });
        assert!(cx.debug_bounds("color-space-sRGB-enabled").is_some());
    }

    #[test]
    fn source_selector_requests_native_presentation_and_preserves_color() {
        use crate::color_demos::source_field::{self, SPACES};
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| InteractiveSpaces::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        let mode = cx.update(|window, _| window.color_status().effective());
        let selector = cx.update(|_, cx| page.read(cx).source_choice.clone());
        let mut colors = Vec::new();
        for (index, space) in SPACES.into_iter().enumerate().chain([(0, SPACES[0])]) {
            cx.update(|_, cx| selector.select(index, cx));
            cx.run_until_parked();
            cx.update(|window, cx| {
                // The headless UI platform cannot enable native wide output. It must
                // report the failed request while retaining its previous configuration.
                assert_eq!(window.color_status().effective(), mode);
                let page = page.read(cx);
                if index > 0 {
                    assert_eq!(page.presentation_attempted, Some(gpui::WindowColorMode::WideGamutSdr));
                    assert!(page
                        .presentation_error
                        .as_ref()
                        .is_some_and(|error| error.contains("Native presentation unavailable")));
                }
                assert_eq!(
                    page.field_settings.window_mode(),
                    if index == 0 {
                        gpui::WindowColorMode::Default
                    } else {
                        gpui::WindowColorMode::WideGamutSdr
                    }
                );
                if index == 0 {
                    assert!(page.presentation_error.is_none());
                }
                let expected_rgb = source_field::convert_rgb(INITIAL, RgbSpace::Srgb, space).unwrap();
                for (actual, expected) in page.rgb.into_iter().zip(expected_rgb) {
                    assert!((actual - expected).abs() < 1e-6);
                }
                assert_eq!(page.channels[0].input.read(cx).value(), format!("{:.3}", expected_rgb[0]));
                let coordinates = Coordinates::new(page.rgb, space).unwrap();
                colors.push(coordinates.srgb_reference);
                let field = page.field.read(cx);
                let thumb: gpui::Rgba = field.model.thumb_color(&page.hsv).into();
                let expected = source_field::srgb_reference(page.rgb, space).unwrap();
                for channel in 0..3 {
                    for endpoint in [0., 1.] {
                        let mut sample = page.rgb;
                        sample[channel] = endpoint;
                        let expected = source_field::srgb_reference(sample, space).unwrap();
                        let actual: gpui::Rgba =
                            gpui_luma::controls::slider::DomainTrackRenderer::get_color_at_position(
                                page.channels[channel].track.as_ref(),
                                endpoint as f32,
                            )
                            .unwrap()
                            .into();
                        for (actual, expected) in
                            [actual.r, actual.g, actual.b].into_iter().zip([expected.r, expected.g, expected.b])
                        {
                            assert!((actual - expected).abs() < 1e-5, "source-aware RGB track endpoint");
                        }
                    }
                }

                for (actual, expected) in
                    [thumb.r, thumb.g, thumb.b].into_iter().zip([expected.r, expected.g, expected.b])
                {
                    assert!((actual - expected).abs() < 1e-5);
                }
                // Extended sRGB coordinates must retain the source color through conversion.
                let roundtrip = EncodedRgb::new(RgbSpace::Srgb, coordinates.srgb)
                    .unwrap()
                    .decode()
                    .unwrap()
                    .convert(space)
                    .unwrap()
                    .encode()
                    .unwrap()
                    .components();
                for (actual, expected) in roundtrip.into_iter().zip(page.rgb) {
                    assert!((actual - expected).abs() < 1e-5);
                }
            });
        }
        for color in colors {
            for (actual, expected) in [color.r, color.g, color.b].into_iter().zip(INITIAL) {
                assert!((actual as f64 - expected).abs() < 1e-6);
            }
        }
    }
    #[test]
    fn srgb_card_is_disabled_only_for_colors_outside_srgb() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| InteractiveSpaces::new(crate::theme::default_look(), cx));
        page.update(cx, |page, cx| {
            for (index, value) in [0., 1., 0.].into_iter().enumerate() {
                page.set_channel(index, value, None, cx);
            }
        });
        let selector = cx.update(|_, cx| page.read(cx).source_choice.clone());
        for (source, disabled) in [(1, false), (2, false), (0, false)] {
            cx.update(|_, cx| selector.select(source, cx));
            cx.run_until_parked();
            assert_eq!(cx.debug_bounds("color-space-sRGB-disabled").is_some(), disabled);
            assert_eq!(cx.debug_bounds("color-space-sRGB-enabled").is_some(), !disabled);
            for name in ["color-space-Display P3-enabled", "color-space-OKLCH-enabled", "color-space-Rec2020-enabled"] {
                assert!(cx.debug_bounds(name).is_some());
            }
        }
        cx.update(|_, cx| selector.select(1, cx));
        page.update(cx, |page, cx| {
            for (index, value) in [0., 1., 0.].into_iter().enumerate() {
                page.set_channel(index, value, None, cx);
            }
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("color-space-sRGB-disabled").is_some());
        for source in [0, 2, 1] {
            cx.update(|_, cx| selector.select(source, cx));
            cx.run_until_parked();
            assert!(cx.debug_bounds("color-space-sRGB-disabled").is_some());
            cx.update(|_, cx| {
                let page = page.read(cx);
                let p3 = Coordinates::new(page.rgb, page.field_settings.source_space()).unwrap().p3;
                for (actual, expected) in p3.into_iter().zip([0., 1., 0.]) {
                    assert!((actual - expected).abs() < 1e-6, "wide color survives a narrower source");
                }
            });
        }
        page.update(cx, |page, cx| {
            for (index, value) in [0.6, 0.4, 0.5].into_iter().enumerate() {
                page.set_channel(index, value, None, cx);
            }
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("color-space-sRGB-enabled").is_some());
    }
}
