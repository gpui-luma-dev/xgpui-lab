//! Native PaintSurface checks, deliberately independent of CustomDraw video.
use anyhow::{Context as _, Result, ensure};
use gpui::*;
use objc2_core_foundation::{CFDictionary, CFRetained, CFType};
use objc2_core_video::*;
use std::ptr::{self, NonNull};

pub(super) fn buffer(nv12: bool, reverse: bool) -> Result<CFRetained<CVPixelBuffer>> {
    let attrs = CFDictionary::from_slices(
        &[unsafe { kCVPixelBufferIOSurfacePropertiesKey }],
        &[&*CFDictionary::<CFType, CFType>::empty()],
    );
    let mut raw = ptr::null_mut();
    // SAFETY: CoreVideo writes one retained buffer to a valid output pointer.
    let status = unsafe {
        CVPixelBufferCreate(
            None,
            32,
            16,
            if nv12 {
                kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
            } else {
                kCVPixelFormatType_32BGRA
            },
            Some(attrs.as_opaque()),
            NonNull::from(&mut raw),
        )
    };
    ensure!(status == kCVReturnSuccess, "Surface buffer allocation: {status}");
    let buffer = unsafe { CFRetained::from_raw(NonNull::new(raw).context("null Surface buffer")?) };
    // SAFETY: writes stay within the locked buffer's reported row strides and extents.
    unsafe {
        let status = CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags::empty());
        ensure!(status == kCVReturnSuccess, "Surface lock: {status}");
        if nv12 {
            let stride = CVPixelBufferGetBytesPerRowOfPlane(&buffer, 0);
            let bytes = std::slice::from_raw_parts_mut(
                CVPixelBufferGetBaseAddressOfPlane(&buffer, 0).cast::<u8>(),
                stride * 16,
            );
            for y in 0..16 {
                for x in 0..32 {
                    bytes[y * stride + x] = if (x < 16) ^ reverse { 96 } else { 160 };
                }
            }
            let stride = CVPixelBufferGetBytesPerRowOfPlane(&buffer, 1);
            let bytes =
                std::slice::from_raw_parts_mut(CVPixelBufferGetBaseAddressOfPlane(&buffer, 1).cast::<u8>(), stride * 8);
            for y in 0..8 {
                for x in 0..16 {
                    let uv = if (x < 8) ^ reverse { [96, 192] } else { [192, 96] };
                    bytes[y * stride + x * 2..y * stride + x * 2 + 2].copy_from_slice(&uv);
                }
            }
        } else {
            let stride = CVPixelBufferGetBytesPerRow(&buffer);
            let bytes = std::slice::from_raw_parts_mut(CVPixelBufferGetBaseAddress(&buffer).cast::<u8>(), stride * 16);
            for y in 0..16 {
                for x in 0..32 {
                    let bgra = if (x < 16) ^ reverse {
                        [32, 64, 224, 255]
                    } else {
                        [224, 192, 32, 255]
                    };
                    bytes[y * stride + x * 4..y * stride + x * 4 + 4].copy_from_slice(&bgra);
                }
            }
        }
        ensure!(
            CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags::empty()) == kCVReturnSuccess,
            "Surface unlock failed"
        );
    }
    Ok(buffer)
}

pub(super) fn run(output: &std::path::Path) -> Result<()> {
    let mut renderer = gpui_platform::current_headless_renderer()?.context("native Surface renderer unavailable")?;
    for nv12 in [false, true] {
        for reverse in [false, true] {
            for width in [32., 64.] {
                let buffer = buffer(nv12, reverse)?;
                let bounds = super::atlas::rect(4., 4., width, 16.);
                let mask = super::atlas::rect(8., 4., width - 4., 16.);
                let mut scene = Scene::default();
                let full = super::atlas::rect(0., 0., 80., 32.);
                scene.insert_primitive(Quad {
                    bounds: full,
                    content_mask: ContentMask { bounds: full },
                    background: black().into(),
                    ..Default::default()
                });
                scene.insert_primitive(PaintSurface {
                    order: 0,
                    bounds,
                    content_mask: ContentMask { bounds: mask },
                    image_buffer: buffer,
                });
                scene.finish();
                let extent = size(DevicePixels(80), DevicePixels(32));
                let pixels = renderer.render_scene_to_image(&scene, extent)?;
                for (fraction, left) in [(0.25, true), (0.75, false)] {
                    let x = (4. + width * fraction) as u32;
                    let first = left ^ reverse;
                    let expected = if nv12 {
                        if first { [186, 61, 39] } else { [115, 161, 255] }
                    } else if first {
                        [224, 64, 32]
                    } else {
                        [32, 192, 224]
                    };
                    let actual = pixels.get_pixel(x, 12);
                    ensure!(
                        actual.0[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                        "Surface nv12={nv12} reverse={reverse} width={width} at {x}: {actual:?}, expected {expected:?}"
                    );
                }
                for (x, y) in [(5, 12), (2, 12), (12, 24)] {
                    ensure!(pixels.get_pixel(x, y).0 == [0, 0, 0, 255], "Surface clip/bounds leaked at {x},{y}");
                }
                let mut replay = Scene::default();
                replay.replay(0..scene.len(), &scene);
                replay.finish();
                drop(scene); // Replay must own the pixel buffer independently.
                ensure!(renderer.render_scene_to_image(&replay, extent)? == pixels, "Surface replay changed pixels");
                pixels.save(
                    output.join(format!("surface-{}-{reverse}-{width}.png", if nv12 { "nv12" } else { "bgra" })),
                )?;
            }
        }
    }
    Ok(())
}
