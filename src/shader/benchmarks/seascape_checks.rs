//! Seascape rendering checks through public application-facing shader APIs.
use super::*;
use crate::shader::seascape::{SeaInstance, SeaPrograms};

fn scene(
    programs: &SeaPrograms,
    p: SeaInstance,
    typed: bool,
    width: u32,
    height: u32,
    clipped: bool,
    opacity: f32,
) -> Result<Scene> {
    let mut scene = Scene::default();
    let bounds = rect(0., 0., width as f32, height as f32);
    scene.insert_primitive(Quad {
        bounds,
        content_mask: ContentMask { bounds },
        background: black().into(),
        ..Default::default()
    });
    let mask = ContentMask {
        bounds: if clipped {
            rect(width as f32 * 0.25, height as f32 * 0.25, width as f32 * 0.5, height as f32 * 0.5)
        } else {
            bounds
        },
    };
    if typed {
        let mut data = [[0.; 4]; gpui::SHADER_DATA_SLOTS];
        data[0].copy_from_slice(&p.values()[..4]);
        data[1].copy_from_slice(&p.values()[4..]);
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
fn render(
    renderer: &mut dyn PlatformHeadlessRenderer,
    programs: &SeaPrograms,
    p: SeaInstance,
    typed: bool,
    width: u32,
    height: u32,
    clipped: bool,
    opacity: f32,
) -> Result<image::RgbaImage> {
    renderer.render_scene_to_image(
        &scene(programs, p, typed, width, height, clipped, opacity)?,
        size(DevicePixels(width as i32), DevicePixels(height as i32)),
    )
}
/// Procedural sin-hash noise can vary under backend floating-point optimization.
/// Gate image similarity and retain numerical metrics; don't claim bit-exact parity.
fn compare(a: &image::RgbaImage, b: &image::RgbaImage, label: &str) -> Result<serde_json::Value> {
    ensure!(a.dimensions() == b.dimensions(), "{label}: extent mismatch");
    let mut total = 0u64;
    let mut large = 0usize;
    let mut max = 0u8;
    for (a, b) in a.pixels().zip(b.pixels()) {
        let mut pixel_max = 0;
        for (a, b) in a.0[..3].iter().zip(&b.0[..3]) {
            let diff = a.abs_diff(*b);
            total += diff as u64;
            pixel_max = pixel_max.max(diff);
            max = max.max(diff);
        }
        large += usize::from(pixel_max > 24);
        ensure!(a.0[3] == b.0[3], "{label}: alpha differs");
    }
    let mean = total as f64 / (a.width() as f64 * a.height() as f64 * 3.);
    let outlier_fraction = large as f64 / (a.width() as f64 * a.height() as f64);
    ensure!(
        mean <= 1. && outlier_fraction <= 0.005,
        "{label}: MAE={mean:.3}/255; outliers={outlier_fraction:.3}; max={max}/255"
    );
    Ok(
        serde_json::json!({"comparison":label, "mean_absolute_rgb_error":mean, "max_channel_error":max, "pixels_over_24_fraction":outlier_fraction}),
    )
}
fn sky_reference(width: u32, height: u32, x: u32, y: u32) -> [u8; 3] {
    // Independent CPU evaluation of the time=0 camera and analytic sky.
    let uv = [
        (2. * (x as f32 + 0.5) / width as f32 - 1.) * width as f32 / height as f32,
        1. - 2. * (y as f32 + 0.5) / height as f32,
    ];
    let norm = (uv[0] * uv[0] + uv[1] * uv[1] + 4.).sqrt();
    let mut ray = [uv[0] / norm, uv[1] / norm, -2. / norm + (uv[0] * uv[0] + uv[1] * uv[1]).sqrt() * 0.14];
    let norm = (ray[0] * ray[0] + ray[1] * ray[1] + ray[2] * ray[2]).sqrt();
    for r in &mut ray {
        *r /= norm;
    }
    let direction_y = ray[1] * 0.3f32.cos() + ray[2] * 0.3f32.sin();
    assert!(direction_y > 0., "reference point must be sky");
    let v = 1. - (direction_y * 0.8 + 0.2) * 0.8;
    [v * v * 1.1, v * 1.1, (0.6 + v * 0.4) * 1.1].map(|c| (c.max(0.).powf(0.65).clamp(0., 1.) * 255.).round() as u8)
}
pub(crate) fn run(output: &std::path::Path, cancel: &std::sync::atomic::AtomicBool) -> Result<serde_json::Value> {
    let programs = SeaPrograms::new()?;
    let mut native = gpui_platform::current_headless_renderer()?.context("native headless renderer unavailable")?;
    #[cfg(feature = "grid-wgpu")]
    let mut portable = gpui_wgpu::WgpuHeadlessRenderer::new()?;
    let mut comparisons = Vec::new();
    let cases = [
        (SeaInstance::default(), 320, 180),
        (SeaInstance { time: 2.5, ..Default::default() }, 192, 108),
        (
            SeaInstance {
                time: 4.,
                sea_height: 1.2,
                choppiness: 6.,
                frequency: 0.25,
                camera: -0.4,
                detail: 0.,
                ..Default::default()
            },
            160,
            160,
        ),
    ];
    let mut first = None;
    let mut first_shader_paint = None;
    let retained_shader_paint = scene(&programs, SeaInstance::default(), false, 320, 180, false, 1.)?;
    for (case, (p, width, height)) in cases.into_iter().enumerate() {
        crate::benchmarks::check_cancel(cancel)?;
        let shader_paint = render(native.as_mut(), &programs, p, false, width, height, false, 1.)?;
        let instanced = render(native.as_mut(), &programs, p, true, width, height, false, 1.)?;
        shader_paint.save(output.join(format!("sea-{case}-shader-paint.png")))?;
        instanced.save(output.join(format!("sea-{case}-instanced.png")))?;
        comparisons.push(compare(&shader_paint, &instanced, &format!("native shader paint/instanced case {case}"))?);
        if case == 0 {
            for x in [width / 4, width / 2, width * 3 / 4] {
                let expected = sky_reference(width, height, x, 1);
                for image in [&shader_paint, &instanced] {
                    let actual = image.get_pixel(x, 1);
                    ensure!(
                        actual.0[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3),
                        "analytic sky reference differs: {:?} vs {expected:?}",
                        actual.0
                    );
                }
            }
            ensure!(instanced.pixels().all(|p| p.0[3] == 255), "opaque seascape expected");
            ensure!(
                instanced.get_pixel(width / 2, height / 8).0 != instanced.get_pixel(width / 2, height * 7 / 8).0,
                "sky and sea must differ"
            );
            first = Some(instanced.clone());
            first_shader_paint = Some(shader_paint.clone());
        } else if case == 1 {
            let baseline = render(native.as_mut(), &programs, SeaInstance::default(), true, width, height, false, 1.)?;
            let changed = baseline.pixels().zip(instanced.pixels()).filter(|(a, b)| a.0 != b.0).count();
            ensure!(changed > 100, "time must change sea pixels at the same resolution");
        }
        for typed in [false, true] {
            let masked = render(native.as_mut(), &programs, p, typed, width, height, true, 0.5)?;
            let original = if typed { &instanced } else { &shader_paint };
            for y in 0..height {
                for x in 0..width {
                    let inside = x >= width / 4 && x < width * 3 / 4 && y >= height / 4 && y < height * 3 / 4;
                    let expected = original.get_pixel(x, y).0[..3]
                        .iter()
                        .map(|v| if inside { (*v as f32 * 0.5).round() as u8 } else { 0 })
                        .collect::<Vec<_>>();
                    ensure!(
                        masked.get_pixel(x, y).0[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3),
                        "clip/opacity mismatch at {x},{y}"
                    );
                }
            }
            #[cfg(feature = "grid-wgpu")]
            {
                let wgpu = render(&mut portable, &programs, p, typed, width, height, false, 1.)?;
                wgpu.save(output.join(format!("sea-{case}-wgpu-{}.png", if typed { "instanced" } else { "shader-paint" })))?;
                comparisons.push(compare(
                    original,
                    &wgpu,
                    &format!("native/wgpu {} case {case}", if typed { "instanced" } else { "shader-paint" }),
                )?);
            }
        }
    }
    // Retained old parameters remain immutable after other time/shape renders.
    let replay = render(native.as_mut(), &programs, SeaInstance::default(), true, 320, 180, false, 1.)?;
    ensure!(replay.as_raw() == first.as_ref().unwrap().as_raw(), "same snapshot must render deterministically");
    let old_snapshot = native.render_scene_to_image(&retained_shader_paint, size(DevicePixels(320), DevicePixels(180)))?;
    ensure!(
        old_snapshot.as_raw() == first_shader_paint.as_ref().unwrap().as_raw(),
        "shader paint snapshot changed after parameter edits"
    );
    let mut timing = Vec::new();
    for typed in [false, true] {
        let mut samples = Vec::new();
        for frame in 0..12 {
            crate::benchmarks::check_cancel(cancel)?;
            let start = Instant::now();
            render(
                native.as_mut(),
                &programs,
                SeaInstance { time: frame as f32 / 30., ..Default::default() },
                typed,
                320,
                180,
                false,
                1.,
            )?;
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        samples.sort_by(f64::total_cmp);
        timing.push(serde_json::json!({"api":if typed{"instanced"}else{"shader_paint"}, "frames":12,"median_ms":samples[6],"max_ms":samples[11]}));
    }
    Ok(
        serde_json::json!({"checks":["public constructors", "32-byte typed ABI", "shader paints expanded/instanced shaders looped rendering", "analytic sky reference", "time updates", "shape and detail controls", "aspect ratio", "clip/opacity", "deterministic replay"],
        "native_wgpu_parity":if cfg!(feature="grid-wgpu"){"passed"}else{"not run; enable grid-wgpu"},
        "comparisons":comparisons,"similarity_limits":{"mean_absolute_rgb_error":1,"pixels_over_24_fraction":0.005},
        "render_readback_timings":timing,"measurement":"CPU wall time including scene creation, GPU wait/readback at 320x180; not interactive FPS or GPU-only time.","errors":[]}),
    )
}
