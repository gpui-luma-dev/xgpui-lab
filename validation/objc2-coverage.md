# objc2 / Metal migration coverage

Assessed xgpui revision: `534cdf465615be127480b5add62b2b8df89915dd`.

macOS Apple Silicon; Intel Managed storage and other OSes not validated.

Assessed commit plus uncommitted test-support hooks in gpui-apple metal_renderer.rs, gpui platform.rs/window.rs, and gpui-macos window.rs. Production rendering behavior is unchanged; native ownership probes require these hooks.

This is a reviewed coverage inventory, not a claim that every path is migrated or that every test ran on this machine. Regenerate with `just coverage-report`; execute native tests and the separate consumer audit to refresh evidence. A source-reference check proves presence, not execution.

## Legend

- **V** — Verified specific assertion with named native evidence; not blanket coverage.
- **P** — Partial or indirect, shared infrastructure, synthetic state, or narrower producer coverage.
- **G** — Gap: no direct assertion located.
- **N** — Not applicable to the explicitly stated resource; not a substitute for testing related caches.

## Rendering paths and migration boundaries

| Path | Migration boundary | Rendering oracle |
|---|---|---|
| Atlas allocation/upload and sprite binding | objc2 atlas; temporary legacy binding bridge | Exact monochrome/BGRA byte and pixel assertions |
| Text/glyph rendering | Atlas migrated; text shaping/rasterization and compositor not fully migrated | Native/Parley screenshots only |
| Quads, borders, paths and masks | Compositor remains metal-rs; migrated atlas bindings can participate in glyph masks | Independent native/custom geometry comparisons |
| Atlas sprites and dedicated MediaImage textures | Atlas migrated; dedicated uploads/compositor remain legacy Metal | Atlas exact pixels; dedicated image capture/upload checks |
| Custom solid, gradient and mesh paints | Producers remain metal-rs; queue retention uses objc2 | Native alpha/coverage/geometry pixel assertions |
| Portable WGSL and instanced shader programs | Compiler/pipelines/compositor remain legacy; shared queue bridge is a migrated dependency | WGSL pixels and parameter/source-cache assertions |
| WideColor producer and wide-SDR composition | WideColor allocation/encoding migrated to objc2; host compositor still legacy | CPU reference, native float-P3 assertions and instanced parity |
| Custom-draw CoreVideo BGRA/NV12 | Import device/texture bridge migrated; compositing remains legacy | All five format/range/matrix paths checked |
| Native PaintSurface BGRA/full-range NV12 | Texture bridge migrated; cache ownership fix committed | Independent Surface pixels; cache ownership after renderer drop |

## Ownership

| Path | pending | submitted | completed | abandoned | failed | renderer destruction | window destruction |
|---|---|---|---|---|---|---|---|
| Atlas allocation/upload and sprite binding | V (AT, LC, OW) | V (LC, OW) | V (AT, OW) | V (OW) | P (OW) | V (LC, OW) | V (LC, OW) |
| Text/glyph rendering | P (OW) | P (TX, OW) | P (TX, OW) | P (OW) | P (OW) | P (TX, OW) | V (TX, OW) |
| Quads, borders, paths and masks | V (SH, OW) | V (SH, OW) | V (SH, OW) | V (OW) | P (FR, OW) | V (LC, OW) | V (LC, OW) |
| Atlas sprites and dedicated MediaImage textures | V (IM, OW) | V (LC, OW) | V (IM, OW) | V (OW) | P (IM, OW) | V (LC, OW) | V (LC, OW) |
| Custom solid, gradient and mesh paints | V (SC, OW) | V (SC, OW) | V (SC, OW) | V (WS, OW) | P (SC, FR, OW) | V (LC, OW) | V (LC, OW) |
| Portable WGSL and instanced shader programs | V (WG, OW) | V (WG, OW) | V (WG, OW) | V (OW) | P (WG, FR, OW) | V (LC, OW) | V (LC, OW) |
| WideColor producer and wide-SDR composition | V (WO, OW) | V (WO, OW) | V (WO, OW) | V (WS, OW) | P (WO, WS, OW) | V (WO, WS, OW) | V (LC, OW) |
| Custom-draw CoreVideo BGRA/NV12 | V (CV, OW) | V (LC, OW) | V (CV, OW) | V (OW) | P (FR, OW) | V (LC, OW) | V (OW) |
| Native PaintSurface BGRA/full-range NV12 | V (CS, OW) | V (LC, OW) | V (CS, OW) | V (OW) | P (FR, OW) | V (CS, LC, OW) | V (OW) |

