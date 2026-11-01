use crate::chrome_tokens::{disabled_overlay, slider_blocked_overlay};
use crate::style::active_color_control_theme;
use gpui::Hsla;

#[derive(Clone, Copy)]
pub struct ColorSliderVisual {
    pub border: Hsla,
    pub disabled_overlay: Hsla,
    pub blocked_overlay: Hsla,
}

pub fn default_color_slider_visual(enabled: bool) -> ColorSliderVisual {
    let theme = active_color_control_theme();

    ColorSliderVisual {
        border: theme.border,
        disabled_overlay: if enabled {
            theme.background.opacity(0.0)
        } else {
            disabled_overlay(theme.background)
        },
        blocked_overlay: slider_blocked_overlay(theme.is_dark()),
    }
}
