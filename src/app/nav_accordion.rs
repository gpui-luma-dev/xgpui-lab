//! Lab section headers retain SDK accordion behavior with leading folder glyphs.
use std::sync::Arc;

use gpui::{App, Div, Stateful, Window, div, prelude::*, px};
use gpui_luma::controls::accordion::*;
use gpui_luma::theme::{ControlSize, InteractionState, MetricTokens};
use gpui_luma_look_radix::{Look, ScaleFamily};
use lucide_svg_static::Icon;

struct NavigationTheme {
    look: Look,
    radix: Arc<dyn AccordionTheme>,
}

impl AccordionTheme for NavigationTheme {
    fn resolve_trigger(&self, state: InteractionState, size: ControlSize) -> AccordionPalette {
        let mut palette = self.radix.resolve_trigger(state, size);
        palette.foreground = self.look.resolve_step(ScaleFamily::Gray, 11).hsla();
        palette.icon_color = palette.foreground;
        palette
    }

    fn resolve_content(&self, expanded: bool) -> AccordionContentPalette {
        self.radix.resolve_content(expanded)
    }

    fn metrics(&self) -> MetricTokens {
        self.radix.metrics()
    }

    fn resolve_scale(&self, size: ControlSize, scale_factor: f32) -> AccordionScale {
        let mut scale = self.radix.resolve_scale(size, scale_factor);
        // The custom trigger owns the disclosure glyph directly before its label.
        scale.chevron_size = 0.0;
        scale
    }
}

struct NavigationTemplate {
    look: Look,
    inner: ThemedAccordionTemplate,
}

impl AccordionTemplate for NavigationTemplate {
    fn render(
        &self,
        model: &AccordionRenderModel<'_>,
        handlers: AccordionTemplateHandlers,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        // Section IDs are their displayed labels. Read expansion from the SDK's
        // render model so keyboard, pointer, and programmatic changes stay in sync.
        let triggers: Vec<_> = model
            .items
            .iter()
            .map(|item| {
                let label = item.id.clone();
                let expanded = item.expanded;
                let look = self.look.clone();
                AccordionTrigger::custom(move |_, _| {
                    let color = look.resolve_step(ScaleFamily::Gray, 11).hsla();
                    let icon = if expanded { Icon::FolderOpen } else { Icon::FolderClosed };
                    let label_selector = format!("nav-label-{label}");
                    let selector = format!("nav-folder-{}-{}", label, if expanded { "open" } else { "closed" });
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .text_color(color)
                        .child(
                            div()
                                .debug_selector(move || selector.clone())
                                .flex_none()
                                .child(gpui_luma::infra::icon::lucide_icon(icon, color, 14.0)),
                        )
                        .child(div().debug_selector(move || label_selector.clone()).child(label.clone()))
                        .into_any_element()
                })
            })
            .collect();
        let items = model
            .items
            .iter()
            .zip(&triggers)
            .map(|(item, trigger)| AccordionItemRenderModel {
                id: item.id,
                trigger,
                content: item.content,
                expanded: item.expanded,
                progress: item.progress,
                content_height_px: item.content_height_px,
                enabled: item.enabled,
                state: item.state,
            })
            .collect();
        self.inner.render(
            &AccordionRenderModel {
                id: model.id,
                items,
                selection_mode: model.selection_mode,
                collapsible: model.collapsible,
                enabled: model.enabled,
                size: model.size,
                item_dividers: model.item_dividers,
                content_padding_y: model.content_padding_y,
                content_padding_top: model.content_padding_top,
                content_padding_bottom: model.content_padding_bottom,
                trigger_min_height: model.trigger_min_height,
                trigger_padding_y: model.trigger_padding_y,
                disclosure_icons: model.disclosure_icons,
                focus: model.focus,
            },
            handlers,
            window,
            cx,
        )
    }
}

pub(super) fn template(look: &Look) -> Arc<dyn AccordionTemplate> {
    let theme = Arc::new(NavigationTheme { look: look.clone(), radix: gpui_luma_look_radix::accordion_theme(look) });
    Arc::new(NavigationTemplate { look: look.clone(), inner: ThemedAccordionTemplate::new(theme) })
}
