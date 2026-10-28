use std::sync::Arc;
use gpui_luma::prelude::HasPresenter;

use gpui::{Context, Entity, Render, Window, div, prelude::*, px};
use gpui_luma::controls::tabs::{Tabs, TabsEvent, TabsItem, TabsWidthMode};
use gpui_luma::theme::ControlSize;
use gpui_luma_look_radix::Look;

use crate::theme::ColorVizLookExt;

/// An extension's navigation and lab content, independent of the original demos.
pub struct ExtensionPane {
    look: Arc<Look>,
    benchmarks: Option<Entity<crate::benchmarks::Page>>,
    benchmarks_active: bool,
    tabs: Entity<Tabs>,
    tab_width: f32,
    tab_scroll: gpui::ScrollHandle,
    scroll_buttons: Vec<Entity<gpui_luma::controls::button::Button>>,
    gamut: Option<Entity<crate::gamut::GamutPage>>,
    shader: Option<Entity<crate::shader::ShaderPage>>,
    color: Option<Entity<crate::color_lab::ColorPage>>,
    media: Option<Entity<crate::media_lab::MediaPage>>,
    parley: Option<Entity<crate::parley_lab::ParleyPage>>,
    svg: Option<Entity<crate::svg_lab::SvgPage>>,
    _subscription: gpui::Subscription,
}

impl ExtensionPane {
    pub fn requires_sdr(&self, cx: &gpui::App) -> bool {
        self.shader.as_ref().is_some_and(|page| page.read(cx).requires_sdr())
            || self.media.as_ref().is_some_and(|page| page.read(cx).requires_sdr(cx))
    }

    pub fn deactivate(&mut self, cx: &mut Context<Self>) {
        if let Some(shader) = &self.shader {
            shader.update(cx, |page, cx| page.suspend(cx));
        }
        if let Some(svg) = &self.svg {
            svg.update(cx, |page, _| page.suspend());
        }
        if let Some(media) = &self.media {
            media.update(cx, |page, _| page.suspend());
        }
    }

    pub fn new(
        id: &'static str,
        labels: &[&'static str],
        look: Arc<Look>,
        p3_error: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let tab_scroll = gpui::ScrollHandle::new();
        let scroll_buttons = [-1., 1.]
            .into_iter()
            .enumerate()
            .map(|(index, direction)| {
                let scroll = tab_scroll.clone();
                gpui_luma_look_radix::Button::new(format!("{id}-scroll-{index}"))
                    .look(look.as_ref())
                    .outline()
                    .label(if index == 0 { "‹" } else { "›" })
                    .on_click(cx, move |_: &mut Self, cx| {
                        let mut offset = scroll.offset();
                        offset.x = (offset.x - px(direction * 440.)).clamp(-scroll.max_offset().x, px(0.));
                        scroll.set_offset(offset);
                        cx.notify();
                    })
                    .spawn(cx)
            })
            .collect();
        let tabs = Tabs::new(id)
            .template(crate::studio_tabs::template(look.clone(), true))
            .items(labels.iter().map(|label| TabsItem::new(*label).label(*label)).collect::<Vec<_>>())
            .size(ControlSize::Sm)
            .width_mode(TabsWidthMode::Uniform)
            .active(labels[0])
            .spawn(cx);
        let gamut =
            (id == "xgpui-gamut-tabs").then(|| cx.new(|cx| crate::gamut::GamutPage::new(look.clone(), p3_error, cx)));
        let shader = (id == "xgpui-shader-tabs").then(|| cx.new(|cx| crate::shader::ShaderPage::new(look.clone(), cx)));
        let color = (id == "xgpui-color-tabs").then(|| cx.new(|cx| crate::color_lab::ColorPage::new(look.clone(), cx)));
        let media = (id == "xgpui-media-tabs").then(|| cx.new(|cx| crate::media_lab::MediaPage::new(look.clone(), cx)));
        let svg = (id == "xgpui-svg-tabs").then(|| cx.new(|cx| crate::svg_lab::SvgPage::new(look.clone(), cx)));
        let parley =
            (id == "xgpui-parley-tabs").then(|| cx.new(|cx| crate::parley_lab::ParleyPage::new(look.clone(), cx)));
        let suite = match id {
            "xgpui-parley-tabs" => Some(crate::benchmarks::Suite::Parley),
            "xgpui-shader-tabs" => Some(crate::benchmarks::Suite::Shader),
            "xgpui-svg-tabs" => Some(crate::benchmarks::Suite::Svg),
            _ => None,
        };
        let benchmarks = suite.map(|suite| cx.new(|cx| crate::benchmarks::Page::new(suite, look.clone(), cx)));
        let subscription = cx.subscribe(&tabs, |this, _, event: &TabsEvent, cx| {
            if let TabsEvent::Activate { tab_id, .. } = event {
                this.benchmarks_active = tab_id == "Benchmarks" && this.benchmarks.is_some();
                cx.notify();
                if let Some(gamut) = &this.gamut {
                    gamut.update(cx, |page, cx| {
                        page.activate(tab_id.clone(), cx);
                    });
                }
                if let Some(media) = &this.media {
                    media.update(cx, |page, cx| page.activate(tab_id.clone(), cx));
                }
                if let Some(parley) = &this.parley {
                    parley.update(cx, |page, cx| page.activate(tab_id.clone(), cx));
                }
                if let Some(svg) = &this.svg {
                    svg.update(cx, |page, cx| page.activate(tab_id.clone(), cx));
                }
                if let Some(color) = &this.color {
                    color.update(cx, |page, cx| {
                        page.active_tab = tab_id.clone();
                        cx.notify();
                    });
                }
                if let Some(shader) = &this.shader {
                    shader.update(cx, |page, cx| page.activate(tab_id.clone(), cx));
                }
            }
        });
        Self {
            tab_width: labels.len() as f32 * 145.,
            tab_scroll,
            scroll_buttons,
            look,
            benchmarks,
            benchmarks_active: false,
            tabs,
            gamut,
            shader,
            color,
            media,
            svg,
            parley,
            _subscription: subscription,
        }
    }
}

impl Render for ExtensionPane {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let chrome = self.look.chrome();
        div()
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .px(px(16.0))
                    .py(px(6.0))
                    .border_b_1()
                    .border_color(chrome.border)
                    .bg(chrome.panel_background)
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(self.tab_width > 870., |view| view.child(self.scroll_buttons[0].clone()))
                    .child(
                        div()
                            .id("extension-tab-scroll")
                            .flex_1()
                            .min_w_0()
                            .overflow_x_scroll()
                            .track_scroll(&self.tab_scroll)
                            .child(div().w(px(self.tab_width)).flex_shrink_0().child(self.tabs.clone())),
                    )
                    .when(self.tab_width > 870., |view| view.child(self.scroll_buttons[1].clone())),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .when(self.benchmarks_active, |view| view.children(self.benchmarks.clone()))
                    .when(!self.benchmarks_active, |view| {
                        view.children(self.gamut.clone())
                            .children(self.shader.clone())
                            .children(self.color.clone())
                            .children(self.media.clone())
                            .children(self.svg.clone())
                            .children(self.parley.clone())
                    }),
            )
    }
}
