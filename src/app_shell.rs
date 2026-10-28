use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_luma::shell::{TitleBar, TITLE_BAR_HEIGHT};

pub(crate) const SHELL_TITLEBAR_HEIGHT: gpui::Pixels = px(40.0);

use crate::app::XgpuiLabApp;

pub fn open(cx: &mut App) -> anyhow::Result<()> {
    let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);

    let mut titlebar = TitleBar::title_bar_options();
    if let Some(position) = &mut titlebar.traffic_light_position {
        position.y += (SHELL_TITLEBAR_HEIGHT - TITLE_BAR_HEIGHT) / 2.0;
    }
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(720.0), px(480.0))),
            titlebar: Some(titlebar),
            app_owns_titlebar_drag: true,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| XgpuiLabApp::new(window, cx)),
    )?;

    cx.activate(true);
    Ok(())
}
