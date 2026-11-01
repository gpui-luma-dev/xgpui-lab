use crate::chrome_tokens::disabled_overlay;
use crate::style::active_color_control_theme;
use gpui::Hsla;

#[derive(Clone, Copy)]
pub struct ColorRingVisual {
    pub border: Hsla,
    pub disabled_overlay: Hsla,
    pub center_hole: Hsla,
}

pub fn default_color_ring_visual(enabled: bool) -> ColorRingVisual {
    let theme = active_color_control_theme();

    ColorRingVisual {
        border: theme.border,
        disabled_overlay: if enabled {
            theme.background.opacity(0.0)
        } else {
            disabled_overlay(theme.background)
        },
        center_hole: theme.background,
    }
}
