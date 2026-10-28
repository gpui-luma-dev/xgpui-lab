use std::sync::Arc;

use gpui::{AppContext, Entity, FocusHandle, SharedString};

use gpui_luma::controls::scrollbar::ScrollbarTemplate;

use super::control::EventLogView;
use super::theme::EventLogTheme;

#[derive(Clone)]
pub struct EventLogViewModel {
    pub(crate) id: SharedString,
    pub(crate) placeholder: SharedString,
    pub(crate) rows: usize,
    pub(crate) full_width: bool,
    pub(crate) font_family: Option<String>,
    pub(crate) theme: Arc<dyn EventLogTheme>,
    pub(crate) scrollbar_template: Arc<dyn ScrollbarTemplate>,
}

pub struct EventLogViewRenderModel<'a> {
    pub id: &'a SharedString,
    pub text: &'a str,
    pub placeholder: &'a SharedString,
    pub full_width: bool,
    pub look: super::theme::EventLogLook,
    pub focus_handle: &'a FocusHandle,
}

pub struct EventLogViewBuilder {
    pub(crate) model: EventLogViewModel,
}

impl EventLogViewBuilder {
    pub(crate) fn with_parts(
        id: impl Into<SharedString>,
        theme: Arc<dyn EventLogTheme>,
        scrollbar_template: Arc<dyn ScrollbarTemplate>,
    ) -> Self {
        Self {
            model: EventLogViewModel {
                id: id.into(),
                placeholder: SharedString::default(),
                rows: 4,
                full_width: false,
                font_family: None,
                theme,
                scrollbar_template,
            },
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.model.placeholder = placeholder.into();
        self
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.model.rows = rows.max(1);
        self
    }

    pub fn full_width(mut self, full_width: bool) -> Self {
        self.model.full_width = full_width;
        self
    }

    pub fn font_family(mut self, font_family: impl Into<String>) -> Self {
        self.model.font_family = Some(font_family.into());
        self
    }

    pub fn spawn(self, cx: &mut impl AppContext) -> Entity<EventLogView> {
        cx.new(|cx| EventLogView::from_builder(self, cx))
    }
}
