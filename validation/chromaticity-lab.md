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


Metadata now cycles named → synthetic parametric → synthetic sampled →
published Compact v4 sRGB → P3 → Adobe-compatible → sRGB2014 → named. Published files
are pinned, unmodified CC0 fixtures with hashes and license alongside them.
Their serialized curves/primaries are retained, including Adobe-compatible's
slope limiting and distinct encoded gamma. The three real-profile modes have
independent green/gray XYZ references, exact-f32 gray curve checks and alpha
checks. All 14 standalone tests, strict Clippy and dev build pass. The published selector modes were subsequently checked in the native Metal
window; the detailed results follow below. No general image-format decoding or arbitrary file import was
added. Published-profile rejection cases remain library tests, with details in
its real-icc fixture README.


The sampled sRGB2014 mode uses the unmodified ICC-authored fixture. Its encoded
PCS matrix is not forced onto a perfectly neutral ramp; the header PCS white
is used for one D65 adaptation. It retains the actual 1024-entry table rather
than using named sRGB. Independent green, exact-f32 gray and white XYZ checks
pass, as do gray curve and alpha checks. The source white intentionally differs
slightly from D65; the inspector may report explicit storage mapping at white.
Published-profile switching and GPU composition were checked in the native
Metal window, as recorded below.


Published-profile live validation (2026-10-30): application code `8f0d071`,
library `75afbe2`, macOS Metal, WideGamutSdr active, display wide capability
`Some(true)`, fallback `None`. All cases used Rec.2020 storage, Clip mapping,
source alpha 0.5 and the sample's opaque white backdrop. These percentages are
observed on this GPU, not cross-device limits; the defined budgets are unchanged.

| Metadata / patch | Captures | GPU result | Worst fraction of channel budget |
| --- | --- | --- | --- |
| Compact v4 sRGB / Green | Off | PASS, 9 center pixels | 32.4% |
| Compact v4 P3 / Green | Off | PASS, 9 center pixels | 20.9% |
| Compact v4 Adobe-compatible / Green | Off | PASS, 9 center pixels | 17.2% |
| sRGB2014 / Green | Off | PASS, 9 center pixels | 32.4% |
| sRGB2014 / Mid gray | Off | PASS, 9 center pixels | 55.3% |
| sRGB2014 / Mid gray | Nested opacity + blur | PASS, 9 center pixels | 0.1% |

Observed sRGB2014 Mid gray decoding was `[0.0272147727; 3]`, XYZ D65
`[0.0258687606, 0.0272139801, 0.0296451330]`, coordinates
`[0.3126970330, 0.3289578089]`, and stored half words
`[0x22f8, 0x22f7, 0x22f8, 0x3800]`. These match the independent references
at the inspector's displayed precision and retain the small neutral offset.
The direct readback was `[0.5131836; 3]` with alpha 1; nested readback was
`[0.7470703; 3]` with alpha 1. Source/transfer ICC labels, changing source
markers, control wrapping, sample retention while scrolling and returning from
sRGB2014 to named Adobe RGB/Mid gray were verified. No implementation change
was needed. This completes the earlier pending published-mode UI check; it
remains a composition validation, not a measurement of physical monitor output.

## Full-app integration — 2026-10-30

The main app now has a Color & Gamut rail entry (chart icon), in addition to the existing crate-specific color/gamut examples. It lazily creates and retains the shared chromaticity view. The standalone binary delegates to the same `src/chromaticity.rs` implementation, retaining its fixture flags, diagnostics and separate-window behavior. The embedded view follows the actual shared window's effective color mode rather than keeping a stale local wide-mode flag.

The view retains source/transfer and ICC selection, P3/Rec.2020 storage, mapping, alpha, xy/u′v′, fills, native/preview samples, nested effects, CPU FP16 inspection and the existing on-demand Metal readback. New rail chrome uses the app's existing SDK navigation controls. Source pixels, references and profile/data assets are unchanged.

Validation: both app binaries build with the aligned local xgpui/Luma dependencies; all 14 standalone color tests pass; strict standalone Clippy passes. The focused headless `app::tests::shell_rail_routes_to_retained_workspaces` test passes, including entry/re-entry and retained color-view identity. No GUI launch or new full-app live Metal acceptance is claimed. Restart the existing app and select the chart icon to try it. Existing standalone native GPU evidence remains separate.

## Hidden wide-gamut symbol example

Color & Gamut now offers a “Try the hidden wide-gamut symbol” view, shared by the standalone binary and app. It generates an original ring/X motif as supplied pixels: native linear P3 red background and sRGB red converted to P3 for the symbol. Selectable thumbnails show a clipped sRGB preview, native P3 presentation or a magenta out-of-sRGB mask. The mask classifies pre-storage linear source colors with a 1e-10 boundary tolerance; edge coverage is linear. Both source colors clip to sRGB red, while their native P3 coordinates differ. Wide-disabled/unsupported output explicitly falls back to the sRGB preview. No external image format/profile decoding or borrowed WebKit artwork is used.

