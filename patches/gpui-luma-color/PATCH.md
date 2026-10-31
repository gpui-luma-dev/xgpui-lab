# Temporary field background-painter hook

Based on the published `gpui-luma-color` 0.2.5 crate. All other sources are unchanged.

Adds `ColorFieldBackgroundPainter`, a callback for painting a field background in
logical pixel bounds. `ColorFieldState::background_painter` and
`set_background_painter` retain SDK pointer/keyboard interactions, thumb, borders,
clipping, events and the selected built-in renderer. Custom paint errors use the
SDK vector background. With a custom painter installed, raster caches are neither
created nor refreshed; removing it restores the configured CPU renderer.

The app supplies the hue/value shader. The SDK patch adds no shader dependency.
This is a prototype API for eventual upstream integration; remove this local
crate patch after a published release provides the corresponding extension point.
