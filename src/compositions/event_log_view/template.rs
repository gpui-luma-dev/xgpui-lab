use std::sync::Arc;

use gpui::{App, Div, KeyDownEvent, MouseButton, MouseDownEvent, ScrollWheelEvent, Stateful, Window, div, prelude::*, px};

use gpui_luma::controls::scroll_container::ScrollContainer;
use crate::theme::TypographyExt;

use super::model::EventLogViewRenderModel;
use super::theme::EventLogTheme;

type ScrollWheelHandler = Box<dyn Fn(&ScrollWheelEvent, &mut Window, &mut App) + 'static>;

type MouseDownHandler = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;
type KeyDownHandler = Box<dyn Fn(&KeyDownEvent, &mut Window, &mut App) + 'static>;

pub struct EventLogViewTemplateHandlers {
    pub mouse_down: MouseDownHandler,
    pub key_down: KeyDownHandler,
    pub scroll_wheel: ScrollWheelHandler,
}

pub trait EventLogViewTemplate: Send + Sync {
    fn render(
        &self,
        model: &EventLogViewRenderModel<'_>,
        scroll: &ScrollContainer,
        handlers: EventLogViewTemplateHandlers,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div>;
}

pub struct ThemedEventLogViewTemplate;

impl ThemedEventLogViewTemplate {
    pub fn new(_theme: Arc<dyn EventLogTheme>) -> Self {
        Self
    }
}

impl EventLogViewTemplate for ThemedEventLogViewTemplate {
    fn render(
        &self,
        model: &EventLogViewRenderModel<'_>,
        scroll: &ScrollContainer,
        handlers: EventLogViewTemplateHandlers,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Stateful<Div> {
        let look = &model.look;
        let show_placeholder = model.text.is_empty();
        let text_color = if show_placeholder {
            look.placeholder
        } else {
            look.foreground
        };

        let content = div()
            .id(format!("{}-content", model.id))
            .min_h(px(look.min_height))
            .on_mouse_down(MouseButton::Left, handlers.mouse_down)
            .w_full()
            .px(px(look.padding_x))
            .py(px(look.padding_y))
            .child(
                div()
                    .w_full()
                    .font_family(look.font_family.clone())
                    .typography_style(look.typography)
                    .text_color(text_color)
                    .child(if show_placeholder {
                        model.placeholder.to_string()
                    } else {
                        model.text.to_owned()
                    }),
            )
            .into_any_element();

        let mut root = div()
            .id(model.id.clone())
            .track_focus(model.focus_handle)
            .on_key_down(handlers.key_down)
            .on_scroll_wheel(handlers.scroll_wheel)
            .overflow_hidden()
            .rounded(px(look.radius))
            .border(px(look.border_width))
            .border_color(look.border)
            .bg(look.background)
            .h(px(look.min_height))
            .min_h(px(look.min_height))
            .when(model.full_width, |element| element.w_full());

        // The control applies wheel input only after checking its live focus.
        let viewport = scroll.render_without_wheel(content);
        root = root.child(viewport.w_full().h(px(look.min_height)));
        root
    }
}

pub fn default_event_log_template(theme: Arc<dyn EventLogTheme>) -> Arc<dyn EventLogViewTemplate> {
    Arc::new(ThemedEventLogViewTemplate::new(theme))
}
