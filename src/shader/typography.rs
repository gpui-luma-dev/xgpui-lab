//! Shared native/browser acceptance demo for issue #28's static and motion milestones.
use gpui::{prelude::*, *};
use std::{sync::Arc, time::Instant};
use gpui_luma::{prelude::HasPresenter, controls::button::Button};
use gpui_luma_look_radix::{self as radix, Look};
use xgpui_shader::typography::{KineticEffect, SelectableText, TextShadow};
use xgpui_shader::{Brush, BrushStop};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MotionMode {
    Static,
    Typewriter,
    Scramble,
    Wave,
}

const SAMPLES: [(&str, &str, &str); 4] = [
    (
        "Editorial",
        "Good typography keeps its meaning.",
        "A shaped line, an authentic source, and a selection you can trust.",
    ),
    (
        "Ligatures",
        "office · affine · efficient · café",
        "Select inside fi / ffi: caret positions interpolate within the shaped ligature.",
    ),
    (
        "Unicode",
        "Cafe\u{301} · naïve · 👩🏽‍💻 · 🇺🇸",
        "Combining accents, skin tones, joined emoji and flags stay together when selecting.",
    ),
    (
        "Overflow",
        "The whole sentence remains selectable, even when the viewport is deliberately narrow.",
        "Drag beyond the edge, or use Shift + End. The caret scrolls into view.",
    ),
];
pub struct TypographyDemo {
    buttons: Vec<(Action, String, Entity<Button>)>,
    choices: Vec<(Action, crate::lab_controls::Choice)>,
    focus_pending: bool,
    samples: Vec<Entity<SelectableText>>,
    active: usize,
    narrow: bool,
    large: bool,
    light: bool,
    status: SharedString,
    _subscriptions: Vec<Subscription>,
    motion: MotionMode,
    playing: bool,
    progress: f32,
    phase: f32,
    reduced: bool,
    last_tick: Instant,
    seed: u64,
    fill: usize,
    shadow: usize,
    softness: usize,
}
impl TypographyDemo {
    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let samples: Vec<_> = SAMPLES
            .iter()
            .map(|(_, text, _)| cx.new(|cx| SelectableText::new(*text, cx).expect("LTR demo sample")))
            .collect();
        let subscriptions = samples.iter().map(|sample| cx.observe(sample, |_, _, cx| cx.notify())).collect();
        let mut demo = Self {
            buttons: Self::make_buttons(look.clone(), cx),
            choices: Self::make_choices(&look, cx),
            focus_pending: false,
            samples,
            active: 0,
            narrow: false,
            large: false,
            light: false,
            status: "Select a phrase to inspect its authentic source.".into(),
            _subscriptions: subscriptions,
            motion: MotionMode::Static,
            playing: false,
            progress: 0.45,
            phase: 0.,
            reduced: false,
            last_tick: Instant::now(),
            seed: 42,
            fill: 1,
            shadow: 1,
            softness: 1,
        };
        demo.apply_style(cx);
        demo
    }
    fn apply_style(&mut self, cx: &mut Context<Self>) {
        let fill = match self.fill {
            1 => Some(
                Brush::linear(
                    0.,
                    vec![
                        BrushStop::new(
                            0.,
                            if self.light {
                                [0.02, 0.15, 0.6, 1.]
                            } else {
                                [0.25, 0.65, 1., 1.]
                            },
                        ),
                        BrushStop::new(
                            1.,
                            if self.light {
                                [0.0, 0.45, 0.25, 1.]
                            } else {
                                [0.5, 1., 0.8, 1.]
                            },
                        ),
                    ],
                )
                .unwrap(),
            ),
            2 => Some(
                Brush::linear(
                    0.,
                    vec![
                        BrushStop::new(
                            0.,
                            if self.light {
                                [0.65, 0.02, 0.25, 1.]
                            } else {
                                [1., 0.35, 0.6, 1.]
                            },
                        ),
                        BrushStop::new(
                            1.,
                            if self.light {
                                [0.5, 0.2, 0., 1.]
                            } else {
                                [1., 0.85, 0.35, 1.]
                            },
                        ),
                    ],
                )
                .unwrap(),
            ),
            _ => None,
        };
        let radius = px([4., 8., 12.][self.softness]);
        let shadow = match self.shadow {
            1 => Some(TextShadow::glow(rgba(if self.light { 0x286de580 } else { 0x56dac8e0 }).into(), radius).unwrap()),
            2 => Some(TextShadow::new(rgba(0x000000b0).into(), point(px(3.), px(5.)), radius).unwrap()),
            _ => None,
        };
        for sample in &self.samples {
            sample.update(cx, |text, cx| {
                text.set_fill(fill.clone(), cx).unwrap();
                text.set_shadow(shadow.clone(), cx);
            });
        }
        cx.notify();
    }
    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        self.active = index;
        self.status = "Sample changed. Selection is retained independently for each sample.".into();
        self.focus_pending = true;
        cx.notify();
    }
    fn select_all(&mut self, cx: &mut Context<Self>) {
        self.samples[self.active].update(cx, |text, cx| text.select_all(cx));
        self.focus_pending = true;
    }
    fn copy(&mut self, cx: &mut Context<Self>) {
        let copied = self.samples[self.active].update(cx, |text, cx| text.copy_selection(cx));
        self.status = if copied {
            "Copy requested for the selected source. Paste into another app to verify."
        } else {
            "Select some text first; the clipboard was left unchanged."
        }
        .into();
        cx.notify();
    }
}
impl Render for TypographyDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_pending {
            self.focus_pending = false;
            window.focus(&self.samples[self.active].focus_handle(cx), cx);
        }
        let reduced = self.reduced || cx.reduce_motion();
        let now = Instant::now();
        if self.playing && !reduced && self.motion != MotionMode::Static {
            let dt = now.duration_since(self.last_tick).as_secs_f32().min(0.1);
            if matches!(self.motion, MotionMode::Typewriter | MotionMode::Scramble) {
                if self.motion == MotionMode::Scramble {
                    self.phase += dt * 12.;
                }
                self.progress = (self.progress + dt / 4.).min(1.);
                if self.progress >= 1. {
                    self.playing = false;
                }
            } else {
                self.phase = (self.phase + dt * 2.2).rem_euclid(std::f32::consts::TAU);
            }
            if self.playing {
                window.request_animation_frame();
            }
        }
        self.last_tick = now;
        self.sync_buttons(reduced, cx);
        let effect = if reduced {
            KineticEffect::Static
        } else {
            match self.motion {
                MotionMode::Static => KineticEffect::Static,
                MotionMode::Typewriter => KineticEffect::Typewriter { progress: self.progress },
                MotionMode::Scramble => KineticEffect::Scramble {
                    progress: self.progress,
                    seed: self.seed,
                    tick: self.phase.floor() as u64,
                },
                MotionMode::Wave => KineticEffect::Wave { amplitude: px(8.), phase: self.phase, cluster_phase: 0.65 },
            }
        };
        self.samples[self.active].update(cx, |text, cx| text.set_effect(effect, cx).unwrap());
        let text = self.samples[self.active].read(cx);
        let range = text.selection();
        let selected: SharedString = if range.is_empty() {
            "No selection yet".into()
        } else {
            text.selected_text().to_owned().into()
        };
        let count = text.selected_text().chars().count();
        let background = if self.light { 0xf1f3ed } else { 0x0c1422 };
        let surface = if self.light { 0xffffff } else { 0x131f30 };
        let foreground = if self.light { 0x142238 } else { 0xeaf1f6 };
        let muted = if self.light { 0x50617a } else { 0x9fb0c5 };
        div().id("typography-demo").size_full().overflow_y_scroll()
            .bg(rgb(background)).text_color(rgb(foreground)).font_family("IBM Plex Sans")
            .child(div().max_w(px(1040.)).mx_auto().p_8().flex().flex_col().gap_6()
                .child(div().flex().justify_between().items_center()
                    .child(div().text_size(px(12.)).text_color(rgb(0x36b8a7)).child("TYPE / 04     •     LIGHT, COLOUR & MEANING"))
                    .child(div().text_size(px(12.)).text_color(rgb(muted)).child("xgpui-shader / issue #28")))
                .child(div().text_size(px(12.)).text_color(rgb(muted)).child("SDR preview for glyph glow and shadow · previous color presentation restored on exit"))
                .child(div().flex().flex_col().gap_2()
                    .child(div().text_size(px(42.)).line_height(px(50.)).child("Bright ideas. Real words."))
                    .child(div().text_size(px(16.)).line_height(px(25.)).text_color(rgb(muted))
                        .child("Only the painted glyphs move or disappear. Select across the original line, then copy: the source stays complete.")))
                .child(div().flex().flex_wrap().gap_2().children(SAMPLES.iter().enumerate().map(|(index, (_label, _, _))| {
                    self.button(Action::Sample(index))

                })))
                .child(div().flex().flex_col().gap_3()
                    .child(div().flex().flex_wrap().gap_2().children([
                        (MotionMode::Static, "Static"), (MotionMode::Typewriter, "Typewriter"), (MotionMode::Scramble, "Scramble"), (MotionMode::Wave, "Wave")
                    ].into_iter().map(|(mode, _label)| self.button(Action::Motion(mode)))))
                    .child(div().flex().flex_wrap().gap_2()
                        .child(self.button(Action::Play))
                        .child(self.button(Action::Restart))
                        .child(self.button(Action::Back))
                        .child(self.button(Action::Next))
                        .when(self.motion == MotionMode::Scramble, |el| el
                            .child(self.button(Action::Seed))
                            .child(self.button(Action::Frame)))
                        .child(self.button(Action::Reduced)))
                    .child(div().text_size(px(12.)).text_color(rgb(muted)).child(if reduced {
                        "Reduced motion shows the complete static source and stops animation requests.".to_owned()
                    } else if self.motion == MotionMode::Scramble {
                        format!("Decoded: {:.0}% · seed {} / frame {} · spaces and emoji stay original. Pause or step to inspect.", self.progress * 100., self.seed, self.phase.floor() as u64)
                    } else if self.motion == MotionMode::Typewriter {
                        format!("Reveal: {:.0}% · complete shaped clusters appear together. Selection can include unrevealed text.", self.progress * 100.)
                    } else {
                        if self.motion == MotionMode::Static { "Static reference · select the unchanged source.".to_owned() } else { "Wave: 8 px amplitude · selection follows the original baseline. Pause to inspect a frame.".to_owned() }
                    })))
                .child(div().flex().flex_col().gap_2()
                    .child(div().flex().flex_wrap().gap_2()
                        .child(self.button(Action::Fill))
                        .child(self.button(Action::Shadow))
                        .child(self.button(Action::Softness)))
                    .child(div().text_size(px(12.)).text_color(rgb(muted)).child("Gradient fill and a glyph-only halo. Selection stays sharp; emoji keep their original colors.")))
                .child(div().p_6().rounded_xl().bg(rgb(surface)).border_1().border_color(rgb(0x42546b)).flex().flex_col().gap_4()
                    .child(div().flex().justify_between().flex_wrap().gap_2().text_size(px(12.)).text_color(rgb(muted))
                        .child("VISUAL LAYER  /  CANONICAL SELECTION")
                        .child(if self.narrow { "320 px viewport • horizontal overflow" } else { "Fluid viewport • resize the window" }))
                    .child(div().id("typography-surface").debug_selector(|| "typography-surface".into()).w(if self.narrow { Length::from(px(320.)) } else { Length::from(relative(1.)) }).max_w_full()
                        .text_size(px(if self.large { 36. } else { 27. })).line_height(px(if self.large { 52. } else { 42. }))
                        .child(self.samples[self.active].clone()))
                    .child(div().text_size(px(13.)).line_height(px(21.)).text_color(rgb(muted)).child(SAMPLES[self.active].2))
                    .child(div().flex().flex_wrap().gap_2()
                        .child(self.button(Action::SelectAll)
                            )
                        .child(self.button(Action::Copy)
                            )
                        .child(self.button(Action::Width)
                            )
                        .child(self.button(Action::Size)
                            )
                        .child(self.button(Action::Theme)
                            ))
                    .child(div().id("copy-status").role(Role::Status).text_size(px(12.)).text_color(rgb(muted)).child(self.status.clone())))
                .child(div().flex().flex_col().gap_3().p_5().rounded_lg().bg(rgb(surface))
                    .child(div().flex().justify_between().text_size(px(12.)).text_color(rgb(muted))
                        .child("LOGICAL LAYER  /  ORIGINAL SOURCE")
                        .child(format!("UTF-8 {}..{}  /  {} Unicode scalars", range.start, range.end, count)))
                    .child(div().text_size(px(15.)).line_height(px(24.)).text_color(rgb(muted)).child(SAMPLES[self.active].1))
                    .child(div().text_size(px(20.)).line_height(px(30.)).child(selected))
                    .child(div().text_size(px(12.)).text_color(rgb(muted)).child("The inspector and clipboard read the original string, even where glyphs are hidden or displaced.")))
                .child(div().flex().flex_col().gap_2().text_size(px(13.)).line_height(px(22.)).text_color(rgb(muted))
                    .child("TRY IT  /  Tab to focus · Shift + arrows to select · Home / End · Cmd or Ctrl + A / C · double-click a word · triple-click the line")
                    .child(if cfg!(target_family = "wasm") { "ACCESSIBILITY  /  The web platform has no canvas accessibility bridge yet. Use the native demo to inspect read-only text and selection semantics." } else { "ACCESSIBILITY  /  Read-only text field with the full source, a text run and selection endpoints. Motion does not change the accessible source or selection." })
                    .child("SCOPE  /  Single-line LTR text. Wrapping, bidirectional selection and rich text editing remain outside this milestone.")))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Sample(usize),
    Motion(MotionMode),
    Play,
    Restart,
    Back,
    Next,
    Seed,
    Frame,
    Reduced,
    Fill,
    Shadow,
    Softness,
    SelectAll,
    Copy,
    Width,
    Size,
    Theme,
}
impl TypographyDemo {
    fn make_buttons(look: Arc<Look>, cx: &mut Context<Self>) -> Vec<(Action, String, Entity<Button>)> {
        use self::Action::*;
        let actions = [
            Sample(0),
            Sample(1),
            Sample(2),
            Sample(3),
            Motion(MotionMode::Static),
            Motion(MotionMode::Typewriter),
            Motion(MotionMode::Scramble),
            Motion(MotionMode::Wave),
            Play,
            Restart,
            Back,
            Next,
            Seed,
            Frame,
            Reduced,
            SelectAll,
            Copy,
            Width,
            Size,
            Theme,
        ];
        actions
            .into_iter()
            .map(|action| {
                let button = radix::Button::new(format!("typography-{action:?}"))
                    .look(look.as_ref())
                    .outline()
                    .label("…")
                    .on_click(cx, move |this: &mut Self, cx| this.action(action, cx))
                    .spawn(cx);
                (action, String::new(), button)
            })
            .collect()
    }
    fn make_choices(look: &Look, cx: &mut Context<Self>) -> Vec<(Action, crate::lab_controls::Choice)> {
        [
            (Action::Fill, "Fill", crate::lab_controls::labels(&["Native", "Cool gradient", "Warm gradient"])),
            (Action::Shadow, "Shadow", crate::lab_controls::labels(&["Off", "Glow", "Drop"])),
            (Action::Softness, "Softness", crate::lab_controls::labels(&["4 px", "8 px", "12 px"])),
        ]
        .into_iter()
        .map(|(action, label, options)| {
            (
                action,
                crate::lab_controls::Choice::new(
                    format!("typography-{action:?}"),
                    label,
                    look,
                    options,
                    0,
                    cx,
                    move |this: &mut Self, index, cx| {
                        match action {
                            Action::Fill => this.fill = index,
                            Action::Shadow => this.shadow = index,
                            Action::Softness => this.softness = index,
                            _ => unreachable!(),
                        }
                        this.apply_style(cx);
                        cx.notify();
                    },
                ),
            )
        })
        .collect()
    }
    fn button(&self, action: Action) -> AnyElement {
        if let Some((_, choice)) = self.choices.iter().find(|(key, _)| *key == action) {
            return choice.clone().into_any_element();
        }
        self.buttons
            .iter()
            .find(|(key, _, _)| *key == action)
            .expect("registered typography control")
            .2
            .clone()
            .into_any_element()
    }
    fn sync_buttons(&mut self, reduced: bool, cx: &mut Context<Self>) {
        for (action, choice) in &self.choices {
            let (options, index) = match action {
                Action::Fill => (crate::lab_controls::labels(&["Native", "Cool gradient", "Warm gradient"]), self.fill),
                Action::Shadow => (crate::lab_controls::labels(&["Off", "Glow", "Drop"]), self.shadow),
                Action::Softness => (crate::lab_controls::labels(&["4 px", "8 px", "12 px"]), self.softness),
                _ => unreachable!(),
            };
            choice.sync(options, index, cx);
        }
        for (action, previous, button) in &mut self.buttons {
            let label = match *action {
                Action::Sample(index) => {
                    format!("{}{}", if index == self.active { "✓ " } else { "" }, SAMPLES[index].0)
                }
                Action::Motion(mode) => format!("{}{:?}", if mode == self.motion { "✓ " } else { "" }, mode),
                Action::Play => if self.playing { "Pause" } else { "Play" }.into(),
                Action::Restart => "Restart".into(),
                Action::Back => "−10%".into(),
                Action::Next => "+10%".into(),
                Action::Seed => "New seed".into(),
                Action::Frame => "Next frame".into(),
                Action::Reduced => format!("Reduced motion: {}", if reduced { "on" } else { "off" }),
                Action::Fill => ["Fill: native", "Fill: cool gradient", "Fill: warm gradient"][self.fill].into(),
                Action::Shadow => ["Shadow: off", "Shadow: glow", "Shadow: drop"][self.shadow].into(),
                Action::Softness => format!("Softness: {} px", [4, 8, 12][self.softness]),
                Action::SelectAll => "Select all".into(),
                Action::Copy => "Copy selection ↗".into(),
                Action::Width => format!("Width: {}", if self.narrow { "narrow" } else { "fluid" }),
                Action::Size => format!("Type: {} px", if self.large { 36 } else { 27 }),
                Action::Theme => format!("Theme: {}", if self.light { "light" } else { "ink" }),
            };
            if *previous != label {
                *previous = label.clone();
                button.update(cx, |button, cx| button.set_label(label, cx));
            }
        }
    }
    fn action(&mut self, action: Action, cx: &mut Context<Self>) {
        use self::Action::*;
        match action {
            Fill | Shadow | Softness => unreachable!("selector setting"),
            Sample(index) => self.choose(index, cx),
            Motion(mode) => {
                self.motion = if self.motion == mode { MotionMode::Static } else { mode };
                self.playing = self.motion == MotionMode::Wave;
                // A newly selected reveal effect must show its initial preview, even
                // after the previous effect completed. Samples share this state.
                self.progress = 0.;
                self.phase = 0.;
                self.last_tick = Instant::now();
            }
            Play => {
                if self.motion == MotionMode::Static {
                    self.motion = MotionMode::Wave;
                }
                if self.progress >= 1. && matches!(self.motion, MotionMode::Typewriter | MotionMode::Scramble) {
                    self.progress = 0.;
                }
                self.playing = !self.playing;
                self.last_tick = Instant::now();
            }
            Restart => {
                self.progress = 0.;
                self.phase = 0.;
                self.last_tick = Instant::now();
            }
            Back | Next => {
                if self.motion != MotionMode::Scramble {
                    self.motion = MotionMode::Typewriter;
                }
                self.playing = false;
                self.progress = (self.progress + if action == Back { -0.1 } else { 0.1 }).clamp(0., 1.);
            }
            Seed => {
                self.seed = self.seed.wrapping_add(1);
                self.playing = false;
            }
            Frame => {
                self.phase += 1.;
                self.playing = false;
            }
            Reduced => self.reduced = !self.reduced,
            SelectAll => self.select_all(cx),
            Copy => self.copy(cx),
            Width => self.narrow = !self.narrow,
            Size => self.large = !self.large,
            Theme => {
                self.light = !self.light;
                self.apply_style(cx);
            }
        }
        cx.notify();
    }
    pub(super) fn suspend(&mut self) {
        self.playing = false;
        self.focus_pending = false;
        self.last_tick = Instant::now();
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::{Action, MotionMode, SAMPLES, TypographyDemo};
    use gpui::{Focusable, TestAppContext};
    use gpui_luma::controls::button::ButtonEvent;
    use xgpui_shader::typography::KineticEffect;

    #[test]
    fn luma_effects_preserve_canonical_selection_and_unicode_clipboard() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| TypographyDemo::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        let click = |action, cx: &mut gpui::VisualTestContext| {
            cx.update(|_, cx| {
                let demo = page.read(cx);
                if let Some((_, choice)) = demo.choices.iter().find(|(key, _)| *key == action) {
                    let index = match action {
                        Action::Fill => (demo.fill + 1) % 3,
                        Action::Shadow => 1,
                        Action::Softness => 2,
                        _ => unreachable!(),
                    };
                    choice.clone().select(index, cx);
                } else {
                    let button = demo.buttons.iter().find(|(key, _, _)| *key == action).unwrap().2.clone();
                    button.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
                }
            });
            cx.run_until_parked();
        };
        click(Action::Sample(2), cx);
        click(Action::SelectAll, cx);
        let text = cx.update(|_, cx| page.read(cx).samples[2].clone());
        let identity = text.entity_id();
        for action in [
            Action::Motion(MotionMode::Typewriter),
            Action::Restart,
            Action::Next,
            Action::Motion(MotionMode::Scramble),
            Action::Seed,
            Action::Frame,
            Action::Fill,
            Action::Fill,
            Action::Fill,
            Action::Shadow,
            Action::Softness,
            Action::Width,
            Action::Size,
            Action::Theme,
        ] {
            click(action, cx);
            cx.update(|_, cx| {
                assert_eq!(page.read(cx).samples[2].entity_id(), identity);
                assert_eq!(text.read(cx).text(), SAMPLES[2].1);
                assert_eq!(text.read(cx).selected_text(), SAMPLES[2].1);
            });
        }
        click(Action::Copy, cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), SAMPLES[2].1);
        click(Action::Reduced, cx);
        cx.update(|_, cx| assert_eq!(text.read(cx).effect(), KineticEffect::Static));
        click(Action::Reduced, cx);
        cx.update(|window, cx| window.focus(&text.focus_handle(cx), cx));
        cx.simulate_keystrokes("end shift-left");
        cx.update(|_, cx| assert_eq!(text.read(cx).selected_text(), "🇺🇸"));
        cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-c" } else { "ctrl-c" });
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "🇺🇸");
        click(Action::Sample(1), cx);
        click(Action::Sample(2), cx);
        cx.update(|_, cx| assert_eq!(text.read(cx).selected_text(), "🇺🇸"));
        page.update(cx, |page, cx| {
            page.action(Action::Motion(MotionMode::Wave), cx);
            assert!(page.playing);
            page.suspend();
            assert!(!page.playing);
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(matches!(text.read(cx).effect(), KineticEffect::Wave { .. }));
            assert_eq!(text.read(cx).selected_text(), "🇺🇸");
        });
    }
    #[test]
    fn motion_selection_resets_completed_previews_and_toggles_across_samples() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| TypographyDemo::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        page.update(cx, |demo, cx| {
            demo.progress = 1.;
            demo.phase = 48.;
            demo.action(Action::Motion(MotionMode::Scramble), cx);
            assert_eq!(demo.motion, MotionMode::Scramble);
            assert_eq!(demo.progress, 0.);
            assert_eq!(demo.phase, 0.);
            assert!(!demo.playing);
            demo.action(Action::Play, cx);
            assert!(demo.playing);
            demo.action(Action::Sample(1), cx);
            demo.action(Action::Sample(0), cx);
            assert_eq!(demo.motion, MotionMode::Scramble);
            demo.action(Action::Motion(MotionMode::Scramble), cx);
            assert_eq!(demo.motion, MotionMode::Static);
            assert!(!demo.playing);
            demo.progress = 1.;
            demo.action(Action::Motion(MotionMode::Typewriter), cx);
            assert_eq!(demo.progress, 0.);
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            let demo = page.read(cx);
            assert_eq!(demo.samples[0].read(cx).effect(), KineticEffect::Typewriter { progress: 0. });
        });
    }

    #[test]
    fn pointer_drag_word_and_line_selection_use_the_original_text() {
        use gpui::{Modifiers, MouseButton, MouseDownEvent, VisualTestContext, div, point, prelude::*, px, size};
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let window =
            app.open_window(size(px(1400.), px(1800.)), |_, cx| TypographyDemo::new(crate::theme::default_look(), cx));
        let page = window.root(&mut app).unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), &app);
        let draw = |cx: &mut VisualTestContext| {
            cx.draw(point(px(0.), px(0.)), size(px(1400.), px(1800.)), |_, _| div().size_full().child(page.clone()));
        };
        draw(&mut cx);
        let bounds = cx.debug_bounds("typography-surface").expect("mounted selectable surface");
        let start = point(bounds.left() + px(30.), bounds.center().y);
        cx.simulate_event(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 2,
            modifiers: Modifiers::default(),
            first_mouse: false,
        });
        cx.simulate_mouse_up(start, MouseButton::Left, Modifiers::default());
        cx.update(|_, cx| {
            let page = page.read(cx);
            assert_eq!(page.samples[0].read(cx).selected_text(), "Good");
        });
        cx.simulate_event(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 3,
            modifiers: Modifiers::default(),
            first_mouse: false,
        });
        cx.simulate_mouse_up(start, MouseButton::Left, Modifiers::default());
        cx.update(|_, cx| assert_eq!(page.read(cx).samples[0].read(cx).selected_text(), SAMPLES[0].1));
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        let end = start + point(px(130.), px(0.));
        cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
        cx.update(|_, cx| {
            let page = page.read(cx);
            let text = page.samples[0].read(cx);
            assert!(!text.selection().is_empty());
            assert!(text.selection().len() < SAMPLES[0].1.len());
            assert_eq!(text.selected_text(), &SAMPLES[0].1[text.selection()]);
        });
    }
}
