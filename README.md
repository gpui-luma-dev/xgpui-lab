# XGPUI Lab

Initial working version of a unified native benchmark, testing, and validation app for xgpui extensions. The shell and interactive controls use GPUI Luma. The original Gradients and Compositions workspaces from gpui-luma-color-viz remain separate from the extension labs.

## Current progress

| Workspace | Implemented coverage |
| --- | --- |
| Color | Color spaces, conversions, interpolation, gamut mapping, CPU gradients |
| Gamut | P3 negotiation with sRGB fallback, wide fills/gradients, native versus CPU mapping, presentation routes |
| Media | Image viewing, atlas/mipmap comparison, filters, linear adjustments, float/P3, tiled canvas, video frames, resource limits |
| Parley | Shaping, distinct line layout and editable paragraphs, fallback/fonts, rasterization, scrolling, separate native/Parley glyph-atlas diagnostics |
| Shader | Analytic gradients, direct/legacy Hermite editing, gradient coverage, WGSL, instancing, typography selection/effects, brushes, layer effects, shadows, borders |
| SVG | Distinct rendering/scaling views, camera gestures, path effects, scheduling/cache, benchmark views |

Validation tabs for Gamut, Media, Parley, Shader, and SVG expose isolated workloads and retain logs, reports, and captures under `artifacts/validation/`. Original source workers build offline in release on first use; their dependencies must already be cached. SVG corpus validation requires a user-selected corpus matching the source manifest. The full benchmark matrix has not been run as part of the initial publication.

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

Cargo path dependencies and patches select the sibling xgpui checkout. Some font and shader fixtures also use sibling paths. Keep those repositories present when compiling or invoking source validation workers. Branch names are informational; Cargo does not enforce the checked-out branch or revision.

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

The `CI` workflow runs on pull requests, pushes to `main`, and manual dispatch.
Its stable check names are **Native (macOS)** and **Native (Windows x64)**.
Each job checks formatting, compiles all app targets with test support, runs
Clippy with warnings denied, and executes the headless test suite. Linux CI
and distributable packaging are separate follow-up work.

CI checks out three sibling folders under the runner workspace: `app/`
(this repository), `xgpui/`, and `gpui-luma/`. Exact dependency commits live in
`.github/dependencies.json`. No crates.io publication of xgpui is required.
The pinned Luma revision still targets gpui-unofficial, so CI applies the
tracked `.github/patches/gpui-luma-xgpui.patch` to its fresh checkout. This
bridge selects the sibling xgpui GPUI/platform dependencies and is included
in the cache identity. It does not depend on uncommitted developer edits.
Remove the bridge when a tested upstream Luma revision supports xgpui directly.

The reusable `.github/actions/prepare-dependencies` action expects the app at
`app/` and can also be used by the future manual Windows builder (#5).
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
Windows native rendering and packaging still need separate smoke testing.

To reproduce the workflow commands from a prepared app checkout:

```sh
python -m unittest discover -s .github/scripts -p 'test_*.py' -v
python .github/scripts/check.py format
python .github/scripts/check.py compile
python .github/scripts/check.py clippy
python .github/scripts/check.py test
```

These commands save logs in `artifacts/ci/` and return the underlying Cargo
exit status. The portable worker cancellation fixture uses the Rust test
executable itself instead of `/bin/sleep`.

## Status and follow-up

This is a functional integration baseline, not the final polished product. The initial verification includes 60 passing tests, a development build, Clippy, selected native rendering/interaction checks, and an isolated source SVG benchmark. It does not establish complete performance or browser parity.

- [#1: Deep cleanup and consistent UI polish](https://github.com/gpui-luma-dev/xgpui-lab/issues/1)
- [#2: Lab-code ownership and WASM website migration boundaries](https://github.com/gpui-luma-dev/xgpui-lab/issues/2)

Lab extraction has not been performed. The existing xgpui website still imports some lab source directly and packages lab assets; moving those files needs a coordinated migration. Neither follow-up issue is completed by this initial snapshot.

## Source attribution

The adapted application and lab source is covered by `LICENSE-APACHE` and documented in `NOTICE`. Font licenses are in `licenses/fonts/`. The repository's original MIT license is retained in `LICENSE`.
