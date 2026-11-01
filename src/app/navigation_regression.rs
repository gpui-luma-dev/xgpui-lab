//! Native regression for the reported down-then-up Gamut navigation freeze.
use super::*;
use anyhow::{Context as _, Result, ensure};
use gpui::{AppContext, Bounds, WindowBounds, WindowColorMode, WindowOptions, px, size};
use serde_json::{Value, json};

pub(crate) const PAGES: &[&str] = &[
    "Wide Color Fills",
    "Wide Gradients",
    "Chromaticity & Gamut",
    "Gamut Mapping",
    "Presentation",
    "Benchmarks",
    "Presentation",
    "Gamut Mapping",
    "Chromaticity & Gamut",
    "Wide Gradients",
    "Wide Color Fills",
];

/// Run in a separate process with an external timeout: the original failure
/// blocks the UI thread, so an in-process async timeout cannot interrupt it.
pub(crate) fn run(cx: &mut gpui::App, output: &std::path::Path) -> Result<Value> {
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1400.), px(2200.)), cx))),
            show: false,
            focus: false,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| XgpuiLabApp::new(window, cx)),
    )?;
    let result = (|| -> Result<Value> {
        let mut steps = Vec::new();
        for cycle in 0..3 {
            for &page in PAGES {
                handle.update(cx, |app, window, cx| -> Result<()> {
                    app.activate(Workspace::Gamut, page, cx);
                    window.set_color_mode(WindowColorMode::WideGamutSdr)?;
                    ensure!(
                        window.color_status().effective() == WindowColorMode::WideGamutSdr,
                        "navigation regression requires native wide SDR"
                    );
                    Ok(())
                })??;
                cx.update_window(handle.into(), |_, window, cx| -> Result<()> {
                    for _ in 0..3 {
                        window.refresh();
                        window.draw(cx).clear(cx);
                        let image = window.render_to_image().context("native navigation readback")?;
                        ensure!(image.width() > 0 && image.height() > 0, "empty navigation frame");
                        if page == "Gamut Mapping" {
                            image.save(output.join(format!("mapping-cycle-{cycle}.png")))?;
                        }
                    }
                    Ok(())
                })??;
                steps.push(json!({"cycle": cycle, "page": page, "frames": 3}));
            }
        }
        Ok(json!({"case": "navigation", "outcome": "completed", "steps": steps}))
    })();
    let _ = cx.update_window(handle.into(), |_, window, _| window.remove_window());
    result
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    #[test]
    fn gamut_navigation_down_then_up_retains_the_shell_and_selected_route() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (view, cx) = app.add_window_view(XgpuiLabApp::new);
        cx.simulate_resize(size(px(1400.), px(2200.)));
        for _ in 0..3 {
            for &page in PAGES {
                let button = cx.update(|_, cx| {
                    view.read(cx)
                        .navigation
                        .iter()
                        .find(|entry| entry.workspace == Workspace::Gamut && entry.page == page)
                        .unwrap()
                        .button
                        .clone()
                });
                button.update(cx, |_, cx| cx.emit(gpui_luma::controls::button::ButtonEvent::Click));
                cx.run_until_parked();
                cx.update(|_, cx| {
                    let shell = view.read(cx);
                    assert_eq!((shell.active, shell.active_page), (Workspace::Gamut, page));
                });
            }
        }
    }
}
