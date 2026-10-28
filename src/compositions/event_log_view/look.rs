use std::sync::Arc;

use gpui::SharedString;
use gpui_luma::controls::textarea::TextAreaState;
use gpui_luma::theme::{ControlSize, StandardBoxScale};
use crate::theme::{Look, ColorVizLookExt, TextSize};

use super::model::EventLogViewBuilder;
use super::theme::{EventLogTheme, compose_event_log_look};

struct RadixEventLogTheme {
    look: Arc<Look>,
}

impl EventLogTheme for RadixEventLogTheme {
    fn metrics(&self) -> gpui_luma::theme::MetricTokens {
        self.look.metrics()
    }

    fn resolve_look(&self, rows: usize, scale: &StandardBoxScale, focused: bool) -> super::theme::EventLogLook {
        let textarea = gpui_luma_look_radix::textarea_theme(&self.look).resolve_look(
            TextAreaState { focused, focus_visible: focused, ..Default::default() },
            true,
            ControlSize::Md,
            scale,
        );
        let typography = self.look.typography_scale(TextSize::Xs);

        compose_event_log_look(
            textarea.background,
            textarea.foreground,
            textarea.border,
            textarea.placeholder,
            typography,
            textarea.font_family,
            rows,
            scale,
            textarea.border_width,
        )
    }
}

pub trait EventLogViewLookExt {
    fn event_log_view(&self, id: impl Into<SharedString>) -> EventLogViewBuilder;
}

impl EventLogViewLookExt for Arc<Look> {
    fn event_log_view(&self, id: impl Into<SharedString>) -> EventLogViewBuilder {
        EventLogViewBuilder::with_parts(id, radix_event_log_theme(Arc::clone(self)), self.scrollbar_template())
    }
}

pub fn radix_event_log_theme(look: Arc<Look>) -> Arc<dyn EventLogTheme> {
    Arc::new(RadixEventLogTheme { look })
}
