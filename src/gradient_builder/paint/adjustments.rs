use std::sync::Arc;
use gpui::{Hsla, RenderImage, Rgba};

#[derive(Clone, Copy, Default)]
pub struct ImageAdjustments {
    pub saturation: f32,
    pub vibrance: f32,
}

impl ImageAdjustments {
    pub fn is_neutral(self) -> bool {
        self.saturation == 0.0 && self.vibrance == 0.0
    }

    fn adjust(self, color: Rgba) -> Rgba {
        let mut hsl = Hsla::from(color);
        // Vibrance preferentially affects muted colors; neutral gray stays gray.
        hsl.s = (hsl.s + self.vibrance.clamp(-100.0, 100.0) / 100.0 * hsl.s * (1.0 - hsl.s)).clamp(0.0, 1.0);
        hsl.s = (hsl.s * (1.0 + self.saturation.clamp(-100.0, 100.0) / 100.0)).clamp(0.0, 1.0);
        hsl.to_rgb()
    }

    pub fn apply(self, source: Arc<RenderImage>) -> Option<Arc<RenderImage>> {
        if self.is_neutral() {
            return Some(source);
        }
        let size = source.size(0);
        let mut bytes = source.as_bytes(0)?.to_vec();
        // GPUI raster images use premultiplied BGRA. Unpremultiply before editing.
        for pixel in bytes.as_chunks_mut::<4>().0 {
            let alpha = pixel[3] as f32 / 255.0;
            if alpha == 0.0 {
                continue;
            }
            let color = self.adjust(Rgba {
                r: (pixel[2] as f32 / 255.0 / alpha).clamp(0.0, 1.0),
                g: (pixel[1] as f32 / 255.0 / alpha).clamp(0.0, 1.0),
                b: (pixel[0] as f32 / 255.0 / alpha).clamp(0.0, 1.0),
                a: alpha,
            });
            pixel[0] = (color.b * alpha * 255.0).round() as u8;
            pixel[1] = (color.g * alpha * 255.0).round() as u8;
            pixel[2] = (color.r * alpha * 255.0).round() as u8;
        }
        let buffer = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
            size.width.0.try_into().ok()?,
            size.height.0.try_into().ok()?,
            bytes,
        )?;
        Some(Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(buffer)])))
    }
}

/// Save a GPUI premultiplied BGRA preview as a straight-alpha RGBA PNG.
pub fn save_preview_png(source: &RenderImage, path: &std::path::Path) -> anyhow::Result<()> {
    let size = source.size(0);
    let mut bytes = source.as_bytes(0).ok_or_else(|| anyhow::anyhow!("missing preview pixels"))?.to_vec();
    for pixel in bytes.as_chunks_mut::<4>().0 {
        let alpha = pixel[3] as f32 / 255.0;
        pixel.swap(0, 2);
        for channel in &mut pixel[..3] {
            *channel = if alpha > 0.0 {
                (*channel as f32 / alpha).round().clamp(0.0, 255.0) as u8
            } else {
                0
            };
        }
    }
    let buffer =
        image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(size.width.0.try_into()?, size.height.0.try_into()?, bytes)
            .ok_or_else(|| anyhow::anyhow!("invalid preview dimensions"))?;
    buffer.save_with_format(path, image::ImageFormat::Png)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_export_writes_straight_rgba_pixels() {
        let buffer = image::ImageBuffer::from_raw(1, 1, vec![0, 0, 128, 128]).unwrap();
        let source = RenderImage::new(smallvec::smallvec![image::Frame::new(buffer)]);
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("color-viz-export-{}-{nonce}.png", std::process::id()));
        save_preview_png(&source, &path).unwrap();
        let output = image::open(&path).unwrap().into_rgba8();
        std::fs::remove_file(path).unwrap();
        assert_eq!(output.dimensions(), (1, 1));
        assert_eq!(output.get_pixel(0, 0).0, [255, 0, 0, 128]);
    }

    #[test]
    fn saturation_zeroes_chroma_and_preserves_alpha() {
        let color =
            ImageAdjustments { saturation: -100.0, vibrance: 100.0 }.adjust(gpui::hsla(0.2, 0.8, 0.4, 0.5).to_rgb());
        assert!((color.r - color.g).abs() < 0.00001 && (color.g - color.b).abs() < 0.00001);
        assert_eq!(color.a, 0.5);
    }

    #[test]
    fn vibrance_boosts_muted_colors_and_preserves_saturated_colors() {
        let settings = ImageAdjustments { saturation: 0.0, vibrance: 100.0 };
        let muted = gpui::hsla(0.6, 0.4, 0.5, 1.0);
        assert!(Hsla::from(settings.adjust(muted.to_rgb())).s > muted.s);
        let saturated = gpui::hsla(0.6, 1.0, 0.5, 1.0).to_rgb();
        let output = settings.adjust(saturated);
        assert!((output.r - saturated.r).abs() < 0.00001);
        assert!((output.g - saturated.g).abs() < 0.00001);
        assert!((output.b - saturated.b).abs() < 0.00001);
    }

    #[test]
    fn postprocess_handles_bgra_transparency_and_neutral_identity() {
        let buffer = image::ImageBuffer::from_raw(1, 1, vec![0, 0, 128, 128]).unwrap();
        let source = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(buffer)]));
        assert!(Arc::ptr_eq(&source, &ImageAdjustments::default().apply(source.clone()).unwrap()));
        let output = ImageAdjustments { saturation: -100.0, vibrance: 0.0 }.apply(source).unwrap();
        assert_eq!(output.as_bytes(0).unwrap(), &[64, 64, 64, 128]);
    }
}
