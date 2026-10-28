use super::*;
use gpui::{prelude::*, *};
use xgpui_shader::{
    instanced::{AnimatedBorder, AnimatedBorderStyle, ShadowPainter, ShadowStop, ShadowStyle},
    painted,
};

#[derive(Clone)]
pub struct ShadowDemo {
    painter: ShadowPainter,
    border: AnimatedBorder,
    pub blur: f32,
    pub spread: f32,
    pub falloff: f32,
    pub offset: f32,
    pub gradient: bool,
    pub inset: bool,
    pub card_dim: bool,
    pub clicks: usize,
}
impl ShadowDemo {
    pub fn new() -> Self {
        Self {
            border: AnimatedBorder::new().expect("border shader"),
            painter: ShadowPainter::new().expect("shadow shader"),
            blur: 24.,
            spread: 0.,
            falloff: 1.,
            offset: 12.,
            gradient: true,
            inset: false,
            card_dim: false,
            clicks: 0,
        }
    }
    pub fn border_button(&self, phase: f32, buttons: &[Entity<Button>]) -> impl IntoElement {
        div().child(
            painted()
                .rounded(px(999.))
                .bg(rgb(0xffffff))
                .animated_border(self.border.layer(AnimatedBorderStyle::default(), phase).expect("border frame"))
                .px_6()
                .py_3()
                .text_color(rgb(0x10151f))
                .child(buttons[2].clone()),
        )
    }
    fn style(&self) -> ShadowStyle {
        ShadowStyle {
            stops: if self.gradient {
                vec![
                    ShadowStop::new(0., [0.98, 0.16, 0.45, 0.85]),
                    ShadowStop::new(0.35, [0.55, 0.25, 1., 0.8]),
                    ShadowStop::new(0.7, [0.05, 0.6, 1., 0.8]),
                    ShadowStop::new(1., [0.1, 0.85, 0.65, 0.75]),
                ]
            } else {
                vec![ShadowStop::new(0., [0., 0., 0., 0.6])]
            },
            angle: 90.,
            offset: point(px(self.offset * 0.35), px(self.offset)),
            blur_radius: px(self.blur),
            spread_radius: px(self.spread),
            falloff: self.falloff,
            inset: self.inset,
        }
    }
    /// Both cards are real styled elements; no canvas or manual shadow paint calls.
    pub fn render(&self, opacity: f32, buttons: &[Entity<Button>]) -> Stateful<Div> {
        let native_color = if self.gradient {
            rgba(0x7160d9b3)
        } else {
            rgba(0x00000099)
        };
        let native = BoxShadow {
            color: native_color.into(),
            offset: point(px(self.offset * 0.35), px(self.offset)),
            blur_radius: px(self.blur),
            spread_radius: px(self.spread),
            inset: self.inset,
        };
        let layer = self.painter.layer(self.style()).expect("validated demo shadow style");
        let card_opacity = if self.card_dim { 0.5 } else { 1. };
        let native_button = buttons[0].clone();
        let helper_button = buttons[1].clone();
        let native_card = div()
            .w_full()
            .max_w(px(240.))
            .h(px(160.))
            .rounded(px(16.))
            .bg(rgb(0xf8fafd))
            .shadow(vec![native])
            .opacity(card_opacity)
            .overflow_hidden()
            .p_4()
            .flex()
            .flex_col()
            .justify_between()
            .text_color(rgb(0x22304a))
            .child(div().child("Standard GPUI"))
            .child(div().text_sm().child("A normal styled div"))
            .child(native_button);
        let helper_card = painted()
            .w_full()
            .max_w(px(240.))
            .h(px(160.))
            .rounded(px(16.))
            .bg(rgb(0xf8fafd))
            .shadow_paint(layer)
            .opacity(card_opacity)
            .overflow_hidden()
            .p_4()
            .flex()
            .flex_col()
            .justify_between()
            .text_color(rgb(0x22304a))
            .child(div().child("Extension helper"))
            .child(div().text_sm().child("painted().shadow_paint(layer)"))
            .child(helper_button);
        div()
            .id("shadow-helper-demo")
            .w_full()
            .flex_1()
            .min_h(px(180.))
            .overflow_hidden()
            .bg(rgb(0xe6ebf3))
            .opacity(opacity)
            .flex()
            .items_center()
            .child(div().w_1_2().px_8().flex().justify_center().child(native_card))
            .child(div().w_1_2().px_8().flex().justify_center().child(helper_card))
    }
}

