use gpui::{AnyElement, div, prelude::*, px};

pub(crate) fn controls_mono_font() -> gpui::SharedString {
    #[cfg(target_os = "macos")]
    {
        "Menlo".into()
    }
    #[cfg(target_os = "windows")]
    {
        "Consolas".into()
    }
    #[cfg(target_os = "linux")]
    {
        "DejaVu Sans Mono".into()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "monospace".into()
    }
}

pub(super) fn render_composition_exposition(id: &'static str, preview: AnyElement) -> AnyElement {
    div()
        .id(id)
        .w_full()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .w_full()
                .min_w(px(0.0))
                .px(px(16.0))
                .pb(px(12.0))
                .flex()
                .flex_col()
                .items_stretch()
                .child(preview),
        )
        .into_any_element()
}
