# XGPUI Lab

Initial working version of a unified native benchmark, testing, and validation app for xgpui extensions. The shell and interactive controls use GPUI Luma. The original Gradients and Compositions workspaces from gpui-luma-color-viz remain separate from the extension labs.

## Current progress

| Workspace | Implemented coverage |
| --- | --- |
| Color | Color spaces, conversions, interpolation, gamut mapping, CPU gradients |
| Gamut | P3 negotiation with sRGB fallback, wide fills/gradients, native versus CPU mapping, presentation routes, built-in capture benchmarks |
| Media | Image viewing, atlas/mipmap comparison, filters, linear adjustments, float/P3, tiled canvas, video frames, resource limits, embedded upload/cadence/scroll/capture benchmarks |
| Parley | Shaping, distinct line layout and editable paragraphs, fallback/fonts, rasterization, scrolling, separate native/Parley glyph-atlas diagnostics |
| Shader | Analytic gradients, direct/legacy Hermite editing, gradient coverage, WGSL, instancing, typography selection/effects, brushes, layer effects, shadows, borders |
| SVG | Distinct rendering/scaling views, camera gestures, path effects, scheduling/cache, benchmark views |

Every extension lab has an embedded **Benchmarks** tab. Workloads use code compiled into this application and save reports/captures to a retained temporary folder, accessible with **Reveal results**. No benchmark builds another application or requires Cargo, Python, sibling source trees, or cached build dependencies at runtime. Parley backend isolation, SVG corpus isolation, and lifecycle stress launch fresh copies of this executable. External SVG artwork is selected by the user and is not downloaded automatically.

Gamut → **Benchmarks** runs the embedded Gamut renderer and Luma controls directly in this application. Wide texture, instanced, gradient, SDR baseline, and current-control workloads draw five frames in a temporary hidden window. Results include per-frame CPU draw and render/readback timings, negotiated presentation status, and clipped sRGB PNG previews. On macOS, each workload also checks 12 small Metal compositing scenes against an independent linear-light CPU reference: black/white backdrops, source alpha, inherited opacity, reversed overlap, clipping, native overlays, replay, and viewport resize. Pixel failures fail the workload; diagnostic captures and check results are saved with its report. Other platforms explicitly report these Apple checks as unavailable. GPU execution time and physical display fidelity are not measured. **Reveal results** opens the retained temporary results folder. These workloads are available in portable builds and do not invoke Cargo or external source workers.

For an explicit native smoke run using an already-built app:

```sh
./target/debug/xgpui-lab --gamut-benchmark all --output artifacts/gamut-benchmarks
```

Media → **Benchmarks** embeds upload timing, visible frame cadence, sustained scrolling, atlas/mipmap capture, and native video pixel checks. Upload/capture workloads run on background threads; cadence and scrolling use temporary windows in this application. The scroll workload measures 5 seconds of baseline, 30 seconds of repeated decode/mip/upload replacement, and 5 seconds of cached scrolling. Keep measurement windows visible and foreground. Debug builds support functional runs and identify their profile; provisional release performance criteria are never reported as passed in debug builds. The atlas workload also asserts odd-width monochrome/BGRA upload pixels, cache hits, replay, eviction, stale-scene replay, and reinsertion with changed data. On macOS, the video workload additionally tests native `PaintSurface` BGRA and full-range NV12 pixels, clipping, resize, replacement, and replay, separately from custom-draw video. Native Surface uses its existing full-range conversion; it does not support the custom-video path’s complete range/matrix selection. CoreVideo validation is available on macOS and explicitly unavailable on other platforms. **Cancel** stops a run and **Reveal results** opens its retained temporary reports and captures. Media benchmarks do not invoke Cargo; lifecycle stress alone launches an isolated copy of this application so backend crashes can be reported without closing the lab UI.

```sh
./target/debug/xgpui-lab --media-benchmark all --output artifacts/media-benchmarks
```

Parley → **Benchmarks** compares native and Parley line shaping, cache hits, glyph rasterization, paragraph support, and a 5,000-line CPU corpus. It also runs each real text backend through 600 virtualized-scroll frames with four font styles and captures the Parley glyph atlas. Backends use fresh application processes with the same embedded sample/font corpus. The native paragraph capability is optional and is explicitly reported as unavailable where unsupported; line and glyph measurements remain comparable.

Shader → **Benchmarks** embeds the original coverage, alpha/opacity, clip mapping, replay, resize, gradient geometry, source reuse, and portable WGSL checks. Pixel assertions run against the native renderer before five measured capture frames. Render/readback timings include GPU waits and are not GPU-only timings.

