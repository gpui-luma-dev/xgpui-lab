use std::sync::Arc;

use gpui::{Context, Entity, Render, Window, div, prelude::*};
use gpui_luma_look_radix::Look;

/// An extension's navigation and lab content, independent of the original demos.
pub struct ExtensionPane {
    look: Arc<Look>,
    interactive_spaces: Option<Entity<crate::color_demos::InteractiveSpaces>>,
    italy: Option<Entity<crate::color_demos::ItalyLab>>,
    chromaticity: Option<Entity<crate::color_demos::ChromaticityLab>>,
    active_demo: Option<gpui::AnyView>,
    benchmarks: Option<Entity<crate::benchmarks::Page>>,
    benchmarks_active: bool,
    gamut: Option<Entity<crate::gamut::GamutPage>>,
    shader: Option<Entity<crate::shader::ShaderPage>>,
    color: Option<Entity<crate::color_lab::ColorPage>>,
    media: Option<Entity<crate::media_lab::MediaPage>>,
    parley: Option<Entity<crate::parley_lab::ParleyPage>>,
    svg: Option<Entity<crate::svg_lab::SvgPage>>,
}

impl ExtensionPane {
    pub fn requires_sdr(&self, cx: &gpui::App) -> bool {
        self.shader.as_ref().is_some_and(|page| page.read(cx).requires_sdr())
            || self.media.as_ref().is_some_and(|page| page.read(cx).requires_sdr(cx))
    }

    pub fn deactivate(&mut self, cx: &mut Context<Self>) {
        if let Some(page) = &self.interactive_spaces {
            page.update(cx, |page, cx| page.deactivate(cx));
        }
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

    pub(crate) fn activate_page(&mut self, page: &'static str, cx: &mut Context<Self>) {
        if let Some(view) = &self.interactive_spaces {
            view.update(cx, |view, cx| view.deactivate(cx));
        }
        self.active_demo = match page {
            "Color Spaces" if self.color.is_some() => {
                let view = self.interactive_spaces.get_or_insert_with(|| {
                    cx.new(|cx| crate::color_demos::InteractiveSpaces::new(self.look.clone(), cx))
                });
                Some(view.clone().into())
            }
            "Italy Photo Comparison" if self.color.is_some() => {
                let view = self
                    .italy
                    .get_or_insert_with(|| cx.new(|cx| crate::color_demos::ItalyLab::new(self.look.clone(), cx)));
                view.update(cx, |view, cx| view.activate(cx));
                Some(view.clone().into())
            }
            "Chromaticity & Gamut" if self.gamut.is_some() => {
                let view = self.chromaticity.get_or_insert_with(|| {
                    cx.new(|cx| crate::color_demos::ChromaticityLab::new(self.look.clone(), cx))
                });
                Some(view.clone().into())
            }
            _ => None,
        };
        if self.active_demo.is_some() {
            self.deactivate(cx);
            cx.notify();
            return;
        }
        let tab_id: gpui::SharedString = page.into();
        self.benchmarks_active = tab_id == "Benchmarks" && self.benchmarks.is_some();
        cx.notify();
        if let Some(gamut) = &self.gamut {
            gamut.update(cx, |page, cx| {
                page.activate(tab_id.clone(), cx);
            });
        }
        if let Some(media) = &self.media {
            media.update(cx, |page, cx| page.activate(tab_id.clone(), cx));
        }
        if let Some(parley) = &self.parley {
            parley.update(cx, |page, cx| page.activate(tab_id.clone(), cx));
        }
        if let Some(svg) = &self.svg {
            svg.update(cx, |page, cx| page.activate(tab_id.clone(), cx));
        }
        if let Some(color) = &self.color {
            color.update(cx, |page, cx| {
                page.active_tab = tab_id.clone();
                cx.notify();
            });
        }
        if let Some(shader) = &self.shader {
            shader.update(cx, |page, cx| page.activate(tab_id.clone(), cx));
        }
    }

    pub fn new(id: &'static str, look: Arc<Look>, p3_error: Option<String>, cx: &mut Context<Self>) -> Self {
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
        Self {
            look,
            interactive_spaces: None,
            italy: None,
            chromaticity: None,
            active_demo: None,
            benchmarks,
            benchmarks_active: false,
            gamut,
            shader,
            color,
            media,
            svg,
            parley,
        }
    }
}

impl Render for ExtensionPane {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(view) = &self.active_demo {
            return div().size_full().child(view.clone()).into_any_element();
        }
        div()
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
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
            .into_any_element()
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[test]
    fn native_color_labs_are_lazy_and_retained_when_returning_to_existing_pages() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        for (id, lab, existing) in [
            ("xgpui-color-tabs", "Italy Photo Comparison", "Conversions"),
            ("xgpui-color-tabs", "Color Spaces", "Conversions"),
            ("xgpui-gamut-tabs", "Chromaticity & Gamut", "Wide Color Fills"),
        ] {
            let (view, cx) =
                app.add_window_view(|_, cx| ExtensionPane::new(id, crate::theme::default_look(), None, cx));
            cx.run_until_parked();
            cx.update(|_, cx| {
                let pane = view.read(cx);
                assert!(pane.interactive_spaces.is_none() && pane.italy.is_none() && pane.chromaticity.is_none());
            });
            view.update(cx, |pane, cx| pane.activate_page(lab, cx));
            cx.run_until_parked();
            let first = cx.update(|_, cx| {
                let pane = view.read(cx);
                let active = pane.active_demo.as_ref().unwrap().entity_id();
                if lab == "Color Spaces" {
                    assert_eq!(active, pane.interactive_spaces.as_ref().unwrap().entity_id());
                }
                active
            });
            view.update(cx, |pane, cx| pane.activate_page(existing, cx));
            cx.run_until_parked();
            cx.update(|_, cx| assert!(view.read(cx).active_demo.is_none()));
            view.update(cx, |pane, cx| pane.activate_page(lab, cx));
            cx.run_until_parked();
            cx.update(|_, cx| assert_eq!(view.read(cx).active_demo.as_ref().unwrap().entity_id(), first));
        }
    }
}
