# gpui-luma-color

Source repository: [gpui-luma-dev/gpui-luma](https://github.com/gpui-luma-dev/gpui-luma).

Color system primitives and interactive controls for **GPUI-Luma** ([gpui-luma.dev](https://gpui-luma.dev)).

Provides domain renderers, checkerboard painting, palette calculations, and interactive color manipulation components built on the lookless Luma SDK.

## Components

* **`ColorField`**: 2D saturation/brightness or hue/saturation selection surface.
* **`ColorRing` & `ColorArc`**: Radial hue and chromaticity selectors.
* **`ColorSlider`**: 1D gradient sliders for alpha, hue, saturation, and lightness.
* **`ColorSwatch` & `ColorSwatchButtonTemplate`**: Swatches with checkerboard transparency backdrops and selected states.
* **`CheckerboardPaint`**: GPU-friendly alpha background rendering.

## Installation

```sh
cargo add gpui-luma
cargo add gpui-luma-color
cargo add gpui-unofficial --rename gpui
```

The package is named `gpui-luma-color`; Rust imports use `gpui_luma_color`.

## Usage

Create a swatch to include in a GPUI view:

```rust,no_run
use gpui::hsla;
use gpui_luma_color::ColorSwatch;

fn accent_swatch() -> ColorSwatch {
    ColorSwatch::new(hsla(0.6, 0.8, 0.5, 1.0))
}
```

## Build

In your application, run `cargo check` or `cargo build`.
From the Luma repository, run `cargo check -p gpui-luma-color`.

## License

Apache-2.0. See `LICENSE` in the package.
