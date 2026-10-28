use super::*;
use xgpui_shader::experimental::ShaderPaint;

#[derive(Debug)]
struct LogicalSource(ShaderPaint);
impl CustomContent for LogicalSource {
    fn prepare(&self, backend: &dyn std::any::Any, extent: Size<DevicePixels>) -> Result<Box<dyn std::any::Any>> {
        self.0
            .prepare_with_request(backend, &PreparationRequest::new(size(px(160.), px(80.)), extent, 0.5)?)
    }
}
fn scene(content: Arc<dyn CustomContent>, clipped: bool, opacity: f32) -> Scene {
    let mut scene = base();
    let mut draw = record(rect(10., 20., 80., 40.), content, None);
    draw.corner_radii = Corners::default();
    draw.opacity = opacity;
    if clipped {
        draw.content_mask.bounds = rect(68., 35., 10., 10.);
    }
    scene.insert_primitive(draw);
    scene.finish();
    scene
}
fn check(pixel: [u8; 4], expected: [u8; 4]) -> Result<()> {
    ensure!(
        pixel.into_iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
        "portable pixel {pixel:?}, expected {expected:?}"
    );
    Ok(())
}
pub(super) fn verify(renderer: &mut dyn PlatformHeadlessRenderer) -> Result<()> {
    let program = super::portable::program()?;
    let paint = super::portable::paint(&program, 0.25, 0.5)?;
    let extent = size(DevicePixels(128), DevicePixels(80));
    let original = scene(Arc::new(paint.clone()), false, 1.);
    let image = renderer.render_scene_to_image(&original, extent)?;
    // At local pixel (60.5,19.5), blend clamps to 1. The explicit
    // logical size gives accent=(.1,19.5/160,80/400), then alpha=.5.
    check(image.get_pixel(70, 39).0, [13, 16, 26, 255])?;
    let clipped = scene(Arc::new(paint.clone()), true, 0.5);
    let image = renderer.render_scene_to_image(&clipped, extent)?;
    check(image.get_pixel(70, 39).0, [6, 8, 13, 255])?;
    check(image.get_pixel(30, 25).0, [0, 0, 0, 255])?;
    // The same paint and raster extent with different logical geometry must
    // produce different cached output. No app-side OS/backend branching.
    let logical = scene(Arc::new(LogicalSource(paint.clone())), false, 1.);
    let image = renderer.render_scene_to_image(&logical, extent)?;
    check(image.get_pixel(70, 39).0, [13, 31, 51, 255])?;
    let image = renderer.render_scene_to_image(&original, extent)?;
    check(image.get_pixel(70, 39).0, [13, 16, 26, 255])?;
    let next = super::portable::paint(&program, 1., 1.)?;
    let image = renderer.render_scene_to_image(&scene(Arc::new(next), false, 1.), extent)?;
    check(image.get_pixel(70, 39).0, [26, 31, 51, 255])?;
    // Prove the old snapshot still renders after parameter edits.
    let image = renderer.render_scene_to_image(&original, extent)?;
    check(image.get_pixel(70, 39).0, [13, 16, 26, 255])?;
    // Unused uniforms must remain legal: the backend owns explicit bindings,
    // even when a shader compiler optimizes every user parameter away.
    let constant = xgpui_shader::experimental::ShaderProgram::new_v1(
        "struct Parameters { ignored: f32 } fn paint(input: PaintInput) -> vec4<f32> { return vec4(0.25,0.5,0.75,1.); }",
    )?;
    let constant = constant.paint(xgpui_shader::experimental::ShaderParameters::new().float("ignored", 1.))?;
    let image = renderer.render_scene_to_image(&scene(Arc::new(constant), false, 1.), extent)?;
    check(image.get_pixel(70, 39).0, [64, 128, 191, 255])?;
    Ok(())
}
