use gpui::Hsla;

use gpui_luma::theme::{LumaTextStyle, MetricTokens, StandardBoxScale};

#[derive(Clone, Debug)]
pub struct EventLogLook {
    pub background: Hsla,
    pub foreground: Hsla,
    pub border: Hsla,
    pub placeholder: Hsla,
    pub typography: LumaTextStyle,
    pub font_family: String,
    pub min_height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub radius: f32,
    pub border_width: f32,
}

pub trait EventLogTheme: Send + Sync {
    fn metrics(&self) -> MetricTokens;

    fn resolve_look(&self, rows: usize, scale: &StandardBoxScale, focused: bool) -> EventLogLook;
}

#[allow(clippy::too_many_arguments)]
pub fn compose_event_log_look(
    background: Hsla,
    foreground: Hsla,
    border: Hsla,
    placeholder: Hsla,
    typography: LumaTextStyle,
    font_family: String,
    rows: usize,
    scale: &StandardBoxScale,
    border_width: f32,
) -> EventLogLook {
    let row_block = typography.line_height * rows as f32;
    EventLogLook {
        background,
        foreground,
        border,
        placeholder,
        typography,
        font_family,
        min_height: row_block + scale.padding_y * 2.0,
        padding_x: scale.padding_x,
        padding_y: scale.padding_y,
        radius: scale.radius,
        border_width,
    }
}
