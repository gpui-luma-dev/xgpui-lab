use gpui::{AppContext, Hsla, Pixels};
use gpui_luma::theme::ThemeTokens;
use std::cell::RefCell;

pub use gpui_luma::infra::{ElementExt, StyledExt};

thread_local! {
    static ACTIVE_COLOR_CONTROL_THEME: RefCell<Option<ColorControlTheme>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug)]
pub struct ColorControlTheme {
    pub border: Hsla,
    pub background: Hsla,
    dark: bool,
}

impl ColorControlTheme {
    pub fn new(border: Hsla, background: Hsla, dark: bool) -> Self {
        Self { border, background, dark }
    }

    pub fn is_dark(&self) -> bool {
        self.dark
    }
}

fn default_color_control_theme() -> ColorControlTheme {
    let tokens = ThemeTokens::default();
    let background = tokens.palette.surface.panel.background;

    ColorControlTheme { border: tokens.palette.border.default, background, dark: background.l < 0.5 }
}

pub fn set_active_color_control_theme(theme: ColorControlTheme) {
    ACTIVE_COLOR_CONTROL_THEME.with(|cell| {
        *cell.borrow_mut() = Some(theme);
    });
}

pub fn active_color_control_theme() -> ColorControlTheme {
    ACTIVE_COLOR_CONTROL_THEME
        .with(|cell| cell.borrow().as_ref().copied())
        .unwrap_or_else(default_color_control_theme)
}

pub trait ActiveTheme {
    fn theme(&self) -> ColorControlTheme;
}

impl<T: AppContext> ActiveTheme for T {
    fn theme(&self) -> ColorControlTheme {
        active_color_control_theme()
    }
}

#[derive(Clone, Default, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    Size(Pixels),
    XSmall,
    Small,
    #[default]
    Medium,
    Large,
}

impl From<Pixels> for Size {
    fn from(value: Pixels) -> Self {
        Size::Size(value)
    }
}

pub trait Sizable: Sized {
    fn size(self, size: impl Into<Size>) -> Self;
}