Inspiration: https://webkit.org/blog-files/color-gamut/ and https://webkit.org/blog/6682/improving-color-on-the-web/. This is a similar demonstration, not byte-identical reproduction of its assets or exact color pair.

Validation: 15 color tests pass, including a new mathematical/actual-FP16 packing check for distinct native colors that collapse to the same sRGB preview, mask endpoints and motif geometry. Both binaries build, strict standalone Clippy passes, and the main app's headless navigation regression passes. No live full-app display observation or physical gamut measurement is claimed for this new example; restart and select the example on a native wide-gamut display for visual acceptance.

## Section tabs and explicit window modes

The app's Color & Gamut workspace now uses the existing SDK Tabs control and StudioTabs template, with “Chromaticity & Metadata” and “Hidden Wide-Gamut Symbol” tabs. The app hides the standalone example-switch/back buttons. One retained color view preserves each example's settings and lazily prepares the symbol images. The standalone binary retains its own navigation buttons. Both examples expose explicit Standard (sRGB) and Wide gamut window mode choices; selection follows the requested mode, while effective output status remains separate.

Checks: both entry points compile, 16 color tests pass (including example-state retention), and the full app headless navigation-retention test passes. No new GUI launch or visual acceptance is claimed.

## Licensed photographic comparison

The app adds a third SDK section tab, Photo Comparison. It embeds Laitche's current original Adobe RGB kingfisher JPEG with SHA-256 `7766d228c6c61a16eab2871b5772738a2ce3ab6b74e1195b4f2095a22aca8fcc`. Provenance, CC BY-SA 4.0 terms and adaptation notes are in `assets/colorimetry/photos/README.md`, NOTICE and clickable in-app credits. Original bytes and embedded ICC are retained.

A cancellable background worker decodes the fixed JPEG using the lab's existing image dependency, parses its 560-byte ICC profile using xgpui-color/icc, and samples a bounded 1200-pixel-wide derivative. The source pixels plus custom RGB definition go to the normal media loader with explicit Rec.2020 storage. sRGB preview and native wide placement share that source texture. A separate mask uses source XYZ→linear-sRGB excursions (1e-7 tolerance), magenta outside sRGB and luminance grayscale inside. No profile reassignment, external sRGB comparison image, new core format loader or network runtime dependency is added. Selection and the existing Standard/Wide window controls are independent. The standalone binary can open this view with `--photo`.

Validation: 17 shared color tests pass, including exact original checksum, successful embedded profile parsing, sampled real gamut excursions and cancellation rejection. Strict standalone Clippy and the main app headless navigation-retention regression pass. Both binaries build. No full-app live photographic display observation is claimed; visual/native GPU acceptance remains separate from decoding and compilation.

### Photo overlay correction

The photographic Out-of-sRGB areas view now retains the color sRGB preview and replaces only source pixels outside sRGB with cyan. It no longer desaturates in-gamut pixels. Classification remains based on the original decoded source XYZ, before clipping; highlight colors are supplied as linear-sRGB metadata. The hidden-symbol example's separate diagnostic mask is unchanged.

### Photo native/preview investigation

A new macOS headless Metal regression paints the same ICC-interpreted, Rec.2020-stored plumage sample through the sRGB-preview and native-wide routes in one scene. It waits for upload and measures a fresh FP16 composition frame, rejecting placeholder values. Observed linear-P3 values: preview `[0.1376953125, 0.75, 0.96630859375, 1]`, native `[0.005950927734375, 0.7431640625, 0.97216796875, 1]`; maximum channel difference `0.131744384765625`. This establishes different compositor output for that photo sample, not perceptual distinguishability on the user's monitor or a resolution of the reported live visual issue. No OS display mapping or physical output is measured.

## WebKit Italy replacement

Replaced the kingfisher fixture with the exact Italy-sRGB.jpg, Italy-P3.jpg and Italy-oog.jpg files from https://webkit.org/blog-files/color-gamut/. Photo Comparison credits and links WebKit. All three original JPEGs and SHA-256 hashes are retained; macOS ColorSync prepares opaque linear Display P3 RGBA16F buffers offline. Runtime consumes explicit pixels and metadata, including WebKit’s supplied highlighted photo. Compare orange/red umbrellas side by side in Wide gamut. Standard clips P3 output; these separately authored source versions are not guaranteed identical in Standard. The Metal test checks an orange umbrella pixel from the actual two source buffers after upload.

### Standard presentation color matching

The Metal Default path previously left CAMetalLayer.colorspace unspecified, including when switching back from Wide. Default now explicitly tags its bounded encoded BGRA8 output as sRGB both at initial layer configuration and after switching. Wide remains RGBA16F extended linear Display P3. A platform regression verifies actual layer colorspace equality, format and EDR state at startup and across repeated mode switches. The photo’s supplied highlighted JPEG contains diagnostic markings; their continued visibility in Standard does not indicate gamut leakage.
