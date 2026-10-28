pub(crate) mod reference;
mod labs;
pub(crate) mod benchmarks;

use std::sync::Arc;

use gpui::{Context, CustomGeometry, Render, Rgba, Window, WindowColorMode, canvas, div, prelude::*, px};
use gpui_luma_look_radix::{self as radix, Look};
use gpui_luma::controls::button::{Button, ButtonEvent};
use gpui_luma::prelude::HasPresenter;
use gpui::{Entity, Subscription, SharedString};
use xgpui_gamut::color::GamutMapping;
use xgpui_gamut::instanced::WidePainter;
use xgpui_gamut::{
    WideColor,
    color::{LinearRgb, RgbSpace},
};

use crate::theme::ColorVizLookExt;

struct Sample {
    label: &'static str,
    wide: Arc<WideColor>,
    srgb: Rgba,
}

fn samples() -> anyhow::Result<Vec<Sample>> {
    [
        ("P3 Red", [1.0, 0.0, 0.0]),
        ("P3 Green", [0.0, 1.0, 0.0]),
        ("P3 Orange", [1.0, 0.25, 0.0]),
        ("P3 Magenta", [1.0, 0.0, 1.0]),
    ]
    .into_iter()
    .map(|(label, components)| {
        let linear = LinearRgb::new(RgbSpace::DisplayP3, components)?;
        let clipped = linear.convert(RgbSpace::Srgb)?.components().map(|v| v.clamp(0.0, 1.0));
        let [r, g, b] = LinearRgb::new(RgbSpace::Srgb, clipped)?.encode()?.components();
        Ok(Sample {
            label,
            wide: Arc::new(WideColor::linear_p3([
                components[0] as f32,
                components[1] as f32,
                components[2] as f32,
                1.0,
            ])?),
            srgb: Rgba { r: r as f32, g: g as f32, b: b as f32, a: 1.0 },
        })
    })
    .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Preset,
    Mapping,
    Alpha,
    Presentation,
    Overlay,
    Opacity,
    Order,
    Background,
    Route,
}

pub struct GamutPage {
    look: Arc<Look>,
    p3_error: Option<String>,
    samples: anyhow::Result<Vec<Sample>>,
    pub active_tab: SharedString,
    preset: usize,
    policy: GamutMapping,
    translucent: bool,
    overlay: bool,
    half_opacity: bool,
    reverse: bool,
    dark: bool,
    painter: anyhow::Result<WidePainter>,
    window_handle: Option<gpui::AnyWindowHandle>,
    requested_wide: bool,
    route: usize,
    controls: Vec<(Action, Entity<Button>)>,
    choices: Vec<(Action, crate::lab_controls::Choice)>,
    switches: Vec<(Action, gpui_luma::controls::switch::Switch)>,
    benchmarks: Entity<benchmarks::Benchmarks>,
    last_lab_tab: SharedString,
    paint_errors: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    _subscriptions: Vec<Subscription>,
}

