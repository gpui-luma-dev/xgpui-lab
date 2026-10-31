use gpui::{hsla, Hsla};

/// Blocked-segment overlay for color slider tracks.
pub fn slider_blocked_overlay(is_dark: bool) -> Hsla {
    if is_dark {
        hsla(0., 0., 0.18, 1.)
    } else {
        hsla(0., 0., 0.72, 1.)
    }
}

/// Checkerboard base/accent pair for translucent color swatches.
pub fn swatch_checkerboard_colors(is_dark: bool) -> (Hsla, Hsla) {
    if is_dark {
        (hsla(0., 0., 0.1, 1.), hsla(0., 0., 0.13, 1.))
    } else {
        (hsla(0., 0., 1.0, 1.), hsla(0., 0., 0.95, 1.))
    }
}

/// Disabled-state wash applied over color-control chrome surfaces.
pub fn disabled_overlay(background: Hsla) -> Hsla {
    background.opacity(0.45)
}