pub(super) struct ShadowPane {
    shadow: ShadowDemo,
    pub borders: bool,
    pub animated: bool,
    phase: f32,
    last: std::time::Instant,
    buttons: Vec<Entity<Button>>,
    sliders: Vec<Slider>,
    last_clicks: usize,
}
impl ShadowPane {
    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let light = gpui_luma_look_radix::Look::built_in();
        light.set_mode(gpui_luma::theme::ThemeMode::Light);
        let buttons = [
            "Native child",
            "Extension child",
            "Generate",
            "Animate / Pause",
            "Gradient / solid",
            "Inset / outset",
            "Card opacity",
        ]
        .into_iter()
        .enumerate()
        .map(|(id, label)| {
            radix::Button::new(format!("shadow-{id}"))
                .look(if id < 3 { &light } else { look.as_ref() })
                .outline()
                .label(label)
                .on_click(cx, move |this: &mut Self, cx| {
                    match id {
                        0..=2 => this.shadow.clicks += 1,
                        3 => {
                            this.animated = !this.animated;
                            this.last = std::time::Instant::now();
                        }
                        4 => this.shadow.gradient = !this.shadow.gradient,
                        5 => this.shadow.inset = !this.shadow.inset,
                        _ => this.shadow.card_dim = !this.shadow.card_dim,
                    }
                    cx.notify();
                })
                .spawn(cx)
        })
        .collect();
        let sliders = [(0., 64., 24.), (-12., 24., 0.), (0.5, 4., 1.), (0., 24., 12.)]
            .into_iter()
            .enumerate()
            .map(|(id, (min, max, value))| {
                radix::Slider::new(format!("shadow-value-{id}"))
                    .look(look.as_ref())
                    .range(min..max)
                    .value(value)
                    .step(0.5)
                    .on_value_changed(cx, move |this: &mut Self, _, value, cx| {
                        match id {
                            0 => this.shadow.blur = value,
                            1 => this.shadow.spread = value,
                            2 => this.shadow.falloff = value,
                            _ => this.shadow.offset = value,
                        }
                        cx.notify();
                    })
                    .spawn(cx)
            })
            .collect();
        Self {
            shadow: ShadowDemo::new(),
            borders: false,
            animated: false,
            phase: 0.,
            last: std::time::Instant::now(),
            buttons,
            sliders,
            last_clicks: usize::MAX,
        }
    }
}
impl Render for ShadowPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = std::time::Instant::now();
        if self.borders && self.animated && !cx.reduce_motion() {
            self.phase += (now - self.last).as_secs_f32().min(0.1);
            window.request_animation_frame();
        }
        self.last = now;
        if self.last_clicks != self.shadow.clicks {
            self.last_clicks = self.shadow.clicks;
            for button in self.buttons.iter().take(3) {
                button.update(cx, |button, cx| button.set_label(format!("Child clicks: {}", self.shadow.clicks), cx));
            }
        }
        let mut root = div().id("shadow-pane-scroll").size_full().overflow_y_scroll().p_4().flex().flex_col().gap_4();
        if self.borders {
            root=root.child("Soft animated borders · orbiting highlights")
                .child(self.buttons[3].clone())
                .child(format!("Animation {} · phase {:.2}",self.animated&&!cx.reduce_motion(),self.phase))
                .child(div().p_8().bg(rgb(0xf8f9fb)).child(self.shadow.border_button(self.phase,&self.buttons)))
                .child("The decorated button is a Luma control. Click to verify child interaction; reduced motion freezes animation.");
        } else {
            root=root.child("Gradient shadows · native versus extension helper")
                .child(div().flex().gap_3().children(self.buttons.iter().skip(4).cloned()))
                .child(div().flex().flex_wrap().gap_4().children(self.sliders.iter().enumerate().map(|(id,slider)| {
                    let (label,value)=match id {0=>("Blur",self.shadow.blur),1=>("Spread",self.shadow.spread),2=>("Falloff",self.shadow.falloff),_=>("Offset",self.shadow.offset)};
                    div().w(px(190.)).flex().flex_col().gap_2().child(format!("{label}: {value:.1}")).child(slider.clone())
                })))
                .child(format!("Gradient {} · Inset {} · Card opacity {}",self.shadow.gradient,self.shadow.inset,if self.shadow.card_dim {0.5}else{1.}))
                .child(self.shadow.render(1.,&self.buttons))
                .child("Click either Luma child button. The same blur, spread, offset and inset parameters feed the native and extension cards; extension falloff and gradient stops are additional capabilities.");
        }
        root
    }
}
