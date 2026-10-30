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

Requires the sibling xgpui implementation through draft PR #94 (its stack includes Rec.2020 storage, extended native composition, resolved pixel metadata and the optional ICC adapter). No compatibility layer or old API aliases are added. The main app remains the default Cargo run target.

The app overlays sRGB, Display P3, Adobe RGB (1998), Rec.2020, D65 and a selected source point on CIE xy or u′v′. The spectral locus uses the checksum-verified CIE dataset in `assets/colorimetry`, with its attribution and license. Axes use equal physical units; ranges are stated below the diagram.

Click controls to cycle source primaries, transfer, RGB presets or individual channels, alpha, P3/Rec.2020 storage, and clipping/chroma reduction. Primaries and transfer are independent. Near-black and P3-red presets help expose quantization and containment assumptions. Changing storage explicitly remaps before association/mip generation.

The inspector reads the actual CPU-packed FP16 bytes and shows reconstructed XYZ error. It predicts extended linear P3 composition channels; it does **not** claim GPU readback or display measurement. Zero alpha has canonical zero RGB and undefined chromaticity. CIE coordinates are normalized before evaluation to avoid scaling overflow/underflow.

The two retained image sources show native-wide placement and clipped sRGB preview over white. The effect toggle applies two nested capture opacities (0.65 and 0.8), with radius-8 Gaussian blur on the inner capture. Panels reserve the core capture region’s worst-case four-level blur halo (16 logical + 8 physical pixels per side per level) inside a 1,000-physical-pixel budget, respecting the 1,024-pixel capture limit at Retina scale. Four capture groups total exercise the renderer's supported limit.

On macOS, native-wide presentation and xy/u′v′ switching were checked in a real Metal window. Windows/Linux runtime is unverified. The window reports negotiated output/display evidence; this does not establish monitor coverage or calibration. Existing Windows wide capture restrictions remain, so capture effects are disabled there. Screenshots are sRGB evidence of layout, not proof of wide-gamut monitor delivery.

This is a standalone lab binary because unrelated main-app callers currently require migration to the new library APIs. It does not depend on those callers compiling. The pre-existing local Cargo.lock edits are not part of this change.

For deterministic capture validation, run `cargo run --bin chromaticity-lab -- --effects`. Renderer errors are printed to stderr and recorded in the OS temporary directory as `xgpui-chromaticity-errors.log`.

## Wide gamut and color spaces

“Wide gamut” describes a display capability, not a specific RGB color space. P3 and Adobe RGB cover different regions; neither fully contains the other. The wide-gamut window uses extended linear P3 for composition on macOS, including negative and greater-than-one channels needed to carry colors outside bounded P3. Color management converts that result to the display profile; the panel still limits which colors can be reproduced. A display reporting wide-gamut capability does not establish its P3 or Adobe RGB coverage.

The **Wide-gamut window** control requests native wide presentation. **Simulated gamut** sets the RGB bounds used to map the diagram fill; it does not change the monitor profile or select a hardware display mode. **Presentation** chooses whether the fill takes the clipped sRGB preview route or native wide route.

## Chromaticity color fills

The diagram now fills the CIE locus at a selected relative input luminance **Y** (0.05, 0.18, 0.5 or 1). xyY / u′v′Y are inverted to XYZ, converted to the selected **Simulated gamut** (sRGB, P3, Adobe RGB or Rec.2020), then mapped with the shared clip/chroma-reduction control. Mapping can change luminance. Hatching is an annotation for original colors outside that selected RGB cube at this Y; hatch pixels are not color samples.

**Presentation** cycles off → clipped sRGB preview → native wide. The same retained image supports both routes. The sRGB route adds an explicit sRGB clip, even when Simulated gamut is wider. Native wide requires accepted window-wide mode; the app labels its sRGB fallback. P3/sRGB fills use P3 storage; Adobe RGB/Rec.2020 fills use Rec.2020 storage. That avoids forcing P3 boundary colors into bounded Rec.2020.

Fill generation runs in a cancellable background worker. Rasterization checks the CIE locus polygon, leaves the outside transparent, and maps only inside cells. It produces a 256 × 256 associated-linear mip chain through the media loader. Cached textures are reused when switching preview/native routes. Hidden/offscreen images do not keep the app animating for an idle upload.

