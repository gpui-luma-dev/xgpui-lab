//! Color & Gamut uses the same SDK section tabs as the other lab workspaces.
use std::sync::Arc;
use gpui::{prelude::*, *};
use gpui_luma::{
    controls::tabs::{Tabs, TabsEvent, TabsItem, TabsWidthMode},
    theme::ControlSize,
};
use gpui_luma_look_radix::Look;
use crate::{chromaticity::Lab, theme::ColorVizLookExt};

pub(crate) struct Page {
    look: Arc<Look>,
    tabs: Entity<Tabs>,
    lab: Entity<Lab>,
    _subscription: Subscription,
}
impl Page {
    pub(crate) fn new(window: &mut Window, look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let lab = cx.new(|_| Lab::new(window, false));
        let tabs = Tabs::new("color-gamut-example-tabs")
            .template(crate::studio_tabs::template(look.clone(), true))
            .items(vec![
                TabsItem::new("chromaticity").label("Chromaticity & Metadata"),
                TabsItem::new("reveal").label("Hidden Wide-Gamut Symbol"),
                TabsItem::new("photo").label("Photo Comparison"),
            ])
            .size(ControlSize::Sm)
            .width_mode(TabsWidthMode::Uniform)
            .active("chromaticity")
            .spawn(cx);
        let subscription = cx.subscribe(&tabs, |this, _, event: &TabsEvent, cx| {
            if let TabsEvent::Activate { tab_id, .. } = event {
                this.lab.update(cx, |lab, cx| {
                    if tab_id == "photo" {
                        lab.set_photo();
                    } else {
                        lab.set_example(tab_id == "reveal");
                    }
                    cx.notify();
                });
                cx.notify();
            }
        });
        Self { look, tabs, lab, _subscription: subscription }
    }
}
impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
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
                    .px(px(16.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(chrome.border)
                    .bg(chrome.panel_background)
                    .child(self.tabs.clone()),
            )
            .child(div().flex_1().min_h_0().child(self.lab.clone()))
    }
}
