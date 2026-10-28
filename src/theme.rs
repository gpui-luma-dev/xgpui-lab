use std::sync::Arc;

pub(crate) use gpui_luma_look_radix::Look;
use gpui_luma_look_radix::{Accent, Gray, SemanticRole};
use gpui_luma::theme::{LumaChrome, LumaTextStyle, LumaTypography, ThemeMode};
use gpui::{Styled, px};

/// Built-in Radix palette used at startup.
pub(crate) fn default_look() -> Arc<Look> {
    let look = Look::built_in();
    look.set_palettes(Accent::Indigo, Gray::Sand);
    look.set_mode(ThemeMode::Dark);
    Arc::new(look)
}

/// Typography and shell colors for this application, resolved from Radix roles.
pub(crate) trait ColorVizLookExt {
    fn chrome(&self) -> LumaChrome;
    fn typography_scale(&self, size: TextSize) -> LumaTextStyle;
    fn scrollbar_template(&self) -> Arc<dyn gpui_luma::controls::scrollbar::ScrollbarTemplate>;
}

#[derive(Clone, Copy)]
pub(crate) enum TextSize {
    Xs,
    Sm,
    Base,
}

impl ColorVizLookExt for Look {
    fn chrome(&self) -> LumaChrome {
        let color = |role| self.resolve_role(role).hsla();
        LumaChrome {
            app_background: color(SemanticRole::Background),
            content_background: color(SemanticRole::Background),
            panel_background: color(SemanticRole::Surface),
            border: color(SemanticRole::Border),
            title_text: color(SemanticRole::Foreground),
            body_text: color(SemanticRole::Foreground),
            muted_text: color(SemanticRole::MutedForeground),
        }
    }

    fn typography_scale(&self, size: TextSize) -> LumaTextStyle {
        let scale = LumaTypography::default().text.scale;
        match size {
            TextSize::Xs => scale.xs,
            TextSize::Sm => scale.sm,
            TextSize::Base => scale.md,
        }
    }

    fn scrollbar_template(&self) -> Arc<dyn gpui_luma::controls::scrollbar::ScrollbarTemplate> {
        Arc::new(gpui_luma::controls::scrollbar::ThemedScrollbarTemplate::new(Arc::new(ColorVizScrollbarTheme(
            self.clone(),
        ))))
    }
}

pub(crate) trait TypographyExt: Styled + Sized {
    fn typography_style(self, style: LumaTextStyle) -> Self {
        self.text_size(px(style.size)).line_height(px(style.line_height)).font_weight(style.weight)
    }
}
impl<T: Styled> TypographyExt for T {}

pub(crate) fn icon_button(
    id: impl Into<gpui::SharedString>,
    icon: lucide_svg_static::Icon,
) -> gpui_luma_look_radix::Button {
    gpui_luma_look_radix::Button::new(id)
        .role(gpui_luma::controls::button_family::ButtonFamilyRole::Icon)
        .icon(icon)
}

struct ColorVizScrollbarTheme(Look);
impl gpui_luma::controls::scrollbar::ScrollbarTheme for ColorVizScrollbarTheme {
    fn resolve(
        &self,
        state: gpui_luma::theme::InteractionState,
        orientation: gpui_luma::controls::scrollbar::ScrollbarOrientation,
        size: gpui_luma::theme::ControlSize,
        style: gpui_luma::controls::scrollbar::ScrollbarStyle,
    ) -> gpui_luma::controls::scrollbar::ScrollbarLook {
        use gpui_luma::controls::scrollbar::{DefaultScrollbarTheme};
        let mut resolved = DefaultScrollbarTheme::default().resolve(state, orientation, size, style);
        resolved.track_background = gpui::transparent_black();
        resolved.thumb_background = self
            .0
            .resolve_step(
                gpui_luma_look_radix::ScaleFamily::Gray,
                if state.pressed {
                    10
                } else if state.hovered {
                    9
                } else {
                    7
                },
            )
            .hsla();
        resolved
    }
    fn metrics(&self) -> gpui_luma::theme::MetricTokens {
        self.0.metrics()
    }
}
