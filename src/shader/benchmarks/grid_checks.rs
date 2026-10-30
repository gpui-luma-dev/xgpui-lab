//! Public API rendering validation, owned by the consumer app.
use super::*;
use crate::shader::grid::{GridInstance, GridPrograms};
use gpui::ShaderDraw;

fn scene(programs: &GridPrograms, p: GridInstance, typed: bool, clipped: bool, opacity: f32) -> Result<Scene> {
    let mut scene = base();
    let bounds = rect(8., 8., 160., 96.);
    let mask = ContentMask { bounds: if clipped { rect(40., 24., 80., 48.) } else { bounds } };
    if typed {
        let mut data = [[0.; 4]; gpui::SHADER_DATA_SLOTS];
        data[0] = [p.grid_dimensions, p.cell_origin_x, p.cell_origin_y, p.line_width];
        scene.insert_primitive(gpui::Primitive::Shader(ShaderDraw {
            path_mask: None,
            glyph_mask: None,
            order: 0,
            bounds,
            content_mask: mask,
            opacity,
            scale_factor: 1.,
            program: programs.instanced.scene_program(),
            data,
        }));
    } else {
        let mut draw = record(bounds, Arc::new(programs.paint(p)?), None);
        draw.corner_radii = Corners::default();
        draw.content_mask = mask;
        draw.opacity = opacity;
        scene.insert_primitive(draw);
    }
    scene.finish();
    Ok(scene)
}
fn reference(p: GridInstance, x: u32, y: u32, clipped: bool, opacity: f32) -> u8 {
    if !(8..168).contains(&x)
        || !(8..104).contains(&y)
        || (clipped && (!(40..120).contains(&x) || !(24..72).contains(&y)))
    {
        return 0;
    }
    let u = (x as f32 - 8. + 0.5) / 160.;
    let v = (y as f32 - 8. + 0.5) / 96.;
    let dx = ((u * p.grid_dimensions).fract() - p.cell_origin_x - 0.5).abs();
    let dy = ((v * p.grid_dimensions).fract() - p.cell_origin_y - 0.5).abs();
    let t = ((1. - (1. + p.line_width) * dx.max(dy)) / 0.05).clamp(0., 1.);
    (t * t * (3. - 2. * t) * opacity * 255.).round() as u8
}
fn compare(actual: &image::RgbaImage, expected: &image::RgbaImage, label: &str) -> Result<()> {
    let differences = actual
        .pixels()
        .zip(expected.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0).any(|(a, b)| a.abs_diff(b) > 3))
        .count();
    ensure!(differences == 0, "{label}: {differences} pixels differ by more than 3/255");
    Ok(())
}
fn verify(renderer: &mut dyn PlatformHeadlessRenderer, programs: &GridPrograms) -> Result<Vec<image::RgbaImage>> {
    let extent = size(DevicePixels(176), DevicePixels(112));
    let cases = [
        GridInstance { line_width: 2., ..Default::default() },
        GridInstance::default(),
        GridInstance { grid_dimensions: 1., cell_origin_x: -0.3, cell_origin_y: 0.2, line_width: 1.4 },
        GridInstance { grid_dimensions: 100., cell_origin_x: 1., cell_origin_y: -1., line_width: 8. },
        GridInstance { grid_dimensions: 7., cell_origin_x: 0.1, cell_origin_y: -0.2, line_width: 3. },
    ];
    let mut captures = Vec::new();
    for (case, p) in cases.into_iter().enumerate() {
        for (clipped, opacity) in [(false, 1.), (true, 0.5)] {
            let expected = image::RgbaImage::from_fn(176, 112, |x, y| {
                let value = reference(p, x, y, clipped, opacity);
                image::Rgba([value, value, value, 255])
            });
            let mut pair = Vec::new();
            for typed in [false, true] {
                let image = renderer.render_scene_to_image(&scene(programs, p, typed, clipped, opacity)?, extent)?;
                compare(&image, &expected, &format!("grid case={case} api={} clip={clipped}", if typed { "instanced" } else { "shader_paint" }))?;
                pair.push(image);
            }
            compare(&pair[0], &pair[1], "shader paint/instanced parity")?;
            captures.extend(pair);
        }
    }
    Ok(captures)
}
pub(crate) fn run(output: &std::path::Path, cancel: &std::sync::atomic::AtomicBool) -> Result<serde_json::Value> {
    crate::benchmarks::check_cancel(cancel)?;
    let programs = GridPrograms::new()?;
    let mut renderer = gpui_platform::current_headless_renderer()?.context("native headless renderer unavailable")?;
    let captures = verify(renderer.as_mut(), &programs)?;
    captures[0].save(output.join("grid-shader-paint.png"))?;
    captures[1].save(output.join("grid-instanced.png"))?;
    #[cfg(feature = "grid-wgpu")]
    let parity = {
        crate::benchmarks::check_cancel(cancel)?;
        let mut wgpu = gpui_wgpu::WgpuHeadlessRenderer::new()?;
        let portable = verify(&mut wgpu, &programs)?;
        for (native, portable) in captures.iter().zip(&portable) {
            compare(native, portable, "native/wgpu parity")?;
        }
        portable[0].save(output.join("grid-wgpu-shader-paint.png"))?;
        portable[1].save(output.join("grid-wgpu-instanced.png"))?;
        "passed"
    };
    #[cfg(not(feature = "grid-wgpu"))]
    let parity = "not run; enable --features grid-wgpu";
    // Changing parameters every frame exercises immutable shader-paint snapshots and typed instanced draws.
    let mut timings = Vec::new();
    for typed in [false, true] {
        let mut samples = Vec::new();
        for frame in 0..120 {
            crate::benchmarks::check_cancel(cancel)?;
            let start = Instant::now();
            let p = GridInstance { line_width: 1.5 + 0.5 * (frame as f32 * 0.1).cos(), ..Default::default() };
            renderer.render_scene_to_image(
                &scene(&programs, p, typed, false, 1.)?,
                size(DevicePixels(176), DevicePixels(112)),
            )?;
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        samples.sort_by(f64::total_cmp);
        timings.push(serde_json::json!({"api": if typed { "instanced" } else { "shader_paint" }, "frames":120,
            "median_ms": samples[60], "p95_ms":samples[114], "max_ms":samples[119]}));
    }
    Ok(
        serde_json::json!({"checks":["public constructors", "16-byte typed ABI", "CPU reference", "shader paint/instanced parity", "parameter extremes", "non-square bounds", "clipping", "opacity"],
        "pixel_tolerance":3, "native_wgpu_parity":parity, "render_readback_timings":timings,
        "measurement":"CPU wall time including scene creation, GPU wait and readback at 160x96. Does not establish interactive 120 FPS.",
        "errors":[]}),
    )
}