## Reuse

| Path | replay | eviction | resize | queue separation | producer replacement |
|---|---|---|---|---|---|
| Atlas allocation/upload and sprite binding | V (AT) | V (AT) | P (IM) | P (LC) | V (AT, LC) |
| Text/glyph rendering | P (TX) | P (AT) | G | G | G |
| Quads, borders, paths and masks | V (SH) | N | P (SH) | G | P (SH) |
| Atlas sprites and dedicated MediaImage textures | V (AT) | V (AT) | P (IM) | G | P (LC, IM) |
| Custom solid, gradient and mesh paints | V (SH) | V (SC) | V (SC) | V (SC) | V (SH) |
| Portable WGSL and instanced shader programs | V (WG) | P (WG) | V (WG) | P (SC) | V (WG) |
| WideColor producer and wide-SDR composition | V (GP, GI) | N | V (GP, GI) | V (WO) | P (GI) |
| Custom-draw CoreVideo BGRA/NV12 | V (VP) | P (LC) | V (VP) | P (LC) | V (LC) |
| Native PaintSurface BGRA/full-range NV12 | V (SP) | P (LC) | V (SP) | G | V (LC) |

## Scope limits by path

- **Atlas allocation/upload and sprite binding:** Unsubmitted sampling abandonment releases the actual host pool; completion pixels prove atlas textures survive renderer destruction. Atlas upload itself already occurred during fixture preparation. Synthetic masks do not establish glyph raster correctness; Intel Managed storage remains untested.
- **Text/glyph rendering:** Mixed native windows exercise text/emoji with unpainted, completed and GPU-pending teardown. Shared native pool/atlas ownership is covered indirectly; no isolated glyph-resource weak observer or glyph pixel goldens, fractional coverage, font/size changes.
- **Quads, borders, paths and masks:** Native quad and filled path survive renderer destruction with exact interior pixels. Mixed windows include rounded/bordered shapes. CPU preparation rejection is a later custom producer failure, not shape allocation or GPU-error injection; mask cancellation and resize remain separate.
- **Atlas sprites and dedicated MediaImage textures:** Atlas and uploaded dedicated MediaImage have completion pixel oracles and tracked producer release after abandonment/completion/window teardown. Dedicated-image cache eviction, replacement under pressure and cross-renderer reuse remain incomplete.
- **Custom solid, gradient and mesh paints:** Solid, gradient, mesh and WGSL each verify tracked producer/host-pool retention while sampling is pending and release after abandonment/completion. Mixed-window teardown covers all. CPU preparation rejection is tested; actual GPU-error cleanup is not.
- **Portable WGSL and instanced shader programs:** WGSL paint and instanced shader verify final-owner release after abandonment/completion plus pixels after renderer destruction; native mixed windows include both. Program/pipeline eviction and true cross-queue program reuse remain gaps. No GPU fault injected.
- **WideColor producer and wide-SDR composition:** WideColor has no producer LRU (N); scratch/pipeline caches are separate. Pending renderer destruction, unsubmitted sampling abandonment and mixed-window teardown now have direct probes. GPU errors and interrupted producer replacement remain untested; physical display fidelity unverified.
- **Custom-draw CoreVideo BGRA/NV12:** BGRA and all four NV12 range/matrix variants verify pending producer retention and final-owner release after sampling abandonment/completion and mixed-window teardown. Failed import/allocation and same snapshot across two queues remain gaps. Producer preparation may have submitted work before host sampling abandonment.
- **Native PaintSurface BGRA/full-range NV12:** BGRA/full-range NV12 verify host/cache/buffer release after unsubmitted sampling abandonment, completion and CPU preparation rejection, plus mixed-window teardown. Import failure and cross-queue reuse remain gaps; native Surface does not gain custom-video BT.709/video-range support.

