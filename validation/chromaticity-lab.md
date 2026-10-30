# Chromaticity & Color Gamut Lab

Run independently of the main application. From the **xgpui repository root**:

```sh
just profile=dev chromaticity
just profile=dev chromaticity --effects
just chromaticity-test
```

The xgpui root defaults to release builds; `profile=dev` avoids that rebuild while iterating. These recipes delegate to the sibling xgpui-lab justfile, so Cargo runs in the correct repository.

From **xgpui-lab**:

```sh
just chromaticity
just chromaticity --effects
just profile=release chromaticity
just chromaticity-test
```

The standalone lab defaults to the dev profile here. Arguments after the recipe are forwarded to the app. Its launcher permits lockfile resolution while these unpublished sibling APIs are changing; existing main-app recipes retain their locked builds.

Direct Cargo equivalents:

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

## Chromaticity color fills

The diagram now fills the CIE locus at a selected relative input luminance **Y** (0.05, 0.18, 0.5 or 1). xyY / u′v′Y are inverted to XYZ, converted to the selected **Fill gamut** (sRGB, P3, Adobe RGB or Rec.2020), then mapped with the shared clip/chroma-reduction control. Mapping can change luminance. Hatching is an annotation for original colors outside that selected RGB cube at this Y; hatch pixels are not color samples.

**Fill** cycles off → clipped sRGB preview → native wide. The same retained image supports both routes. The sRGB route adds an explicit sRGB clip, even when Fill gamut is wider. Native wide requires accepted window-wide mode; the app labels its sRGB fallback. P3/sRGB fills use P3 storage; Adobe RGB/Rec.2020 fills use Rec.2020 storage. That avoids forcing P3 boundary colors into bounded Rec.2020.

Fill generation runs in a cancellable background worker. Rasterization checks the CIE locus polygon, leaves the outside transparent, and maps only inside cells. It produces a 256 × 256 associated-linear mip chain through the media loader. Cached textures are reused when switching preview/native routes. Hidden/offscreen images do not keep the app animating for an idle upload.

The percentage is a count of sampled locus cells needing mapping in the current coordinate system and at the current Y. It is not gamut area/volume, perceptual coverage, or monitor coverage. xy and u′v′ percentages need not match because their sample grids differ.

Optional deterministic launch flags:

```sh
just chromaticity --native-fill
just chromaticity --native-fill --uv
```

Validation: eight numerical/data tests and strict standalone Clippy pass. New tests check inverse-coordinate D65 XYZ references in both coordinate systems at four luminances, sRGB-red bounds using independent primary/Y values, transparent locus masking, clip/chroma-reduction output bounds and cancellation. Native macOS fills were inspected in both sRGB-preview and native-wide routes; Windows/Linux runtime remains unverified.
