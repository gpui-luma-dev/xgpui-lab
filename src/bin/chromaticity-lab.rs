//! Standalone entry point; the full app uses the same color workspace.
#[path = "../chromaticity.rs"]
mod chromaticity;
use chromaticity::Lab;
use gpui::*;
struct Diagnostics;
impl log::Log for Diagnostics {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Error
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            use std::io::Write;
            let message = format!("{}: {}\n", r.target(), r.args());
            eprint!("{message}");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(std::env::temp_dir().join("xgpui-chromaticity-errors.log"))
            {
                let _ = f.write_all(message.as_bytes());
            }
        }
    }
    fn flush(&self) {}
}
static DIAGNOSTICS: Diagnostics = Diagnostics;
fn main() {
    let _ = log::set_logger(&DIAGNOSTICS);
    log::set_max_level(log::LevelFilter::Error);
    if std::env::args().any(|a| a == "--icc-parametric") && std::env::args().any(|a| a == "--icc-sampled") {
        eprintln!("Choose one ICC fixture flag");
        std::process::exit(2);
    }
    gpui_platform::application().run(|cx| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1220.), px(880.)), cx))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Chromaticity & Color Gamut Lab");
                cx.new(|_| Lab::new(window, true))
            },
        )
        .expect("open chromaticity lab");
        cx.activate(true);
    });
}
