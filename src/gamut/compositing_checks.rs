//! Small native wide-SDR scenes with an independent linear-light CPU oracle.
use anyhow::Result;
#[cfg(target_os = "macos")]
use anyhow::ensure;
#[cfg(target_os = "macos")]
use gpui::*;
use serde_json::{Value, json};

pub(super) fn verify(output: &std::path::Path) -> Result<Value> {
    #[cfg(target_os = "macos")]
    {
        verify_metal(output)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = output;
        Ok(json!({"outcome": "unavailable", "reason": "Apple native wide-SDR pixel checks require macOS Metal"}))
    }
}

#[cfg(target_os = "macos")]
fn verify_metal(output: &std::path::Path) -> Result<Value> {
    use std::sync::Arc;
    let mut renderer = gpui_apple::metal_renderer::MetalHeadlessRenderer::new();
    renderer.set_color_mode(WindowColorMode::WideGamutSdr)?;
    let extent = size(DevicePixels(40), DevicePixels(32));
    let mut cases = 0;
    for backdrop in [0., 1.] {
        for reverse in [false, true] {
            for (alpha, opacity) in [(0., 1.), (1., 1.), (0.5, 0.5)] {
                let mut scene = Scene::default();
                let full = rect(0., 0., 40., 32.);
                scene.insert_primitive(Quad {
                    bounds: full,
                    content_mask: ContentMask { bounds: full },
                    background: rgb(if backdrop == 0. { 0 } else { 0xffffff }).into(),
                    ..Default::default()
                });
                let order = if reverse { [1, 0] } else { [0, 1] };
                for index in order {
                    let bounds = if index == 0 {
                        rect(4., 4., 20., 16.)
                    } else {
                        rect(12., 8., 20., 16.)
                    };
                    let mask = if index == 0 { full } else { rect(14., 8., 16., 16.) };
                    let color = if index == 0 {
                        [1., 0., 0., alpha]
                    } else {
                        [0., 1., 0., alpha]
                    };
                    scene.insert_primitive(CustomDraw {
                        order: 0,
                        bounds,
                        content_mask: ContentMask { bounds: mask },
                        corner_radii: Default::default(),
                        opacity,
                        content: Arc::new(xgpui_gamut::WideColor::linear_p3(color)?),
                        coverage: None,
                    });
                }
                let overlay = rect(18., 12., 4., 4.);
                scene.insert_primitive(Quad {
                    bounds: overlay,
                    content_mask: ContentMask { bounds: overlay },
                    background: rgba(0xffffff80).into(),
                    ..Default::default()
                });
                scene.finish();
                let pixels = renderer.render_scene_to_image(&scene, extent)?;
                std::fs::create_dir_all(output)?;
                pixels.save(output.join(format!("compositing-{cases}.png")))?;
                for (x, y, pixel) in pixels.enumerate_pixels() {
                    let mut linear = [backdrop; 3];
                    for index in order {
                        let covered = if index == 0 {
                            (4..24).contains(&x) && (4..20).contains(&y)
                        } else {
                            (14..30).contains(&x) && (8..24).contains(&y)
                        };
                        if covered {
                            // Display-P3 linear primaries transformed into linear sRGB.
                            let source = if index == 0 {
                                [1.2249402, -0.04205695, -0.01963755]
                            } else {
                                [-0.2249402, 1.0420569, -0.07863605]
                            };
                            let a = alpha * opacity;
                            for i in 0..3 {
                                linear[i] = source[i] * a + linear[i] * (1. - a);
                            }
                        }
                    }
                    if (18..22).contains(&x) && (12..16).contains(&y) {
                        let a = 128. / 255.;
                        for value in &mut linear {
                            *value = a + *value * (1. - a);
                        }
                    }
                    let expected = linear.map(encode);
                    ensure!(
                        pixel.0[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3) && pixel[3] == 255,
                        "Gamut backdrop={backdrop} reverse={reverse} alpha={alpha} opacity={opacity} at {x},{y}: {pixel:?}, expected {expected:?}"
                    );
                }
                let mut replay = Scene::default();
                replay.replay(0..scene.len(), &scene);
                replay.finish();
                ensure!(renderer.render_scene_to_image(&replay, extent)? == pixels, "Gamut replay changed pixels");
                let resized = renderer.render_scene_to_image(&replay, size(DevicePixels(48), DevicePixels(40)))?;
                for y in 0..32 {
                    for x in 0..40 {
                        ensure!(
                            resized.get_pixel(x, y) == pixels.get_pixel(x, y),
                            "Gamut viewport resize changed pixels"
                        );
                    }
                }
                cases += 1;
            }
        }
    }
    Ok(
        json!({"outcome": "passed", "cases": cases, "checks": ["linear P3 source-over CPU reference", "black and white backdrops", "source alpha and inherited opacity", "reversed overlap", "content mask", "native sRGB overlay", "replay", "viewport resize"], "tolerance_u8": 3}),
    )
}

#[cfg(target_os = "macos")]
fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds::new(point(ScaledPixels(x), ScaledPixels(y)), size(ScaledPixels(w), ScaledPixels(h)))
}
#[cfg(target_os = "macos")]
fn encode(value: f32) -> u8 {
    let value = value.clamp(0., 1.);
    let encoded = if value <= 0.0031308 {
        12.92 * value
    } else {
        1.055 * value.powf(1. / 2.4) - 0.055
    };
    (encoded * 255.).round() as u8
}