The percentage is a count of sampled locus cells needing mapping in the current coordinate system and at the current Y. It is not gamut area/volume, perceptual coverage, or monitor coverage. xy and u′v′ percentages need not match because their sample grids differ.

Optional deterministic launch flags:

```sh
just chromaticity --native-fill
just chromaticity --native-fill --uv
```

Validation: eight numerical/data tests and strict standalone Clippy pass. New tests check inverse-coordinate D65 XYZ references in both coordinate systems at four luminances, sRGB-red bounds using independent primary/Y values, transparent locus masking, clip/chroma-reduction output bounds and cancellation. Native macOS fills were inspected in both sRGB-preview and native-wide routes; Windows/Linux runtime remains unverified.


## ICC metadata fixtures

The **Metadata** control cycles named → ICC parametric fixture → ICC sampled
fixture. The fixtures use Adobe RGB primaries with different serialized curves;
they are synthetic, not monitor profiles. Identical generated RGBA values can
therefore produce different decoded RGB, source XYZ/coordinates, and stored FP16.
The inspector and gold source marker use the resolved profile, including explicit
PCS-to-D65 adaptation. Reference gamut outlines and diagram fill controls remain
independent of patch metadata.

Source/Transfer controls explicitly return the patch to named metadata. While a
fixture is active they say ICC colorants/curves instead of displaying inactive
named settings. Probe gray and Probe blue presets plus the 0.0001 RGB step expose
near-black precision. Probe presets are intentionally dark; Mid gray gives a
visible patch comparison. Inspector values now show ten decimal places alongside
actual half bits; these are CPU-packed values, not monitor measurements.

From xgpui:

```sh
just profile=dev chromaticity --icc-parametric
just profile=dev chromaticity --icc-sampled --native-fill
```

The same flags work with `just chromaticity` in xgpui-lab. They select the fixture
and Probe gray preset; specifying both ICC flags is an error. Trusted embedded
fixtures are parsed/cached before the UI event loop. No file/image decoder was
added; the app supplies pixels + metadata through `load_pixel_buffer`.

Numerical tests check near-black channel references from exact serialized curve
coefficients/tables, zero-alpha canonicalization, invalid values and four
independent XYZ/associated-FP16 references across P3/Rec.2020 storage. These tests
exercise the new library adapter; they do not use its discarded transform engine.

Validation: all 10 standalone tests, strict Clippy and a dev build pass. The real macOS Metal window was checked for named → parametric → sampled metadata switching with identical Mid gray pixels, changed decoded values/source marker, returning to named metadata, scrolling and retained patch rendering. WideGamutSdr was active with no fallback. This is consumer/UI evidence, not a monitor calibration or gamut measurement. Windows/Linux runtime remains unverified.

The main view uses a 680 logical-pixel-tall chromaticity chart (twice the original height), with native-wide and clipped-sRGB samples stacked to its right. Decode tracking follows below the chart; scroll vertically to inspect it.


GPU validation: click **Check GPU (Metal FP16)** beside the samples while the
full native sample is visible. Wide-gamut mode and a completed upload are
required. This synchronously re-renders the current painted scene and checks
nine center pixels from the actual extended linear-P3 FP16 target. The result
shows measured RGBA, independent expected values and per-channel error budgets.
Control changes invalidate the prior result; each PASS is a snapshot.

The reference starts at the actual packed source FP16 bytes, independently
converts Rec.2020 to P3, and accounts for source-over white and the two optional
opacity effects. It isolates GPU conversion/composition from source/ICC
interpretation. Budgets scale with source energy and FP16 rounding stages,
include subnormal absolute error and f32 arithmetic, and require alpha exactly
one over the opaque white backdrop. A constant center checks blur preservation,
not blur edges or measured display output. Other backends return unsupported.

Validation: 12 standalone tests and strict Clippy pass. Native macOS UI checks
passed for Adobe RGB green, half alpha, both direct and nested opacity/blur.
The library additionally passes 13 native wide-color tests and 7 wide-capture
tests, including 96 FP16 color/alpha/capture/blur combinations. Source/ICC
precision remains covered by the existing independent ingestion references.