SVG → **Benchmarks** measures the built-in scene, a chosen SVG, or a hash-verified corpus. Selected documents use five repeated parse/raster samples after first use at 4× zoom, density 2 and a 16 MiB output budget. Corpus mode defaults to four repository-local fixtures (16 cases), verifies every file before starting, and runs one fresh copy of this executable per document/zoom (1×, 4×, 8×, 16×; density 2; 256 MiB output budget). Raw case results, failures and previews are retained. Reported process RSS includes the whole process and is not a document allocation or GPU-memory measurement.

```sh
./target/debug/xgpui-lab --parley-benchmark all --output artifacts/parley-benchmarks
./target/debug/xgpui-lab --shader-benchmark all --output artifacts/shader-benchmarks
./target/debug/xgpui-lab --svg-benchmark all --input /path/to/artwork.svg --output artifacts/svg-benchmarks
```

Captured effects currently require SDR in xgpui. The app temporarily switches presentation for affected views and restores the previous mode when leaving them. Upstream tracking: [xgpui#53](https://github.com/gpui-luma-dev/xgpui/issues/53).

## Local checkout setup

This snapshot intentionally uses sibling source checkouts, including Luma. It is not a standalone crates.io-based build:

```text
parent/
  xgpui-lab/
  xgpui/
  gpui-luma/
```

Verified source references:

- xgpui: `9e6a732110a1fb1cd6b7ff12bf306d95ceb1fb12` on `codex/objc2-metal-interfaces`.
- gpui-luma: `0374b85bac8c074e1d01ea983db1a9156a9cf9a2` on `main`.

Cargo path dependencies and patches select the sibling xgpui framework and backend crates. Supporting crates resolve from the matching published `gpui-unofficial` release. Sample fonts, shader fixtures and the default SVG corpus live in `assets/samples/` and are embedded. Keep the dependency repositories present when compiling. Built applications do not read them at runtime. Branch names are informational; Cargo does not enforce the checked-out branch or revision.

The Rust stable toolchain, rustfmt, and Clippy are selected by `rust-toolchain.toml`. Native verification has been performed on macOS; other platform dependencies are present but cross-platform support is not yet verified. The macOS renderer also needs the usual Xcode/Metal build tools.

```sh
cargo run --locked --release
```

Use these application-scoped development checks:

```sh
cargo fmt --package xgpui-lab -- --check
cargo test --locked --all-targets --features test-support
cargo clippy --locked --all-targets --features test-support -- -D warnings
```

Add `--offline` to Cargo build/test/check commands when dependencies are cached. The `justfile` exposes the same application-scoped build/run/check recipes.

Run `just regression-tests` for native atlas pixel checks, Gamut compositing assertions, the video workload's native Surface checks, and lifecycle stress. Each run builds the debug executable and saves JSON reports, PNG captures, and logs in a unique timestamped directory under `artifacts/regression-tests/`. All four workloads run even if one fails, and any failure produces a nonzero exit status. The Apple Gamut, Surface, and lifecycle checks require macOS; other platforms report them as unavailable. These runs validate rendering correctness, not release performance.

Lifecycle stress first runs deterministic ownership checks: 17 native resource variants each exercise abandonment of an unsubmitted sampling command, gated submission/completion after renderer destruction, and rejection by a later CPU preparation step (51 frame cases). Exact completion pixels and weak/native-owner assertions check resource survival and release. Three mixed native windows close before painting, after completion, and with GPU work still pending; these include native text, shapes, atlas/dedicated images, paints, WGSL/instanced shaders, wide color, all custom video variants, and BGRA/NV12 Surfaces. Reports and captures are saved in `worker/lifecycle/ownership/`. Bounded queue gates release on errors and have a 10-second watchdog. Native window submission uses a test-support hook on the actual window renderer with an offscreen target; drawable presentation is not asserted. Producer preparation/uploads may already have submitted work when host sampling is abandoned. CPU rejection does not simulate a GPU command error or native import/allocation failure. These probes need the sibling xgpui test-support hooks described in the coverage matrix.

Lifecycle stress is also available in Media → Benchmarks or with `--media-benchmark lifecycle --output <directory>`. By default it creates/destroys 128 headless renderers and 64 hidden native windows while reusing one shader producer across their queues. It performs 6,144 replacements each of atlas images, CoreVideo frames, and native Surfaces, checking pixels, replay, cache reuse, atlas retirement after nonblocking GPU submission, and return to the original window/producer reference counts. It saves first/last captures, raw process RSS samples in `memory-samples.json` and `memory.csv`, a per-run `memory.svg` trend plot, and the final report. Lifecycle stress runs in an isolated copy of the application; worker errors, crashes, and timeouts fail the workload. Worker logs, partial memory samples, and the last `checkpoint.json` remain under `worker/lifecycle/` (logs are in `worker/`).

Configure longer runs with `XGPUI_LIFECYCLE_CYCLES` (8–1,000) and `XGPUI_LIFECYCLE_REPLACEMENTS` (4–1,000 per cycle); execution is bounded to five minutes and supports Cancel. The memory gate compares median RSS in the first/last thirds of completed cycles after discarding the first quarter for warmup. It fails on growth above `XGPUI_LIFECYCLE_MAX_RSS_GROWTH_MIB` (default 64 MiB). This is a configurable smoke threshold, not a measured leak baseline. RSS includes process/runtime/driver allocations and does not measure total GPU memory; shader-cache bytes are reported separately and checked against the producer's 64 MiB budget.

## Continuous integration

Automatic `CI` runs are paused while xgpui publication/distribution and the GPUI dependency source are being settled. The workflow retains manual dispatch for explicit verification once enabled; PR and push triggers are removed.
Its stable check names are **Native (macOS)** and **Native (Windows x64)**.
Each job checks formatting, compiles all app targets with test support, runs
Clippy with warnings denied, and executes the headless test suite. Linux CI remains follow-up work; distributable Windows archives use the
separate manual workflow documented below.

CI checks out three sibling folders under the runner workspace: `app/`
(this repository), `xgpui/`, and `gpui-luma/`. Exact dependency commits live in
`.github/dependencies.json`. No crates.io publication of xgpui is required.
Luma 0.2.4 uses xgpui 1.0.0 directly. Cargo patches keep Luma and the extension labs on the same sibling xgpui framework and backends; no compatibility patch is needed.

The reusable `.github/actions/prepare-dependencies` action expects the app at
`app/` and is also used by the manual Windows app builder.
Python 3.13 and Git must be installed before calling it. Preparation verifies
the exact SHAs and refuses dirty sibling trees; run it only in disposable
checkouts, not over a working development environment. Pin updates must include
any necessary bridge/lockfile changes and pass both native jobs.

Hosted jobs install Rust stable, rustfmt, and Clippy. macOS requires Xcode and
the Metal compiler (downloaded if absent); Windows requires MSVC and the Windows
SDK resource compiler supplied by the runner image. Cargo fetches dependencies
on a cold runner rather than assuming an offline developer cache. Native caches
are separated by target, Rust compiler identity, dependency pins/patch, lockfile,
and toolchain selection. Superseded runs are cancelled; jobs time out after one hour.

CI uploads command logs, exit statuses, durations, and resolved source/compiler
versions as seven-day workflow artifacts, including completed diagnostics on
failed runs. It does not open native windows, run the full interactive benchmark
suite, establish GPU/physical-P3 parity, or package/release the app. The current
generated video source is macOS-only and reports that limitation on other platforms.
Windows native rendering still needs a separate interactive smoke test; the
manual builder checks archive packaging and OS-loader startup.

To reproduce the workflow commands from a prepared app checkout:

```sh
python -m unittest discover -s .github/scripts -p 'test_*.py' -v
python .github/scripts/check.py format
python .github/scripts/check.py compile
python .github/scripts/check.py clippy
python .github/scripts/check.py test
python .github/scripts/check.py portable
```

These commands save logs in `artifacts/ci/` and return the underlying Cargo
exit status. Native benchmark smoke runs are explicit developer commands; headless unit tests do not launch GPU windows.

## Manual Windows app builds

The **Build XGPUI Lab Windows Apps** workflow (`.github/workflows/build.yml`)
is manually triggered only. In GitHub **Actions**, select this workflow, choose
**Run workflow**, select the desired branch/ref, and run it. It builds native
Windows x64 and ARM64 release executables in independent jobs, using the same
pinned sibling source setup as CI. It does not publish a GitHub Release, run
automatically for PRs/pushes, or build macOS/Linux app archives.

Download the matching `xgpui-lab-windows-x64-<commit>` or
`xgpui-lab-windows-arm64-<commit>` Actions artifact. The download contains the app
ZIP, a SHA-256 sidecar, build metadata, and an extracted-executable smoke report.
Extract the app ZIP before launching `xgpui-lab.exe`. Artifacts expire after
seven days. Archives are unsigned portable previews, not installers.

Each archive includes the executable, launch instructions, app/dependency
revision and toolchain information, the DLL import report, executable checksum,
and source/font license notices. Release builds use static CRT linkage; packaging
rejects unexpected VC runtime DLL imports or unresolved non-system imports
instead of silently producing an incomplete archive. Windows system libraries
are not redistributed. The release shader compiler comes from the runner's
Windows SDK, with an explicit path for native ARM64 builds.

The builder verifies PE architecture and runs the extracted app's `--version`
entry point in a temporary directory outside the checkout. That proves OS-loader
startup and selected architecture; it does **not** verify a visible window,
GPU rendering, physical P3 output, or input behavior. A Windows desktop with
supported Direct3D graphics/current drivers still needs a manual GUI smoke test.
ARM64 artifacts are built and loader-tested on the native Windows ARM64 runner,
not inferred from an x64 cross-compilation.

Archives use the `portable-app` feature to identify distributable builds. All embedded benchmark tabs remain available; Rust/Cargo and source checkouts are not needed at runtime. The default SVG corpus and font/shader samples are embedded. CoreVideo checks require macOS Metal; other platforms report that capability as unavailable. Windows P3 behavior depends on backend/display negotiation and is recorded in benchmark reports.

CI tests ordinary and portable configurations. Signing, installers, automatic updates, and macOS/Linux packaging remain separate future work.

## Source attribution

The adapted application and lab source is covered by `LICENSE-APACHE` and documented in `NOTICE`, except for separately licensed samples. Seascape's original shader and WGSL adaptation retain CC BY-NC-SA 3.0; see `licenses/Seascape-CC-BY-NC-SA-3.0.txt`. Font licenses are in `licenses/fonts/`. The repository's original MIT license is retained in `LICENSE`.

Benchmark progress and timings appear in the application and retained JSON reports, without stdout logging. `--version` remains a command-line metadata response. Optional file pickers load user artwork; default samples require no external folder.

## objc2 / Metal migration coverage

The reviewed [coverage matrix](validation/objc2-coverage.md) maps rendering paths to ownership transitions, destruction, reuse, isolation, and concrete native test evidence. Its [JSON source](validation/objc2-coverage.json) explicitly separates verified assertions, partial coverage, gaps, and non-applicable cases. `just coverage-report` validates evidence references and regenerates the tables; `python3 .github/scripts/render_objc2_coverage.py --check` also detects a changed xgpui revision. These checks validate the inventory, not native test execution.

`just consumer-audit` builds published xgpui/platform 1.0.0 and all six extensions 0.2.0 in an isolated temporary consumer, with a fresh target and no local patches or Cargo configuration. Registry caches may be reused. Reports, exact lockfile, dependency graph, legacy dependency chains, and compiler logs are retained under `artifacts/consumer-audit/`. This proves published-consumer buildability; it does not validate unpublished migration changes or the entire Luma/lab stack. The matrix lists candidate-release verification and legacy dependency removal as separate gates.

## Shader API naming migration

Requires the sibling xgpui checkout with
[the shader naming change](https://github.com/gpui-luma-dev/xgpui/pull/84) applied. Shader
paints use `xgpui_shader::shader_paint::ShaderProgram::new`; the removed
`experimental` imports and `new_v1` constructor are no longer used.
`instanced::ShaderProgram<T>::new` is unchanged. The rendering and parameter
contracts remain the same, including shader-paint loop restrictions.

Grid and Seascape benchmark reports now identify APIs as `shader_paint` and
`instanced`, and capture filenames use `shader-paint` and `instanced` in place
of the previous version labels. Update scripts that consume these output names.

## Shader Depot: Grid (app-only API validation)

In the Shader workspace, select **Shader Depot: Grid** for side-by-side
shader paint and instanced shader previews. Programs compile once;
sliders update the four scalar fields. The typed instance is 16 bytes with
field offsets 0, 4, 8, 12. Pulse width/speed use the upstream cosine interpolation;
turn pulse off to inspect the base line width. Switching away suspends animation.
The pane includes attribution and a source link to Christian Helgeson's
[WGSL Shader Depot](https://github.com/cmhhelgeson/WGSL_Shader_Depot).
BSD-3-Clause terms are retained in `licenses/ShaderDepot-BSD-3-Clause.txt`.

All sample code, controls, tests and reports belong to this app; this sample
requires no changes to the sibling `xgpui` repository. `src/shader/grid.wgsl`
contains the shared grid calculation, with host-owned wrappers constructed
in `src/shader/grid.rs`. The original unused resources and debug variables
are omitted. Rust uses `ShaderParameters::float` for shader paints and `ShaderInstance`
field descriptions for instanced shaders.

```sh
cargo test shader::grid::tests
cargo test --features test-support shader_tabs_render_and_luma_controls_edit_retained_lab_state
cargo run -- --shader-benchmark grid --output artifacts/grid-native
# Optional native/wgpu pixel comparison using the same public program objects:
cargo run --features grid-wgpu -- --shader-benchmark grid --output artifacts/grid-parity
```

The Grid benchmark checks both APIs against a CPU reference over 176×112
captures (160×96 content), including asymmetric origins, parameter extremes,
clipping and opacity. It compares all RGBA pixels within 3/255 and writes PNGs
and JSON under the output's `grid/` directory. `grid-wgpu` additionally compares
the native backend with an explicit wgpu headless renderer. On macOS this
exercises native Metal and wgpu's Metal backend, not a separate Vulkan device.

The public constructors invoke the framework's existing validation and
translation paths. Success does not independently certify Windows driver
compilation, browser/WebGL2 rendering, or warning-free output on every target.
The 120-frame benchmark measures scene creation/render/wait/readback wall time;
it does not establish interactive 120 FPS. Test the pane on a 120 Hz display
with a frame profiler for that acceptance criterion. Framework web demos and
framework documentation are deliberately outside this app-only change.

## Shadertoy: Seascape (app-only stress sample)

Select **Shader → Shadertoy: Seascape** for the WGSL adaptation of
[Seascape by Alexander Alekseev / TDM](https://www.shadertoy.com/view/Ms2SD1).
The author publishes the GLSL in
[tdmaav/shadertoy](https://github.com/tdmaav/shadertoy/blob/master/Seascape.shader);
the source blob and modifications are recorded in the WGSL and `NOTICE`.
The shader and its adaptation retain **CC BY-NC-SA 3.0** (noncommercial,
share alike), separately from the application's Apache license. See
`licenses/Seascape-CC-BY-NC-SA-3.0.txt` for provenance and license links.

This sample exercises a 32-step height-field tracer, procedural noise,
3 geometry / 5 normal octaves, matrix math, reflection, refraction,
Fresnel lighting and animated camera motion. No textures, external inputs,
or framework changes are required. The 32-byte typed record holds eight
scalars at offsets 0 through 28. The controls expose time scrubbing,
wave height/choppiness/speed/frequency, camera phase, exposure and normal
detail. The pane starts paused; use **Animate / Pause** to animate, and
**Compare rendering models** to display both APIs. Scrubbing pauses animation;
switching away suspends it while preserving the animation preference.

The existing **shader-paint API rejects loops**, even with constant bounds. The app
expands only the two marked loops (5 octave iterations and 32 trace steps)
for its shader-paint source. The same WGSL source retains loops for instanced shaders. This is a
manual GLSL-to-WGSL adaptation, not direct GLSL ingestion or a generic
Shadertoy loader. Host inputs replace `iTime`, `iResolution` and the mouse
camera phase. UV Y is inverted for Shadertoy's bottom-left coordinates;
reversed smoothstep and numerical edge cases are made explicit. Optional
3×3 supersampling is disabled, as in the author's default source.

```sh
cargo test --features test-support shader::
cargo run -- --shader-benchmark seascape --output artifacts/seascape-native
# Reuses the optional wgpu validation feature introduced for Grid:
cargo run --features grid-wgpu -- --shader-benchmark seascape --output artifacts/seascape-parity
```

Reports and PNGs are written under `seascape/` in the output directory.
Checks cover the analytic sky against an independent CPU calculation,
image similarity between shader paints and instanced shaders, time changes at
the same extent, parameter and
aspect-ratio changes, clipping, opacity, replay and immutable shader-paint snapshots.
Optional wgpu checks compare each API with its native rendering. Similarity
requires mean RGB error ≤1/255 and at most 0.5% of pixels differing by more
than 24/255; the report also retains maximum error. Procedural sine-hash
noise and sharp highlights can amplify differences between looped and
expanded evaluation, so bit-exact parity between the rendering models is not
required. These checks
do not certify pixel parity with the original Shadertoy GLSL.

The benchmark records 12 changing frames per API at 320×180, including scene
creation, render, GPU wait and readback. It does not establish interactive
FPS, Windows driver execution or browser/WebGL2 compatibility.

## Chromaticity and color gamut lab

Run `just chromaticity` here, or `just profile=dev chromaticity` from the sibling xgpui root, for a standalone CIE xy/u′v′ diagram, explicit color-pipeline inspector, and native-wide/sRGB rendering comparison. See [controls, dependencies and validation](validation/chromaticity-lab.md).
