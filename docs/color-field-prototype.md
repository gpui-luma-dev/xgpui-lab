# Color field prototype

This implementation lives entirely in xgpui-lab. It is a candidate for promotion
into xgpui-shader after visual review. It does not modify `Brush::freeform` or its
inverse-distance semantics. RBF fitting and diffusion curves are separate future
research options, not part of this prototype.

## Surface

`gradient_builder::color_field` owns the renderer-independent `ColorField`,
`FieldPoint`, `Mixing`, and `Output` types. The builder adapter owns selection,
controls, and conversion from the existing editor state. `ColorField::sample`
is the CPU reference. `Paint` is an immutable native GPU producer; snapshots share
`PipelineCache` but retain independent output textures.

The Freeform tab has separate CPU / GPU field and Ordered / Normalized choices.
Both renderers consume the same field controls. CPU + Ordered remains the default.
The existing P3 switch selects native P3 output where the display/window support
it. GPU field is currently implemented on Metal; unsupported backends must select
CPU. No silent substitution of another field algorithm occurs.

## Coordinates and shape

- Up to 16 independently positioned points; coincident points are valid. The core
  also permits an empty field, which yields its background.
- Positions and radius use normalized full-receiver coordinates, independent of
  clipping. This preserves the current editor: circles in this coordinate system
  appear elliptical on nonsquare receivers.
- Angle is clockwise degrees from positive X. Stretch is an area-preserving axis
  ratio in 1/9..=9. A value of 1 preserves the original circular field.
- The local squared distance is `x*x/stretch + y*y*stretch` after rotating into the
  field's axes. Weight is `exp(-pow(distance*5/(radius*radius), 1/softness))`.
- Editor radius is global spread times per-point spread. Softness 1 reproduces the
  original Gaussian falloff; varying softness preserves the e^-1 contour.

## Mixing and alpha

**Ordered** preserves the existing editor exactly: each field lerps straight RGB
(or background-anchored HSL) toward its color by `weight * point_alpha`, and
updates alpha using source-over accumulation. Order matters. This historical
straight-color operation is not conventional premultiplied source-over when the
background is translucent. It is retained deliberately for comparison; decide
whether to expose it as a compatibility mode before stabilizing an SDK API.

**Normalized** is a weighted mixture, independent of point order except floating
point rounding. A field contributes geometric weight `w`, associated color
`w*a*color`, and associated weight `w*a`. The background contributes the same
terms using the explicit Base weight (default 0.25). Straight RGB is the color
sum divided by associated weight; alpha is associated weight divided by total
geometric weight. Fully transparent contributors do not tint visible colors.
If all associated weights vanish, output is transparent black. If total weight
vanishes, output falls back to the background. This is mixing, not layer stacking.

HSL uses hues lifted once relative to the background using the editor's current
hue policy. This avoids position-dependent shortest-arc branches. Normalized HSL
averages those lifted components with associated weights. Neither mode claims to
match Illustrator or an adopted Freeform interchange standard.

## Color and resources

Input channels are encoded sRGB or encoded Display P3 according to the explicit
output choice. Switching to P3 reinterprets values rather than preserving their
sRGB appearance. Mixing stays in that encoded RGB space or its HSL coordinates.

The GPU writes straight-alpha BGRA8 encoded sRGB, or premultiplied RGBA16Float
linear Display P3 directly. P3 uses the shared sRGB/P3 transfer function after
mixing. The host handles final presentation. Display capability negotiation
belongs to the application, not this primitive. CPU P3 retains its previous image
conversion path. PNG export retains its existing sRGB behavior.

Point records are uploaded as data, not shader-source constants. Editing position,
color, spread, softness, rotation, stretch, mixing, or output does not recompile
the pipeline. The prototype uses the public native-content API to avoid forcing
16 points into the 128-byte main-pass shader payload. Preparation records commands
on the host queue and never submits or waits. Output is cached by queue and extent,
with a 64 MiB output limit. GPU mode resets/disables the existing CPU-only
Saturation/Vibrance controls.

## Validation and promotion

Headless checks cover legacy CPU parity, normalized ordering, transparent colors,
validation, and control-state preservation. The opt-in native test reads back GPU
pixels for both algorithms, RGB/HSL, sRGB/P3, and 16 mutable points; it also checks
that all configurations share one compiled pipeline:

```sh
cargo test --features test-support,gpui_platform/runtime_shaders gradient_builder::
cargo test --features test-support,gpui_platform/runtime_shaders native_field_pixels -- --ignored --nocapture
```

Before promotion: visually approve both mixing modes; settle the ordered alpha
contract and public naming; decide whether normalized coordinates or isotropic
physical coordinates should be the SDK default; implement/validate the additional
SDK backends; and decide whether a main-pass path is needed alongside prepared
textures. The current data model and CPU oracle can move independently of editor UI.
