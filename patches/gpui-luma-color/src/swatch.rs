//! Reusable color swatch rendering.
//!
//! This control renders a color preview with alpha transparency support.
//! It uses a single canvas so the checkerboard, color fill, and border are
//! painted in one place, which avoids the corner bleeding and sub-pixel gaps
//! caused by layered rounded `div`s in GPUI.

use gpui::{prelude::*, *};

use crate::checkerboard_paint::{DEFAULT_CHECKERBOARD_SQUARE_SIZE, paint_masked_checkerboard};
use crate::chrome_tokens::swatch_checkerboard_colors;
use crate::style::ActiveTheme;
use gpui_luma::controls::button::{ButtonRenderModel, ButtonTemplate};
use gpui_luma::color::{ColorValue, GamutMapping, gpui_bridge};
use gpui_luma::theme::ControlSize;

/// Typed payload for a button-backed color swatch.
/// The source space/components are retained; rendering resolves a cached sRGB preview.
/// Invalid source values render transparently and can be diagnosed with `ColorValue::validate`.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorSwatchData {
    pub color: ColorValue,
    pub size: f32,
    pub radius: f32,
    pub open: bool,
    pub checkerboard: bool,
}

impl Default for ColorSwatchData {
    fn default() -> Self {
        Self {
            color: ColorValue::srgb(0.0, 0.0, 0.0, 1.0),
            size: 30.0,
            radius: 4.0,
            open: false,
            checkerboard: true,
        }
    }
}

/// Static button template for color swatches.
pub struct ColorSwatchButtonTemplate;

impl ButtonTemplate<ColorSwatchData> for ColorSwatchButtonTemplate {
    fn render(&self, model: &ButtonRenderModel<ColorSwatchData>, _window: &mut Window, cx: &mut App) -> Stateful<Div> {
        let size = px(model.data.size);
        let radius = px(model.data.radius);
        // Invalid source values have a transparent preview; the bridge exposes validation errors.
        let color =
            gpui_bridge::cached_rgba(model.data.color, GamutMapping::CssLocalMinde, cx).unwrap_or(rgba(0x00000000));
        let is_dark = cx.theme().is_dark();
        let border_color = cx.theme().border;
        let checkerboard = model.data.checkerboard;

        div().id(model.id.clone()).size(size).relative().cursor_pointer().child(
            canvas(
                move |_, _, _| (),
                move |bounds, _, window, _| {
                    let border_width = px(1.0);
                    let inner_bounds = bounds.inset(border_width);
                    let inner_radius = (radius - border_width).max(px(0.0));
                    if checkerboard && color.a < 0.999 && inner_bounds.size.width.as_f32() > 0.0 {
                        let (checker_light, checker_dark) = swatch_checkerboard_colors(is_dark);
                        window.paint_quad(PaintQuad {
                            bounds: inner_bounds,
                            corner_radii: Corners::all(inner_radius),
                            background: checker_light.into(),
                            border_widths: Edges::default(),
                            border_color: transparent_black(),
                            border_style: BorderStyle::default(),
                        });
                        paint_masked_checkerboard(
                            window,
                            inner_bounds,
                            inner_radius,
                            checker_dark,
                            false,
                            DEFAULT_CHECKERBOARD_SQUARE_SIZE,
                        );
                    }
                    window.paint_quad(PaintQuad {
                        bounds,
                        corner_radii: Corners::all(radius),
                        background: color.into(),
                        border_widths: Edges::all(border_width),
                        border_color,
                        border_style: BorderStyle::default(),
                    });
                },
            )
            .absolute()
            .size_full(),
        )
    }
}

/// A reusable color preview swatch.
///
/// The swatch is lookless: it resolves theme colors from the active GPUI
/// theme, but its rendering strategy is independent of any product-specific
/// palette or layout code.
///
/// Rendering model:
/// - The outer swatch is drawn in a single canvas pass.
/// - The color fill and border are painted together in one `PaintQuad` to avoid
///   haloing at the edge.
/// - Checkerboard is opt-in; when enabled and the color has alpha, it is drawn
///   inside an inset inner bounds so the border occludes any checkerboard edge
///   antialiasing.
#[derive(IntoElement)]
pub struct ColorSwatch {
    color: ColorValue,
    mapping: GamutMapping,
    size: ControlSize,
    custom_height: Option<Pixels>,
    corner_radius: Option<Pixels>,
    checkerboard: bool,
    bordered: bool,
}

impl ColorSwatch {
    /// Creates a swatch for the given color.
    ///
    /// The default sizing is `ControlSize::Md`, but callers can override the
    /// height or rounded radius after construction.
    /// Invalid source values render transparently; validate untrusted input before use.
    pub fn new(color: ColorValue) -> Self {
        Self {
            color,
            mapping: GamutMapping::CssLocalMinde,
            size: ControlSize::Md,
            custom_height: None,
            corner_radius: None,
            checkerboard: false,
            bordered: true,
        }
    }

