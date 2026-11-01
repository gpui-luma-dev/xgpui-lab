//! Profile-prepared extended-linear P3 photo fixtures and CPU preview checks.
#[cfg(test)]
use xgpui_color::{GamutMapping, LinearRgb, RgbSpace};
#[cfg(test)]
const WIDTH: u32 = 600;
#[cfg(test)]
const HEIGHT: u32 = 400;
pub(super) const PIXELS: [&[u8]; 3] = [
    include_bytes!("../../assets/color-labs/italy/Italy-sRGB.rgba16f"),
    include_bytes!("../../assets/color-labs/italy/Italy-P3.rgba16f"),
    include_bytes!("../../assets/color-labs/italy/Italy-oog.rgba16f"),
];
// Fixtures contain extended linear P3 samples. Preserve them until the media
// pipeline explicitly maps to bounded P3 storage.
pub(super) fn source_color() -> xgpui_media::pipeline::PixelColor {
    xgpui_media::pipeline::PixelColor::ExtendedLinear {
        primaries: xgpui_media::pipeline::ImagePrimaries::DisplayP3,
        alpha: xgpui_media::pipeline::AlphaMode::Straight,
    }
}

pub(super) fn pixels(selection: usize) -> anyhow::Result<Vec<[f32; 4]>> {
    let bytes = PIXELS.get(selection).ok_or_else(|| anyhow::anyhow!("Unknown Italy version"))?;
    anyhow::ensure!(bytes.len() == 600 * 400 * 8, "Invalid Italy fixture dimensions");
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|pixel| {
            let rgba: [f32; 4] =
                std::array::from_fn(|i| half(u16::from_le_bytes([pixel[i * 2], pixel[i * 2 + 1]])) as f32);
            anyhow::ensure!(rgba.iter().all(|v| v.is_finite()) && rgba[3] == 1., "Invalid opaque Italy fixture pixel");
            Ok(rgba)
        })
        .collect()
}

pub(super) fn half(bits: u16) -> f64 {
    let sign = if bits & 0x8000 == 0 { 1. } else { -1. };
    let exponent = (bits >> 10) & 31;
    let fraction = f64::from(bits & 1023);
    sign * match exponent {
        0 => fraction * 2f64.powi(-24),
        31 => f64::NAN,
        _ => (1. + fraction / 1024.) * 2f64.powi(i32::from(exponent) - 15),
    }
}
#[cfg(test)]
fn preview_rgba(selection: usize) -> anyhow::Result<Vec<u8>> {
    let bytes = PIXELS.get(selection).ok_or_else(|| anyhow::anyhow!("Unknown Italy version"))?;
    anyhow::ensure!(bytes.len() == WIDTH as usize * HEIGHT as usize * 8, "Invalid photo dimensions");
    let mut rgba = Vec::with_capacity(WIDTH as usize * HEIGHT as usize * 4);
    for pixel in bytes.as_chunks::<8>().0 {
        let values: [f64; 4] = std::array::from_fn(|i| half(u16::from_le_bytes([pixel[i * 2], pixel[i * 2 + 1]])));
        anyhow::ensure!(values.iter().all(|v| v.is_finite()) && values[3] == 1., "Invalid opaque fixture pixel");
        // Match the desktop fixture's bounded P3 storage, then its explicit
        // clipped sRGB preview. Original/browser mode never uses this derivative.
        let rgb = LinearRgb::new(RgbSpace::DisplayP3, [values[0], values[1], values[2]].map(|v| v.clamp(0., 1.)))?
            .map_gamut(RgbSpace::Srgb, GamutMapping::Clip)?
            .encode()?
            .components();
        rgba.extend(rgb.map(|v| (v.clamp(0., 1.) * 255.).round() as u8));
        rgba.push(255);
    }
    Ok(rgba)
}
#[cfg(test)]
fn preview_png(selection: usize) -> anyhow::Result<Vec<u8>> {
    let rgba = preview_rgba(selection)?;
    let mut output = std::io::Cursor::new(Vec::new());
    image::write_buffer_with_format(
        &mut output,
        &rgba,
        WIDTH,
        HEIGHT,
        image::ExtendedColorType::Rgba8,
        image::ImageFormat::Png,
    )?;
    Ok(output.into_inner())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn highlighted_fixture_requires_extended_metadata_and_all_three_images_prepare() -> anyhow::Result<()> {
        use xgpui_media::pipeline::{self, ImageColor, ImagePrimaries};
        let highlighted = pixels(2)?;
        assert!(highlighted.iter().any(|p| p[..3].iter().any(|v| !(0. ..=1.).contains(v))));
        let error = pipeline::rgba16f_data(600, 400, highlighted, ImageColor::LINEAR_P3).unwrap_err();
        assert!(error.to_string().contains("invalid bounded SDR pixel"));
        for selection in 0..3 {
            let image = pipeline::pixels_rgba16f_data(
                600,
                400,
                pixels(selection)?,
                source_color(),
                ImagePrimaries::DisplayP3,
                GamutMapping::Clip,
            )?;
            assert_eq!((image.width(), image.height()), (600, 400));
        }
        Ok(())
    }

    #[test]
    fn full_photo_previews_preserve_dimensions_and_show_orange_clipping() -> anyhow::Result<()> {
        let mut samples = Vec::new();
        for selection in 0..3 {
            let png = preview_png(selection)?;
            let decoded = image::load_from_memory_with_format(&png, image::ImageFormat::Png)?.into_rgba8();
            assert_eq!(decoded.dimensions(), (600, 400));
            assert!(decoded.pixels().all(|pixel| pixel[3] == 255));
            samples.push(decoded.get_pixel(515, 220).0);
        }
        // Independently recorded orange-umbrella source values: P3's red is
        // stronger, and negative sRGB green/blue clip to zero in the preview.
        assert!(samples[1][0] > samples[0][0] + 10, "{samples:?}");
        assert_eq!(&samples[1][1..3], &[0, 0]);
        assert!(preview_png(3).is_err());
        Ok(())
    }
}
