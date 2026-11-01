use crate::chrome_tokens::disabled_overlay;
use crate::style::active_color_control_theme;
use gpui::Hsla;

#[derive(Clone, Copy)]
pub struct ColorArcVisual {
    pub disabled_overlay: Hsla,
}

pub fn default_color_arc_visual(enabled: bool) -> ColorArcVisual {
    let theme = active_color_control_theme();

    ColorArcVisual {
        disabled_overlay: if enabled {
            theme.background.opacity(0.0)
        } else {
            disabled_overlay(theme.background)
        },
    }
}
