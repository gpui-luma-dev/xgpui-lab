# Chromaticity & Color Gamut Lab

Run independently of the main application:

```sh
cargo run --offline --bin chromaticity-lab
cargo test --offline --bin chromaticity-lab
```

Requires the sibling xgpui implementation in PRs #88 and #89 (Rec.2020 image storage and extended native composition). No compatibility layer or old API aliases are added. The main app remains the default Cargo run target.

The app overlays sRGB, Display P3, Adobe RGB (1998), Rec.2020, D65 and a selected source point on CIE xy or u′v′. The spectral locus uses the checksum-verified CIE dataset in `assets/colorimetry`, with its attribution and license. Axes use equal physical units; ranges are stated below the diagram.

Click controls to cycle source primaries, transfer, RGB presets or individual channels, alpha, P3/Rec.2020 storage, and clipping/chroma reduction. Primaries and transfer are independent. Near-black and P3-red presets help expose quantization and containment assumptions. Changing storage explicitly remaps before association/mip generation.

The inspector reads the actual CPU-packed FP16 bytes and shows reconstructed XYZ error. It predicts extended linear P3 composition channels; it does **not** claim GPU readback or display measurement. Zero alpha has canonical zero RGB and undefined chromaticity. CIE coordinates are normalized before evaluation to avoid scaling overflow/underflow.

The two retained image sources show native-wide placement and clipped sRGB preview over white. The effect toggle applies two nested capture opacities (0.65 and 0.8), with radius-8 Gaussian blur on the inner capture. Panels reserve the core capture region’s worst-case four-level blur halo (16 logical + 8 physical pixels per side per level) inside a 1,000-physical-pixel budget, respecting the 1,024-pixel capture limit at Retina scale. Four capture groups total exercise the renderer's supported limit.

On macOS, native-wide presentation and xy/u′v′ switching were checked in a real Metal window. Windows/Linux runtime is unverified. The window reports negotiated output/display evidence; this does not establish monitor coverage or calibration. Existing Windows wide capture restrictions remain, so capture effects are disabled there. Screenshots are sRGB evidence of layout, not proof of wide-gamut monitor delivery.

This is a standalone lab binary because unrelated main-app callers currently require migration to the new library APIs. It does not depend on those callers compiling. The pre-existing local Cargo.lock edits are not part of this change.

For deterministic capture validation, run `cargo run --bin chromaticity-lab -- --effects`. Renderer errors are printed to stderr and recorded in the OS temporary directory as `xgpui-chromaticity-errors.log`.
