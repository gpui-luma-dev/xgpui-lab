#![windows_subsystem = "windows"]

mod app;
mod validation;

mod app_shell;
mod compositions;
mod extension_pane;
mod gamut;
mod color_lab;
mod media_lab;
mod parley_lab;
mod svg_lab;
mod shader;
mod gradient_builder;
mod theme;
mod studio_tabs;
#[path = "assets/assets.rs"]
mod assets;

use assets::Assets;
use gpui::{actions, App, KeyBinding, Menu, MenuItem};

actions!(xgpui_lab_app, [Quit]);

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}

fn main() {
    // This path also verifies that a packaged Windows executable can load without
    // opening a GPU window or accessing a build machine's source directories.
    if std::env::args().any(|arg| arg == "--version") {
        println!(
            "XGPUI Lab {} ({}-{}, {})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::ARCH,
            std::env::consts::OS,
            if cfg!(feature = "portable-app") {
                "portable"
            } else {
                "source"
            }
        );
        return;
    }
    if let Some(backend) =
        std::env::args().find_map(|arg| arg.strip_prefix("--native-text-backend=").map(str::to_owned))
    {
        let frames = std::env::args()
            .find_map(|arg| arg.strip_prefix("--scroll-frames=").and_then(|value| value.parse::<usize>().ok()))
            .unwrap_or(0);
        if let Err(error) = parley_lab::native::run(&backend, frames) {
            eprintln!("Text diagnostic failed: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        cx.on_action(quit);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.set_menus([Menu::new("XGPUI Lab").items([MenuItem::action("Quit", Quit)])]);
        if let Err(error) = gpui_luma::init(cx).and_then(|_| {
            gpui_luma::focus::bind_default_focus_keys(cx);
            gpui_luma::key_handling::bind_default_control_keys(cx);
            app_shell::open(cx)
        }) {
            eprintln!("failed to open XGPUI Lab: {error:?}");
        }
    });
}