## Evidence catalog

Paths are relative to this report. Test and log names describe the narrow assertion they support.

- **AT:** [Atlas odd-width pixels, cache hits, replay, eviction and changed-data reinsertion](../src/media_lab/benchmarks/atlas_checks.rs); witness `pub(super) fn verify`; execution: `just regression-tests / atlas`.
- **TX:** [Native/Parley text atlas captures; no glyph pixel oracle](../src/parley_lab/native.rs); witness `window.render_to_image()?.save`; execution: `capture-only; not counted as glyph correctness`.
- **SH:** [Native/custom shapes, paths, opacity, clipping and replay pixel comparisons](../src/shader/benchmarks/mod.rs); witness `native/custom coverage differs`; execution: `shader-pixels`.
- **SC:** [Solid/gradient/mesh cache: pending sharing, completed reuse, eviction, queue isolation and generation errors](../../xgpui/extensions/crates/xgpui-shader/src/metal_cache/tests.rs); witness `all_producers_separate_queues_on_the_same_device`; execution: `shader-cache-tests.log`.
- **WO:** [Owned objc2 texture/command transfer, identity, pending state, submission and queue rejection](../../xgpui/gpui/crates/gpui-apple/src/custom_draw/wide_output_tests.rs); witness `objc2_producer_transfers_owned_resources_without_submitting`; execution: `apple-tests.log`.
- **WS:** [Wide SDR float pixels, ordering, coverage, rejected contracts, mode switches and renderer isolation](../../xgpui/gpui/crates/gpui-apple/src/metal_renderer/wide_sdr/tests.rs); witness `wide_sdr_invalid_contract_discards_pending_commands_and_recovers`; execution: `apple-tests.log`.
- **CV:** [CoreVideo snapshot ownership through pending sampling and completion (BGRA + four NV12 variants)](../../xgpui/gpui/crates/gpui-apple/src/metal_renderer.rs); witness `fn core_video_snapshot_lives_until_sampling_completion`; execution: `apple-tests.log`.
- **CS:** [Native Surface retains cache after renderer destruction; completion releases cache owner](../../xgpui/gpui/crates/gpui-apple/src/metal_renderer.rs); witness `fn surface_sampling_retains_cache_after_renderer_destruction`; execution: `apple-tests.log`.
- **VP:** [Custom video exact bars/range/matrix, masks, opacity and import reuse](../src/media_lab/benchmarks/video.rs); witness `frame.import_count() == 1`; execution: `just regression-tests / video`.
- **SP:** [Independent native PaintSurface BGRA and colored full-range NV12 pixels, clipping, resize and replay](../src/media_lab/benchmarks/surface.rs); witness `Surface replay changed pixels`; execution: `just regression-tests / video`.
- **LC:** [Renderer/window churn, shader sharing, nonblocking atlas/video/Surface retirement, RSS and reference counts](../src/media_lab/benchmarks/lifecycle.rs); witness `first.render_scene(&scene, extent)?`; execution: `just regression-tests / lifecycle`.
- **GP:** [Independent CPU reference for wide compositing, alpha, order, masks, overlays and resize](../src/gamut/compositing_checks.rs); witness `let expected = linear.map(encode)`; execution: `just regression-tests / texture`.
- **GI:** [Instanced P3 versus typed producer pixels, alpha, replay, resize and default-mode rejection](../../xgpui/extensions/crates/xgpui-gamut/tests/instanced_output.rs); witness `wide_instances_match_typed_producers_and_reject_default_mode`; execution: `gamut-tests.log`.
- **WG:** [Portable WGSL parameter/source cache semantics and pixel checks](../src/shader/benchmarks/portable_checks.rs); witness `fn verify(`; execution: `shader-pixels`.
- **FR:** [Batch validation, queue/device/extent rejection and state rechecks; not actual GPU faults](../../xgpui/gpui/crates/gpui-apple/src/custom_draw/submission_tests.rs); witness `submission_validates_whole_batch_before_committing_any_commands`; execution: `apple-tests.log`.
- **IM:** [GPUI atlas versus dedicated MediaImage captures and upload-count checks](../src/media_lab/benchmarks/atlas.rs); witness `capture unexpectedly reuploaded a source`; execution: `just regression-tests / atlas`.
- **OW:** [17 native resource variants: unsubmitted sampling abandonment, gated submission/completion after renderer destruction, CPU preparation rejection and mixed native window teardown](../src/media_lab/benchmarks/ownership.rs); witness `pub(super) async fn run(`; execution: `just regression-tests / lifecycle / worker/lifecycle/ownership/report.json (51 frame cases + 3 mixed-window cases)`.

