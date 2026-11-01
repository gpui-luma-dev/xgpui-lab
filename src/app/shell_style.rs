// Shell geometry from gpui-luma/apps/shells/shell-2026.
use gpui::{Hsla, Pixels, px};
use gpui_luma_look_radix::Look;
use crate::theme::ColorVizLookExt;

pub(super) const RAIL_WIDTH: Pixels = px(52.0);
pub(crate) const SHELL_TITLEBAR_HEIGHT: Pixels = px(40.0);
pub(super) const CANVAS_BORDER: Pixels = px(1.0);
pub(super) const CANVAS_RADIUS: Pixels = px(10.0);
pub(super) const INITIAL_SIDEBAR_WIDTH: Pixels = px(260.0);
pub(super) const MIN_SIDEBAR_WIDTH: Pixels = px(200.0);
pub(super) const MAX_SIDEBAR_WIDTH: Pixels = px(480.0);
pub(super) const MIN_WORKSPACE_WIDTH: Pixels = px(380.0);
pub(super) const CANVAS_TOP_INSET: Pixels = px(2.0);
pub(super) const CANVAS_EDGE_INSET: Pixels = px(5.0);

pub(super) fn shell_background(look: &Look) -> Hsla {
    look.chrome().app_background
}

pub(super) fn canvas_border_color(look: &Look) -> Hsla {
    look.chrome().border
}