impl GamutPage {
    fn init_choices(&mut self, cx: &mut Context<Self>) {
        self.choices = [Action::Preset, Action::Route]
            .into_iter()
            .map(|action| {
                let (label, options, selected) = self.choice_options(action);
                let choice = crate::lab_controls::Choice::new(
                    format!("gamut-{action:?}"),
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
            Action::Preset => ("Palette", reference::PRESETS.iter().map(|s| s.to_string()).collect(), self.preset),
            Action::Route => {
                ("Presentation path", crate::lab_controls::labels(&["Texture", "Instanced", "Gradient"]), self.route)
            }
            _ => unreachable!("action has no selector"),
        }
    }
    fn select_choice(&mut self, action: Action, index: usize, cx: &mut Context<Self>) {
        match action {
            Action::Preset => {
                self.preset = index;
            }
            Action::Route => {
                self.route = index;
            }
            _ => unreachable!("action has no selector"),
        }

        cx.notify();
    }

    pub fn activate(&mut self, tab: SharedString, cx: &mut Context<Self>) {
        if tab == "Benchmarks" {
            let settings = benchmarks::Settings::from_page(self);
            self.benchmarks.update(cx, |benchmarks, _| benchmarks.settings = settings);
        } else {
            self.last_lab_tab = tab.clone();
        }
        self.active_tab = tab;
        cx.notify();
    }

    pub fn new(look: Arc<Look>, p3_error: Option<String>, cx: &mut Context<Self>) -> Self {
        let controls = [
            (Action::Mapping, "Toggle clipping / chroma"),
            (Action::Presentation, "Toggle P3 / sRGB"),
            (Action::Opacity, "Toggle opacity"),
            (Action::Order, "Reverse surfaces"),
            (Action::Background, "Toggle background"),
        ]
        .map(|(action, label)| {
            (
                action,
                radix::Button::new(format!("gamut-{action:?}")).look(look.as_ref()).outline().label(label).spawn(cx),
            )
        })
        .to_vec();
        let switches = [(Action::Alpha, "Transparency", false), (Action::Overlay, "Overlays", true)]
            .map(|(action, label, checked)| {
                (
                    action,
                    radix::Switch::new(format!("gamut-{action:?}"))
                        .look(look.as_ref())
                        .label(label)
                        .checked(checked)
                        .on_value_changed(cx, move |this: &mut Self, on, cx| {
                            match action {
                                Action::Alpha => this.translucent = on,
                                Action::Overlay => this.overlay = on,
                                _ => unreachable!(),
                            }
                            cx.notify();
                        })
                        .spawn(cx),
                )
            })
            .to_vec();
        let subscriptions = controls
            .iter()
            .map(|(action, button)| {
                let action = *action;
                cx.subscribe(button, move |this, _, event: &ButtonEvent, cx| {
                    if event.is_click() {
                        match action {
                            Action::Route | Action::Preset => unreachable!("selector setting"),
                            Action::Mapping => {
                                this.policy = if this.policy == GamutMapping::Clip {
                                    GamutMapping::ReduceChroma
                                } else {
                                    GamutMapping::Clip
                                }
                            }
                            Action::Alpha => this.translucent = !this.translucent,
                            Action::Overlay => this.overlay = !this.overlay,
                            Action::Opacity => this.half_opacity = !this.half_opacity,
                            Action::Order => this.reverse = !this.reverse,
                            Action::Background => this.dark = !this.dark,
                            Action::Presentation => {
                                if let Some(handle) = this.window_handle {
                                    let next_wide = !this.requested_wide;
                                    let result = handle
                                        .update(cx, |_, window, _| {
                                            window.set_color_mode(if next_wide {
                                                WindowColorMode::WideGamutSdr
                                            } else {
                                                WindowColorMode::Default
                                            })
                                        })
                                        .and_then(|result| result);
                                    match result {
                                        Ok(_) => {
                                            this.requested_wide = next_wide;
                                            this.p3_error = None;
                                        }
                                        Err(error) => this.p3_error = Some(error.to_string()),
                                    }
                                }
                            }
                        }
                        cx.notify();
                    }
                })
            })
            .collect();
        let benchmarks = cx.new(|cx| benchmarks::Benchmarks::new(look.clone(), cx));
        let mut page = Self {
            look,
            p3_error,
            samples: samples(),
            active_tab: "Wide Color Fills".into(),
            preset: 1,
            policy: GamutMapping::Clip,
            translucent: false,
            overlay: true,
            half_opacity: false,
            reverse: false,
            dark: true,
            painter: WidePainter::new(),
            window_handle: None,
            requested_wide: true,
            route: 0,
            controls,
            choices: Vec::new(),
            switches,
            benchmarks,
            last_lab_tab: "Wide Color Fills".into(),
            paint_errors: Default::default(),
            _subscriptions: subscriptions,
        };
        page.init_choices(cx);
        page
    }
}

impl Render for GamutPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.active_tab == "Benchmarks" {
            return div().size_full().child(self.benchmarks.clone()).into_any_element();
        }
        self.paint_errors.borrow_mut().clear();
        self.window_handle = Some(window.window_handle());
        let status = window.color_status();
        let wide = status.effective() == WindowColorMode::WideGamutSdr;
        let display = match status.display_wide_gamut() {
            Some(true) => "Wide gamut",
            Some(false) => "sRGB / SDR",
            None => "Unknown",
        };
        let fallback = self.p3_error.clone().unwrap_or_else(|| {
            status.fallback().map(|reason| format!("{reason:?}")).unwrap_or_else(|| {
                if !self.requested_wide {
                    "sRGB baseline selected".into()
                } else if wide {
                    "None".into()
                } else {
                    "Wide output unavailable; using sRGB".into()
                }
            })
        });
        div()
            .id("xgpui-gamut-status")
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .text_color(self.look.chrome().body_text)
            .child("Window presentation")
            .child(format!(
                "Intent: {} · Accepted: {:?} · Effective: {:?}",
                if self.requested_wide {
                    "P3 (WideGamutSdr)"
                } else {
                    "sRGB (Default)"
                },
                status.requested(),
                status.effective()
            ))
            .child(format!("Backend support: {} · Display: {display}", status.supports_wide_gamut_sdr()))
            .child(format!("Fallback: {fallback}"))
            .child("Status reports negotiated presentation, not measured display fidelity.")
            .when(self.active_tab == "Wide Color Fills", |page| match &self.samples {
                Err(error) => page.child(format!("Samples unavailable: {error}")),
                Ok(samples) => page
                    .child(if wide {
                        "Linear Display P3 compared with clipped sRGB"
                    } else {
                        "sRGB fallback previews — native P3 output unavailable"
                    })
                    .children(samples.iter().map(|sample| {
                        let color = sample.wide.clone();
                        let p3 = div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(if wide { "Display P3" } else { "P3 → sRGB fallback" })
                            .child(div().h(px(64.0)).w_full().when(!wide, |swatch| swatch.bg(sample.srgb)).when(
                                wide,
                                |swatch| {
                                    swatch.child(
                                        canvas(
                                            |_, _, _| (),
                                            move |bounds, _, window, _| {
                                                window.paint_custom(
                                                    CustomGeometry::RoundedRect {
                                                        bounds,
                                                        corner_radii: Default::default(),
                                                    },
                                                    color.clone(),
                                                );
                                            },
                                        )
                                        .size_full(),
                                    )
                                },
                            ));
                        div().flex().flex_col().gap_2().child(sample.label).child(
                            div().flex().gap_4().child(p3).child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child("Clipped sRGB")
                                    .child(div().h(px(64.0)).w_full().bg(sample.srgb)),
                            ),
                        )
                    })),
            })
            .when(self.active_tab != "Wide Color Fills", |page| page.child(self.lab_content(wide, window, cx)))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_samples_convert_and_clip_to_valid_srgb() {
        let samples = samples().expect("valid P3 references");
        assert_eq!(samples.len(), 4);
        for sample in &samples {
            for channel in [sample.srgb.r, sample.srgb.g, sample.srgb.b, sample.srgb.a] {
                assert!(channel.is_finite() && (0.0..=1.0).contains(&channel));
            }
        }
        assert!(samples[2].srgb.g > 0.0 && samples[2].srgb.g < 1.0);
        assert_eq!(samples[2].srgb.b, 0.0);
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn gamut_tabs_render_with_srgb_fallback_and_retained_controls() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (page, cx) = app
            .add_window_view(|_, cx| GamutPage::new(crate::theme::default_look(), Some("headless backend".into()), cx));
        for tab in ["Wide Color Fills", "Wide Gradients", "Gamut Mapping", "Presentation"] {
            page.update(cx, |page, cx| {
                page.active_tab = tab.into();
                cx.notify();
            });
            cx.run_until_parked();
        }
        let buttons = cx.update(|_, cx| page.read(cx).controls.clone());
        for (action, button) in buttons {
            if action != Action::Presentation {
                button.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
                cx.run_until_parked();
            }
        }
        let choices = cx.update(|_, cx| page.read(cx).choices.clone());
        for (action, choice) in choices {
            cx.update(|_, cx| choice.select(if action == Action::Preset { 2 } else { 1 }, cx));
        }
        let switches = cx.update(|_, cx| page.read(cx).switches.clone());
        for (_, switch) in switches {
            switch.update(cx, |switch, cx| {
                let on = !*switch.data();
                switch.set_data(on, cx);
                cx.emit(gpui_luma::controls::switch::SwitchEvent::Change { on });
            });
            cx.run_until_parked();
        }
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert!(page.painter.is_ok(), "wide-gradient program must validate");
            assert_eq!(page.preset, 2);
            assert_eq!(page.policy, GamutMapping::ReduceChroma);
            assert!(page.translucent && page.half_opacity && page.reverse);
            assert!(!page.overlay && !page.dark);
        });
    }
}
