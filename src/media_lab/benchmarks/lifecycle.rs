//! Native lifetime stress; process RSS is sampled separately from producer cache accounting.
use anyhow::Result;
use gpui::AsyncApp;
use serde_json::Value;
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

pub(super) async fn run(output: &Path, cancel: &Arc<AtomicBool>, cx: &mut AsyncApp) -> Result<Value> {
    #[cfg(target_os = "macos")]
    {
        use gpui::AppContext;
        if std::env::args().any(|arg| arg == "--lifecycle-worker") {
            return native::run(output, cancel, cx).await;
        }
        // A driver/backend crash must fail this workload without terminating the lab UI.
        let worker = output.join("worker");
        std::fs::create_dir_all(&worker)?;
        let args = vec![
            "--media-benchmark".into(),
            "lifecycle".into(),
            "--lifecycle-worker".into(),
            "--output".into(),
            worker.display().to_string(),
        ];
        let path = worker.clone();
        let token = cancel.clone();
        let report = cx.background_spawn(async move { crate::benchmarks::run_self(&args, &path, &token) }).await?;
        report
            .get("cases")
            .and_then(Value::as_array)
            .and_then(|cases| cases.first())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("lifecycle worker produced no case report"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (output, cancel, cx);
        Ok(serde_json::json!({"outcome":"unavailable", "description":"Apple lifecycle stress requires macOS Metal"}))
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use anyhow::{Context as _, ensure};
    use gpui::{prelude::*, *};
    use serde::Serialize;
    use serde_json::json;
    use std::{
        borrow::Cow,
        path::Path,
        time::{Duration, Instant},
    };
    use xgpui_shader::SolidColor;
    use xgpui_media::VideoFrame;
    use crate::media_lab::video_source::SourceFormat;

    const CACHE_BUDGET: u64 = 64 * 1024 * 1024;
    #[derive(Serialize, Clone, Copy)]
    struct Config {
        cycles: usize,
        replacements_per_cycle: usize,
        max_rss_growth_bytes: u64,
    }
    impl Config {
        fn read() -> Result<Self> {
            fn number(name: &str, default: usize, min: usize, max: usize) -> Result<usize> {
                let value = std::env::var(name).map_or(Ok(default), |value| value.parse::<usize>())?;
                ensure!((min..=max).contains(&value), "{name} must be {min}..{max}");
                Ok(value)
            }
            Ok(Self {
                cycles: number("XGPUI_LIFECYCLE_CYCLES", 64, 8, 1000)?,
                replacements_per_cycle: number("XGPUI_LIFECYCLE_REPLACEMENTS", 96, 4, 1000)?,
                max_rss_growth_bytes: number("XGPUI_LIFECYCLE_MAX_RSS_GROWTH_MIB", 64, 1, 4096)? as u64 * 1024 * 1024,
            })
        }
    }
    #[derive(Serialize)]
    struct Sample {
        cycle: usize,
        phase: &'static str,
        replacement: Option<usize>,
        elapsed_ms: f64,
        rss_bytes: u64,
        shader_cache_bytes: u64,
        shader_preparations: usize,
        windows: usize,
    }
    struct Memory {
        system: sysinfo::System,
        pid: sysinfo::Pid,
        started: Instant,
        samples: Vec<Sample>,
    }
    impl Memory {
        fn new() -> Result<Self> {
            Ok(Self {
                system: sysinfo::System::new(),
                pid: sysinfo::get_current_pid().map_err(anyhow::Error::msg)?,
                started: Instant::now(),
                samples: vec![],
            })
        }
        fn sample(&mut self, cycle: usize, phase: &'static str, shader: &SolidColor, windows: usize) -> Result<()> {
            self.system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[self.pid]),
                sysinfo::ProcessRefreshKind::new().with_memory(),
            );
            let rss = self.system.process(self.pid).context("cannot sample process memory")?.memory();
            ensure!(rss > 0, "process RSS sampling returned zero");
            let bytes = shader.cached_texture_info().map_or(0, |(_, bytes)| bytes);
            ensure!(bytes <= CACHE_BUDGET, "shader cache exceeded its 64 MiB budget: {bytes}");
            self.samples.push(Sample {
                cycle,
                phase,
                replacement: None,
                elapsed_ms: self.started.elapsed().as_secs_f64() * 1000.,
                rss_bytes: rss,
                shader_cache_bytes: bytes,
                shader_preparations: shader.preparations(),
                windows,
            });
            Ok(())
        }
        fn save(&self, output: &Path, config: Config) -> Result<()> {
            std::fs::write(
                output.join("memory-samples.json"),
                serde_json::to_vec_pretty(&json!({"config":config, "samples":self.samples}))?,
            )?;
            let mut csv = String::from(
                "cycle,phase,replacement,elapsed_ms,rss_bytes,shader_cache_bytes,shader_preparations,windows\n",
            );
            for s in &self.samples {
                csv.push_str(&format!(
                    "{},{},{},{:.3},{},{},{},{}\n",
                    s.cycle,
                    s.phase,
                    s.replacement.map_or(String::new(), |n| n.to_string()),
                    s.elapsed_ms,
                    s.rss_bytes,
                    s.shader_cache_bytes,
                    s.shader_preparations,
                    s.windows
                ));
            }
            std::fs::write(output.join("memory.csv"), csv)?;
            // A per-run view complements raw samples; comparisons across runs still need matching hardware/profile.
            let low = (self.samples.iter().map(|s| s.rss_bytes).min().unwrap_or(0) as f64 / 1048576.).floor();
            let high =
                (self.samples.iter().map(|s| s.rss_bytes).max().unwrap_or(0) as f64 / 1048576.).ceil().max(low + 1.);
            let elapsed = self.samples.last().map_or(1., |s| s.elapsed_ms).max(1.);
            let points = self
                .samples
                .iter()
                .map(|s| {
                    format!(
                        "{:.2},{:.2}",
                        70. + s.elapsed_ms / elapsed * 760.,
                        250. - (s.rss_bytes as f64 / 1048576. - low) / (high - low) * 190.
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="900" height="320" viewBox="0 0 900 320"><rect width="900" height="320" fill="white"/><g fill="#222" font-family="sans-serif" font-size="13"><text x="70" y="24" font-size="18">Lifecycle stress: process resident memory</text><text x="70" y="44">Sampled RSS includes runtime/driver caches; total GPU memory is not measured.</text><text x="10" y="65">{high:.0} MiB</text><text x="10" y="254">{low:.0} MiB</text><text x="70" y="275">0 s</text><text x="790" y="275">{seconds:.1} s</text><text x="70" y="303">Raw samples and shader-cache bytes: memory.csv / memory-samples.json</text></g><path d="M70 60 V250 H830" fill="none" stroke="#888"/><polyline points="{points}" fill="none" stroke="#1668b8" stroke-width="1.5"/></svg>"##,
                seconds = elapsed / 1000.
            );
            std::fs::write(output.join("memory.svg"), svg)?;
            Ok(())
        }
    }
    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
        super::super::atlas::rect(x, y, w, h)
    }
    fn custom(scene: &mut Scene, bounds: Bounds<ScaledPixels>, content: Arc<dyn CustomContent>) {
        scene.insert_primitive(CustomDraw {
            order: 0,
            bounds,
            content_mask: ContentMask { bounds },
            corner_radii: Default::default(),
            opacity: 1.,
            coverage: None,
            content,
        });
    }
    fn pixel(image: &image::RgbaImage, x: u32, y: u32, expected: [u8; 3], label: &str) -> Result<()> {
        let actual = image.get_pixel(x, y);
        ensure!(
            actual.0[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3),
            "{label} at {x},{y}: {actual:?}, expected {expected:?}"
        );
        Ok(())
    }
    fn shader_scene(shader: Arc<SolidColor>) -> Scene {
        let mut scene = Scene::default();
        custom(&mut scene, rect(0., 0., 48., 48.), shader);
        scene.finish();
        scene
    }
    fn check_shader(renderer: &mut dyn PlatformHeadlessRenderer, shader: Arc<SolidColor>) -> Result<()> {
        let scene = shader_scene(shader.clone());
        let extent = size(DevicePixels(192), DevicePixels(96));
        let first = renderer.render_scene_to_image(&scene, extent)?;
        pixel(&first, 24, 24, [64, 128, 192], "shared shader")?;
        let prepared = shader.preparations();
        ensure!(renderer.render_scene_to_image(&scene, extent)? == first, "shared shader replay changed pixels");
        ensure!(shader.preparations() == prepared, "shared shader cache missed on same renderer/extent");
        Ok(())
    }
    fn renderers(
        shader: Arc<SolidColor>,
        config: Config,
        cycle: usize,
        output: &Path,
        cancel: &AtomicBool,
        memory: &mut Memory,
        windows: usize,
    ) -> Result<()> {
        objc2::rc::autoreleasepool(|_| -> Result<()> {
            let mut first = gpui_platform::current_headless_renderer()?.context("native renderer unavailable")?;
            let mut survivor =
                gpui_platform::current_headless_renderer()?.context("second native renderer unavailable")?;
            let before = shader.preparations();
            check_shader(first.as_mut(), shader.clone())?;
            check_shader(survivor.as_mut(), shader.clone())?;
            ensure!(shader.preparations() == before + 2, "new renderer queues reused another queue's shader output");
            for frame in 0..config.replacements_per_cycle {
                super::super::check_cancel(cancel)?;
                ensure!(memory.started.elapsed() < Duration::from_secs(300), "lifecycle stress exceeded 300 seconds");
                objc2::rc::autoreleasepool(|_| -> Result<()> {
                    let color = 20 + ((cycle * config.replacements_per_cycle + frame) * 17 % 200) as u8;
                    let pixels = image::RgbaImage::from_pixel(31, 17, image::Rgba([40, 80, color, 255])); // BGRA upload
                    let image = RenderImage::new(vec![image::Frame::new(pixels)]);
                    let key = AtlasKey::Image(RenderImageParams { image_id: image.id, frame_index: 0 });
                    let tile = first
                        .sprite_atlas()
                        .get_or_insert_with(key.clone(), &mut || {
                            Ok(Some((image.size(0), Cow::Borrowed(image.as_bytes(0).unwrap()))))
                        })?
                        .context("atlas did not allocate tile")?;
                    let native = SourceFormat::ALL[frame % SourceFormat::ALL.len()].frame(frame as u64)?;
                    let video = VideoFrame::core_video(native.clone(), Duration::from_millis(frame as u64 * 33));
                    let mut scene = shader_scene(shader.clone());
                    // Rebuild a scene rather than mutate a finished scene's replay operations.
                    let mut combined = Scene::default();
                    combined.replay(0..scene.len(), &scene);
                    let bounds = rect(64., 0., 31., 17.);
                    combined.insert_primitive(PolychromeSprite {
                        order: 0,
                        pad: 0,
                        grayscale: false.into(),
                        opacity: 1.,
                        bounds,
                        content_mask: ContentMask { bounds },
                        corner_radii: Default::default(),
                        tile,
                    });
                    custom(&mut combined, rect(104., 0., 80., 48.), video.content());
                    let bounds = rect(104., 48., 80., 40.);
                    combined.insert_primitive(PaintSurface {
                        order: 0,
                        bounds,
                        content_mask: ContentMask { bounds },
                        image_buffer: super::super::surface::buffer(frame % 2 == 0, frame % 2 != 0)?,
                    });
                    combined.finish();
                    let extent = size(DevicePixels(192), DevicePixels(96));
                    let actual = first.render_scene_to_image(&combined, extent)?;
                    if frame == config.replacements_per_cycle - 1 && (cycle == 0 || cycle == config.cycles - 1) {
                        actual.save(output.join(format!("replacement-{cycle}.png")))?;
                    }
                    pixel(&actual, 24, 24, [64, 128, 192], "shader during replacement")?;
                    pixel(&actual, 75, 8, [color, 80, 40], "replaced atlas image")?;
                    pixel(&actual, 110, 6, [224, 64, 32], "replaced video")?;
                    let expected = if frame % 2 == 0 { [186, 61, 39] } else { [32, 192, 224] };
                    pixel(&actual, 114, 64, expected, "replaced native Surface")?;
                    scene = Scene::default();
                    scene.replay(0..combined.len(), &combined);
                    scene.finish();
                    drop(combined);
                    drop(video);
                    let prepared = shader.preparations();
                    ensure!(
                        first.render_scene_to_image(&scene, extent)? == actual,
                        "replacement scene replay changed pixels"
                    );
                    ensure!(native.import_count() == 1, "video replay reimported a frame");
                    ensure!(shader.preparations() == prepared, "video/atlas replacement regenerated shared shader");
                    if frame % 6 == 0 || frame == config.replacements_per_cycle - 1 {
                        memory.sample(cycle, "replacement_active", &shader, windows)?;
                        memory.samples.last_mut().unwrap().replacement = Some(frame);
                    }
                    // Submit without readback/wait before retiring CPU owners and the atlas key.
                    // The next frame drains this queue; the last submission also crosses host teardown.
                    if frame % 6 == 0 || frame == config.replacements_per_cycle - 1 {
                        memory.save(output, config)?;
                    }
                    std::fs::write(
                        output.join("checkpoint.json"),
                        serde_json::to_vec_pretty(
                            &json!({"cycle":cycle,"replacement":frame,"phase":"before_nonblocking_submission"}),
                        )?,
                    )?;
                    first.render_scene(&scene, extent)?;
                    first.sprite_atlas().remove(&key);
                    ensure!(!first.sprite_atlas().contains(&key), "retired atlas key remains live");
                    Ok(())
                })?;
            }
            memory.save(output, config)?;
            std::fs::write(
                output.join("checkpoint.json"),
                serde_json::to_vec_pretty(&json!({"cycle":cycle,"phase":"before_renderer_drop"}))?,
            )?;
            drop(first);
            check_shader(survivor.as_mut(), shader)?; // Producer outlives one host; peer must still work.
            drop(survivor);
            std::fs::write(
                output.join("checkpoint.json"),
                serde_json::to_vec_pretty(&json!({"cycle":cycle,"phase":"renderers_dropped"}))?,
            )?;
            Ok(())
        })
    }
    struct StressWindow {
        shader: Arc<SolidColor>,
    }
    impl Render for StressWindow {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let shader = self.shader.clone();
            div().size_full().bg(gpui::black()).child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        window.paint_custom(
                            CustomGeometry::RoundedRect { bounds, corner_radii: Default::default() },
                            shader.clone(),
                        )
                    },
                )
                .size(px(48.)),
            )
        }
    }
    fn steady_growth(samples: &[Sample], cycles: usize) -> Result<(u64, u64, u64)> {
        let steady: Vec<u64> = samples
            .iter()
            .filter(|s| s.phase == "window_closed" && s.cycle >= cycles / 4)
            .map(|s| s.rss_bytes)
            .collect();
        ensure!(steady.len() >= 6, "insufficient post-warmup RSS samples");
        let median = |values: &[u64]| {
            let mut values = values.to_vec();
            values.sort_unstable();
            values[values.len() / 2]
        };
        let early = median(&steady[..steady.len() / 3]);
        let late = median(&steady[steady.len() * 2 / 3..]);
        Ok((early, late, late.saturating_sub(early)))
    }

    pub(super) async fn run(output: &Path, cancel: &Arc<AtomicBool>, cx: &mut AsyncApp) -> Result<Value> {
        let ownership = super::super::ownership::run(&output.join("ownership"), cancel, cx).await?;
        let config = Config::read()?;
        std::fs::create_dir_all(output)?;
        let shader = Arc::new(SolidColor::new([64. / 255., 128. / 255., 192. / 255., 1.])?);
        let mut memory = Memory::new()?;
        let baseline = cx.update(|cx| cx.windows().len());
        memory.sample(0, "baseline", &shader, baseline)?;
        memory.save(output, config)?;
        let start = Instant::now();
        for cycle in 0..config.cycles {
            super::super::check_cancel(cancel)?;
            ensure!(start.elapsed() < Duration::from_secs(300), "lifecycle stress exceeded 300 seconds");
            let shared = shader.clone();
            let path = output.to_path_buf();
            let token = cancel.clone();
            let (next_memory, result) = cx
                .background_spawn(async move {
                    let mut memory = memory;
                    let result = renderers(shared, config, cycle, &path, &token, &mut memory, baseline);
                    (memory, result)
                })
                .await;
            memory = next_memory;
            memory.save(output, config)?;
            result.with_context(|| format!("renderer stress cycle {cycle}"))?;
            memory.sample(cycle, "renderers_dropped", &shader, baseline)?;
            let shared = shader.clone();
            let handle = cx.update(|cx| {
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(0.), px(0.)),
                            size(px(160.), px(120.)),
                        ))),
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    |_, cx| cx.new(|_| StressWindow { shader: shared }),
                )
            })?;
            let captured = cx.update(|cx| {
                cx.update_window(handle.into(), |_, window, cx| -> Result<()> {
                    window.refresh();
                    window.draw(cx).clear(cx);
                    let image = window.render_to_image()?;
                    let scale = window.scale_factor();
                    pixel(
                        &image,
                        (24. * scale) as u32,
                        (24. * scale) as u32,
                        [64, 128, 192],
                        "shared shader in new window",
                    )?;
                    if cycle == 0 || cycle == config.cycles - 1 {
                        image.save(output.join(format!("window-{cycle}.png")))?;
                    }
                    Ok(())
                })
            });
            // Always close, including pixel/readback failures.
            let closed = cx.update(|cx| handle.update(cx, |_, window, _| window.remove_window()));
            captured??;
            closed?;
            cx.background_executor().timer(Duration::from_millis(25)).await;
            let windows = cx.update(|cx| cx.windows().len());
            memory.sample(cycle, "window_closed", &shader, windows)?;
            memory.save(output, config)?;
            ensure!(Arc::strong_count(&shader) == 1, "window or scene retained the shader producer after teardown");
            ensure!(windows == baseline, "window count did not return to baseline: {windows} != {baseline}");
            if (cycle + 1) % 8 == 0 || cycle + 1 == config.cycles {
                let rss = memory.samples.last().unwrap().rss_bytes;
                eprintln!(
                    "Lifecycle {}/{} cycles: {} replacements per stream, RSS {:.1} MiB",
                    cycle + 1,
                    config.cycles,
                    (cycle + 1) * config.replacements_per_cycle,
                    rss as f64 / 1048576.
                );
            }
        }
        let (early, late, growth) = steady_growth(&memory.samples, config.cycles)?;
        let peak = memory.samples.iter().map(|s| s.rss_bytes).max();
        let cache = shader.cached_texture_info().map_or(0, |(_, bytes)| bytes);
        let preparations = shader.preparations();
        drop(shader);
        cx.background_executor().timer(Duration::from_millis(50)).await;
        // A fresh producer measures memory after the shared producer and its retained queues are released.
        memory.sample(config.cycles, "producer_dropped", &SolidColor::new([0., 0., 0., 1.])?, baseline)?;
        memory.save(output, config)?;
        let report = json!({"schema_version":1,"ownership":ownership,"config":config,"checks":["repeated renderer and window destruction","cross-queue shader isolation and cached reuse","survivor renderer after peer destruction","atlas/video/Surface replacement pixels and replay","video import reuse","atlas retirement after nonblocking submission","window count returns to baseline","bounded shader cache","post-warmup RSS growth"],"renderers_created":config.cycles*2,"windows_created_and_closed":config.cycles,"atlas_replacements":config.cycles*config.replacements_per_cycle,"video_replacements":config.cycles*config.replacements_per_cycle,"surface_replacements":config.cycles*config.replacements_per_cycle,"shader_preparations":preparations,"shader_cache_bytes_before_drop":cache,"sampled_process_rss_peak_bytes":peak,"steady_early_median_rss_bytes":early,"steady_late_median_rss_bytes":late,"rss_growth_bytes":growth,"memory_limit_pass":growth<=config.max_rss_growth_bytes,"samples":memory.samples,"measurement":"Whole-process resident memory sampled during replacement and after renderer/window teardown, including runtime and driver caches. Does not measure total GPU memory; not a leak detector or performance benchmark. Growth compares first/last thirds after discarding first quarter of cycles. Default 64 MiB is a configurable smoke threshold, not an established baseline.","errors":if growth>config.max_rss_growth_bytes {vec![format!("RSS grew {growth} bytes, exceeding {}",config.max_rss_growth_bytes)]} else {vec![]}});
        Ok(report)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn samples(values: &[u64]) -> Vec<Sample> {
            values
                .iter()
                .enumerate()
                .map(|(cycle, &rss_bytes)| Sample {
                    cycle,
                    phase: "window_closed",
                    replacement: None,
                    elapsed_ms: 0.,
                    rss_bytes,
                    shader_cache_bytes: 0,
                    shader_preparations: 0,
                    windows: 1,
                })
                .collect()
        }
        #[::core::prelude::v1::test]
        fn warmup_spikes_do_not_hide_sustained_rss_growth() {
            let stable = samples(&[90000, 80000, 100, 101, 100, 101, 100, 101]);
            assert_eq!(steady_growth(&stable, 8).unwrap(), (101, 101, 0));
            let growing = samples(&[90000, 80000, 2100, 3100, 4100, 5100, 6100, 7100]);
            assert_eq!(steady_growth(&growing, 8).unwrap(), (3100, 7100, 4000));
        }
        #[::core::prelude::v1::test]
        fn rss_requires_completed_post_teardown_samples() {
            let mut values = samples(&[100; 8]);
            for value in &mut values {
                value.phase = "replacement_active";
            }
            assert!(steady_growth(&values, 8).is_err());
        }
    }
}
