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

Every extension lab has an embedded **Benchmarks** tab. Workloads use code compiled into this application and save reports/captures to a retained temporary folder, accessible with **Reveal results**. No benchmark builds another application or requires Cargo, Python, sibling source trees, or cached build dependencies at runtime. Parley backend isolation and SVG corpus isolation launch fresh copies of this executable. External SVG artwork is selected by the user and is not downloaded automatically.

Gamut → **Benchmarks** runs the embedded Gamut renderer and Luma controls directly in this application. Wide texture, instanced, gradient, SDR baseline, and current-control workloads draw five frames in a temporary hidden window. Results include per-frame CPU draw and render/readback timings, negotiated presentation status, and clipped sRGB PNG previews. GPU execution time and physical display fidelity are not measured. **Reveal results** opens the retained temporary results folder. These workloads are available in portable builds and do not invoke Cargo or external source workers.

For an explicit native smoke run using an already-built app:

```sh
./target/debug/xgpui-lab --gamut-benchmark all --output artifacts/gamut-benchmarks
```

Media → **Benchmarks** embeds upload timing, visible frame cadence, sustained scrolling, atlas/mipmap capture, and native video pixel checks. Upload/capture workloads run on background threads; cadence and scrolling use temporary windows in this application. The scroll workload measures 5 seconds of baseline, 30 seconds of repeated decode/mip/upload replacement, and 5 seconds of cached scrolling. Keep measurement windows visible and foreground. Debug builds support functional runs and identify their profile; provisional release performance criteria are never reported as passed in debug builds. CoreVideo validation is available on macOS and explicitly unavailable on other platforms. **Cancel** stops a run and **Reveal results** opens its retained temporary reports and captures. No Media benchmark invokes Cargo or another executable.

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

- xgpui: `73262f80be7e2b9bb8fa5cdc333f1bb3e7421b9e` on `codex/issue-51-build-hygiene`.
- gpui-luma: `f49e03238db5fc36d0be5df42fa3ad21a872aff8` on `main`.

Cargo path dependencies and patches select the sibling xgpui checkout. Sample fonts, shader fixtures and the default SVG corpus live in `assets/samples/` and are embedded. Keep the dependency repositories present when compiling. Built applications do not read them at runtime. Branch names are informational; Cargo does not enforce the checked-out branch or revision.

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

## Continuous integration

Automatic `CI` runs are paused while xgpui publication/distribution and the GPUI dependency source are being settled. The workflow retains manual dispatch for explicit verification once enabled; PR and push triggers are removed.
Its stable check names are **Native (macOS)** and **Native (Windows x64)**.
Each job checks formatting, compiles all app targets with test support, runs
Clippy with warnings denied, and executes the headless test suite. Linux CI remains follow-up work; distributable Windows archives use the
separate manual workflow documented below.

CI checks out three sibling folders under the runner workspace: `app/`
(this repository), `xgpui/`, and `gpui-luma/`. Exact dependency commits live in
`.github/dependencies.json`. No crates.io publication of xgpui is required.
The pinned Luma revision still targets gpui-unofficial, so CI applies the
tracked `.github/patches/gpui-luma-xgpui.patch` to its fresh checkout. This
bridge selects the sibling xgpui GPUI/platform dependencies and is included
in the cache identity. It does not depend on uncommitted developer edits.
Remove the bridge when a tested upstream Luma revision supports xgpui directly.

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

The adapted application and lab source is covered by `LICENSE-APACHE` and documented in `NOTICE`. Font licenses are in `licenses/fonts/`. The repository's original MIT license is retained in `LICENSE`.

Benchmark progress and timings appear in the application and retained JSON reports, without stdout logging. `--version` remains a command-line metadata response. Optional file pickers load user artwork; default samples require no external folder.
