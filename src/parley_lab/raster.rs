//! Composite Parley's grayscale glyph masks without mixing native and Parley font IDs.
use anyhow::{Result, ensure};
use gpui::{PlatformTextSystem, RenderGlyphParams, RenderImage, ShapedRun, Pixels, Size, point};
use std::sync::Arc;

pub fn render(
    backend: &dyn PlatformTextSystem,
    runs: &[ShapedRun],
    font_size: Pixels,
    size: Size<Pixels>,
    scale: f32,
    baseline: f32,
) -> Result<Arc<RenderImage>> {
    let width = (f32::from(size.width) * scale).ceil().max(1.) as u32;
    let height = (f32::from(size.height) * scale).ceil().max(1.) as u32;
    ensure!(width <= 4096 && height <= 8192, "preview exceeds raster budget");
    let mut pixels = image::RgbaImage::new(width, height);
    for run in runs {
        for glyph in &run.glyphs {
            let params = RenderGlyphParams {
                font_id: run.font_id,
                glyph_id: glyph.id,
                font_size,
                scale_factor: scale,
                subpixel_variant: point(0, 0),
                is_emoji: glyph.is_emoji,
                subpixel_rendering: false,
                dilation: 0,
            };
            let bounds = backend.glyph_raster_bounds(&params)?;
            if bounds.size.width.0 <= 0 || bounds.size.height.0 <= 0 {
                continue;
            }
            let (size, mask) = backend.rasterize_glyph(&params, bounds)?;
            let left = (f32::from(glyph.position.x) * scale).floor() as i32 + bounds.origin.x.0;
            let top = ((f32::from(glyph.position.y) + baseline) * scale).floor() as i32 + bounds.origin.y.0;
            for y in 0..size.height.0 {
                for x in 0..size.width.0 {
                    let dx = left + x;
                    let dy = top + y;
                    if dx < 0 || dy < 0 || dx >= width as i32 || dy >= height as i32 {
                        continue;
                    }
                    let coverage = mask[(y * size.width.0 + x) as usize];
                    let dest = pixels.get_pixel_mut(dx as u32, dy as u32);
                    dest[3] = (u16::from(coverage) + u16::from(dest[3]) * u16::from(255 - coverage) / 255) as u8;
                }
            }
        }
    }
    Ok(Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(pixels)])))
}