    /// Select the preview mapping policy without changing the source color.
    pub fn mapping(mut self, mapping: GamutMapping) -> Self {
        self.mapping = mapping;
        self
    }

    /// Sets the semantic swatch size.
    ///
    /// This picks a default height and corner radius from the control-size
    /// scale when the caller has not provided explicit values.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Overrides the swatch height.
    ///
    /// This is used by gallery layouts that need a fixed preview strip while
    /// still keeping the control reusable elsewhere.
    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.custom_height = Some(height.into());
        self
    }

    /// Overrides the corner radius used for the outer swatch and the inner
    /// checkerboard mask.
    ///
    /// The same radius is intentionally reused across all painted layers so the
    /// swatch does not exhibit corner seams or background bleed.
    pub fn rounded(mut self, radius: impl Into<Pixels>) -> Self {
        self.corner_radius = Some(radius.into());
        self
    }

    /// Sets whether the checkerboard underlay is shown.
    ///
    /// Use `true` for translucent previews where the alpha indicator should be
    /// visible.
    pub fn checkerboard(mut self, enabled: bool) -> Self {
        self.checkerboard = enabled;
        self
    }

    /// Sets whether the theme border is painted around the swatch.
    pub fn bordered(mut self, enabled: bool) -> Self {
        self.bordered = enabled;
        self
    }

    fn height_for_size(size: ControlSize) -> Pixels {
        match size {
            ControlSize::Sm => px(28.0),
            ControlSize::Md => px(36.0),
            ControlSize::Lg => px(44.0),
        }
    }

    fn default_radius(size: ControlSize) -> Pixels {
        match size {
            ControlSize::Sm => px(6.0),
            ControlSize::Md => px(8.0),
            ControlSize::Lg => px(12.0),
        }
    }
}

impl RenderOnce for ColorSwatch {
    /// Renders the swatch as a single canvas-backed element.
    ///
    /// The paint order is deliberate:
    /// 1. Compute the outer radius and derive a 1px inset inner radius.
    /// 2. If checkerboard is enabled and the color is translucent, paint an
    ///    inner base fill and masked checkerboard inside the inset bounds.
    /// 3. Paint the actual color and border together in one `PaintQuad` so the
    ///    outer edge is composed once.
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let chrome = cx.theme();
        let is_dark = chrome.is_dark();
        let height = self.custom_height.unwrap_or_else(|| Self::height_for_size(self.size));
        let radius = self.corner_radius.unwrap_or_else(|| Self::default_radius(self.size));
        let color = gpui_bridge::cached_rgba(self.color, self.mapping, cx).unwrap_or(rgba(0x00000000));
        let border_width = if self.bordered { px(1.0) } else { px(0.0) };
        let corner_radii = Corners::all(radius);

        div().relative().w_full().h(height).child(
            canvas(
                move |_, _, _| (),
                move |bounds, _, window, _| {
                    let (c1, c2) = swatch_checkerboard_colors(is_dark);

                    let inner_bounds = bounds.inset(border_width);
                    let inner_r = (radius - border_width).max(px(0.0));

                    if self.checkerboard
                        && color.a < 0.999
                        && inner_bounds.size.width.as_f32() > 0.0
                        && inner_bounds.size.height.as_f32() > 0.0
                    {
                        window.paint_quad(PaintQuad {
                            bounds: inner_bounds,
                            corner_radii: Corners::all(inner_r),
                            background: c1.into(),
                            border_widths: Edges::default(),
                            border_color: transparent_black(),
                            border_style: BorderStyle::default(),
                        });

                        paint_masked_checkerboard(
                            window,
                            inner_bounds,
                            inner_r,
                            c2,
                            false,
                            DEFAULT_CHECKERBOARD_SQUARE_SIZE,
                        );
                    }

                    window.paint_quad(PaintQuad {
                        bounds,
                        corner_radii,
                        background: color.into(),
                        border_widths: Edges::all(border_width),
                        border_color: chrome.border,
                        border_style: BorderStyle::default(),
                    });
                },
            )
            .absolute()
            .size_full(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_luma::color::gpui_bridge::SrgbRenderCache;

    #[::core::prelude::v1::test]
    fn swatch_source_survives_clone_and_preview_resolution() {
        for source in [ColorValue::display_p3(1.0, 0.0, 0.0, 0.4), ColorValue::oklch(0.7, 0.3, 30.0, 0.8)] {
            let data = ColorSwatchData { color: source, ..Default::default() };
            let cloned = data.clone();
            let swatch = ColorSwatch::new(cloned.color);
            let preview = SrgbRenderCache::default().resolve(swatch.color, swatch.mapping).unwrap();
            assert_eq!(preview.a, source.alpha());
            assert_eq!(data.color, source);
            assert_eq!(cloned.color, source);
            assert_eq!(swatch.color, source);
        }
    }
}
