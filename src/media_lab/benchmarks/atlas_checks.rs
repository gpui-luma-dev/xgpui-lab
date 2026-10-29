//! Deterministic atlas uploads through the native sprite compositor.
use anyhow::{Result, ensure};
use gpui::*;
use std::borrow::Cow;

pub(super) fn verify(output: &std::path::Path) -> Result<()> {
    let mut renderer =
        gpui_platform::current_headless_renderer()?.ok_or_else(|| anyhow::anyhow!("no native renderer"))?;
    let extent = size(DevicePixels(32), DevicePixels(16));
    let tile_size = size(DevicePixels(7), DevicePixels(5)); // Tight, odd-width rows in both formats.
    let mono_key = AtlasKey::Glyph(RenderGlyphParams {
        font_id: FontId(0),
        glyph_id: GlyphId(u32::MAX),
        font_size: px(5.),
        subpixel_variant: point(0, 0),
        scale_factor: 1.,
        is_emoji: false,
        subpixel_rendering: false,
        dilation: 0,
    });
    let image = RenderImage::new(vec![image::Frame::new(image::RgbaImage::new(7, 5))]);
    let color_key = AtlasKey::Image(RenderImageParams { image_id: image.id, frame_index: 0 });
    for generation in 0..2 {
        let mut bgra = Vec::new();
        let mut mask = Vec::new();
        for y in 0..5 {
            for x in 0..7 {
                let value = if (x + y + generation) % 2 == 0 { 255 } else { 0 };
                mask.push(value);
                bgra.extend_from_slice(&[17 + x * 20, 31 + y * 30, 210 - generation * 80, 255]);
            }
        }
        let atlas = renderer.sprite_atlas();
        let mono = atlas
            .get_or_insert_with(mono_key.clone(), &mut || Ok(Some((tile_size, Cow::Borrowed(&mask)))))?
            .unwrap();
        let color = atlas
            .get_or_insert_with(color_key.clone(), &mut || Ok(Some((tile_size, Cow::Borrowed(&bgra)))))?
            .unwrap();
        for key in [&mono_key, &color_key] {
            atlas.get_or_insert_with(key.clone(), &mut || anyhow::bail!("atlas cache hit rebuilt its source"))?;
        }
        let mut scene = Scene::default();
        let background = super::atlas::rect(0., 0., 32., 16.);
        scene.insert_primitive(Quad {
            bounds: background,
            content_mask: ContentMask { bounds: background },
            background: black().into(),
            ..Default::default()
        });
        let bounds = super::atlas::rect(2., 2., 7., 5.);
        scene.insert_primitive(MonochromeSprite {
            order: 0,
            pad: 0,
            bounds,
            content_mask: ContentMask { bounds },
            color: white(),
            tile: mono,
            transformation: TransformationMatrix::unit(),
        });
        let bounds = super::atlas::rect(16., 2., 7., 5.);
        scene.insert_primitive(PolychromeSprite {
            order: 0,
            pad: 0,
            bounds,
            content_mask: ContentMask { bounds },
            grayscale: false.into(),
            opacity: 1.,
            corner_radii: Default::default(),
            tile: color,
        });
        scene.finish();
        let pixels = renderer.render_scene_to_image(&scene, extent)?;
        pixels.save(output.join(format!("atlas-pixels-{generation}.png")))?;
        for y in 0..5u32 {
            for x in 0..7u32 {
                let offset = (y * 7 + x) as usize;
                let expected = [mask[offset]; 3];
                let actual = pixels.get_pixel(x + 2, y + 2);
                ensure!(
                    actual.0[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 1),
                    "monochrome atlas generation {generation} at {x},{y}: {actual:?}"
                );
                let expected = [bgra[offset * 4 + 2], bgra[offset * 4 + 1], bgra[offset * 4], 255];
                let actual = pixels.get_pixel(x + 16, y + 2);
                ensure!(
                    actual.0.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 1),
                    "BGRA atlas generation {generation} at {x},{y}: {actual:?}, expected {expected:?}"
                );
            }
        }
        let mut replay = Scene::default();
        replay.replay(0..scene.len(), &scene);
        replay.finish();
        ensure!(renderer.render_scene_to_image(&replay, extent)? == pixels, "atlas replay changed pixels");
        renderer.sprite_atlas().remove(&mono_key);
        renderer.sprite_atlas().remove(&color_key);
        let stale = renderer.render_scene_to_image(&replay, extent)?;
        ensure!(
            stale.pixels().all(|p| p.0 == [0, 0, 0, 255]),
            "removed atlas textures leaked through stale scene replay"
        );
    }
    Ok(())
}