## Gates before claiming a complete transition

| Gate | Current status | Required evidence |
|---|---|---|
| abandonment | verified-sampling-scope | 51 frame probes cover 17 resource variants across host sampling abandonment, gated completion after renderer destruction and CPU preparation rejection. Abandoned sampling remains NotEnqueued; actual pool/producer/cache/buffer owners release. Producer preparation/upload may already have submitted work. Text is covered through mixed-window/shared-atlas teardown, not isolated glyph cancellation. |
| failed | partial | Every native resource variant rejects a later CPU producer preparation and checks final-owner release. Actual GPU command Error, allocation and native import failure cleanup remain unverified; no intentional GPU fault is injected. |
| windows | verified-native-backend | Three mixed-window cases close before painting, after completion and while GPU work is gated. Text, shapes, atlas/dedicated images, WGSL/instanced shaders, wide color, all custom video variants and native Surfaces are included. Test-support submission uses the window renderer with an offscreen target; drawable presentation itself is not asserted. |
| reuse | partial | Run shader-program, video snapshot and Surface reuse across distinct queues and teardown; strengthen glyph/dedicated-image oracles. |
| consumer | published-baseline-only | Published registry consumer must build without patches, then repeat against the eventual candidate release. Current 534cdf4 migration/fix is not represented by published 1.0.0. |
| legacy-removal | not-met | Prove metal/objc/block/cocoa are absent from selected macOS graph when full migration claims their removal. |
| hardware | gap | Run on Intel Managed storage and appropriate display configurations; timing/RSS baselines must be hardware/profile matched. |
| source-pin | not-met | CI currently pins 9e6a732110a1fb1cd6b7ff12bf306d95ceb1fb12; align it with the reviewed fix revision 534cdf465615be127480b5add62b2b8df89915dd before relying on CI evidence. |
| automation | partial | Native benchmark evidence is local/manual. Existing CI uses mocked app tests and manual dispatch; add native hardware execution with retained reports before making it a release gate. |

## Clean consumer evidence

Run `just consumer-audit`. The audit uses a temporary project, fresh target directory and isolated Cargo configuration; it may reuse the registry download/source cache. All dependencies must have registry sources; no `[patch]`, path, or Git dependencies are allowed. The audit performs `cargo build`, saves the lockfile/metadata/logs, and records selected macOS paths to each legacy dependency.

The verified published baseline is xgpui/platform 1.0.0 plus all six extensions 0.2.0. It does not include this checkout's newer migration/fix, gpui-luma 0.2.4, or the entire lab application. A registry build cannot validate unpublished source changes. Candidate-release consumer verification remains a separate gate.

Observed published macOS dependencies: `metal 0.33.0`, `objc 0.2.7`, `block 0.1.6`, `cocoa 0.26.0`, `cocoa-foundation 0.2.1`, and `objc_exception 0.1.2`. Older Core Foundation/Graphics/Text wrappers also remain. These coexist with objc2; a successful build does not prove their removal. See `artifacts/objc2-coverage/consumer-final/report.json` for exact dependency chains and `build.stderr.log` for compiler warnings.
