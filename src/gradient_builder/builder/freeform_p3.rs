//! Preserve the CPU freeform field and reinterpret its encoded colors as P3.
use super::*;
use gpui::{AnyElement, IntoElement, div, prelude::*};
#[derive(Default)]
pub(super) struct Cache {
    #[cfg(target_os = "macos")]
    cached: Option<(Arc<RenderImage>, Arc<super::mesh_p3::Paint>)>,
    #[cfg(target_os = "macos")]
    pipeline: Arc<super::mesh_p3::Pipeline>,
}
fn straight_bgra(image: &RenderImage) -> anyhow::Result<Vec<u8>> {
    let mut bytes = image.as_bytes(0).ok_or_else(|| anyhow::anyhow!("Freeform pixels unavailable"))?.to_vec();
    for pixel in bytes.chunks_exact_mut(4) {
        let alpha = pixel[3] as u32;
        for channel in &mut pixel[..3] {
            *channel = if alpha == 0 { 0 } else { ((*channel as u32 * 255 + alpha / 2) / alpha).min(255) as u8 };
        }
    }
    Ok(bytes)
}
#[cfg(target_os = "macos")]
mod metal {
    use super::*;
    use anyhow::Context as _;
    use gpui::{CustomContent, DevicePixels};
    use gpui_apple::{custom_draw::{MetalCustomContext, MetalCustomOutput}, metal::*};
    #[derive(Debug)]
    pub(super) struct Source { pub size: Size<DevicePixels>, pub bytes: Vec<u8> }
    impl CustomContent for Source {
        fn prepare(&self, backend: &dyn std::any::Any, _: Size<DevicePixels>) -> anyhow::Result<Box<dyn std::any::Any>> {
            let ctx = backend.downcast_ref::<MetalCustomContext>().context("P3 Freeform requires native Metal")?;
            let descriptor = TextureDescriptor::new();
            unsafe { descriptor.setWidth(self.size.width.0 as usize); descriptor.setHeight(self.size.height.0 as usize); }
            descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
            descriptor.setUsage(MTLTextureUsage::ShaderRead);
            descriptor.setStorageMode(MTLStorageMode::Shared);
            let texture = ctx.device.newTextureWithDescriptor(&descriptor).context("Freeform source allocation failed")?;
            // Validated RenderImage dimensions and tightly packed BGRA bytes.
            unsafe {
                texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                    MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: self.size.width.0 as usize, height: self.size.height.0 as usize, depth: 1 } },
                    0, std::ptr::NonNull::new(self.bytes.as_ptr() as *mut std::ffi::c_void).context("Empty freeform source")?, self.size.width.0 as usize * 4,
                );
            }
            Ok(Box::new(MetalCustomOutput { texture, commands: Vec::new() }))
        }
    }
}
impl Cache {
    pub(super) fn element(&mut self, image: Arc<RenderImage>) -> AnyElement {
        #[cfg(target_os = "macos")]
        {
            use gpui::{Corners, CustomGeometry, canvas};
            if self.cached.as_ref().is_none_or(|(old, _)| !Arc::ptr_eq(old, &image)) {
                let bytes = match straight_bgra(&image) {
                    Ok(bytes) => bytes,
                    Err(error) => return div().p_3().child(error.to_string()).into_any_element(),
                };
                let source = Arc::new(metal::Source { size: image.size(0), bytes });
                self.cached = Some((image, Arc::new(super::mesh_p3::Paint::new(source, self.pipeline.clone()))));
            }
            let paint = self.cached.as_ref().unwrap().1.clone();
            return canvas(|_, _, _| (), move |bounds, _, window, _| {
                window.paint_custom(CustomGeometry::RoundedRect { bounds, corner_radii: Corners::default() }, paint.clone());
            }).size_full().into_any_element();
        }
        #[cfg(not(target_os = "macos"))]
        { let _ = image; div().child("P3 Freeform requires native Metal").into_any_element() }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upload_unassociates_alpha_and_preserves_bgra_order() {
        let image = RenderImage::new(smallvec::smallvec![image::Frame::new(image::RgbaImage::from_raw(3, 1, vec![32, 64, 96, 128, 0, 0, 0, 0, 10, 20, 30, 255]).unwrap())]);
        assert_eq!(straight_bgra(&image).unwrap(), vec![64, 128, 191, 128, 0, 0, 0, 0, 10, 20, 30, 255]);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod native_tests {
    use super::*;
    use gpui::CustomContent;
    use gpui_apple::{custom_draw::{MetalCustomContext, MetalWideSdrOutput}, metal::*};
    #[test]
    #[ignore = "requires a native Metal device"]
    fn native_freeform_p3_upload_and_scaled_preparation() {
        objc2::rc::autoreleasepool(|_| {
            let device = MTLCreateSystemDefaultDevice().expect("Metal device");
            let queue = device.newCommandQueue().expect("Metal queue");
            let ctx = MetalCustomContext { device, queue };
            let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(image::RgbaImage::from_raw(2, 1, vec![0, 0, 255, 255, 32, 64, 96, 128]).unwrap())]));
            let mut cache = Cache::default();
            let _ = cache.element(image.clone());
            let first = cache.cached.as_ref().unwrap().1.clone();
            let _ = cache.element(image);
            assert!(Arc::ptr_eq(&first, &cache.cached.as_ref().unwrap().1));
            let size = gpui::size(gpui::DevicePixels(64), gpui::DevicePixels(32));
            assert!(first.prepare(&ctx, size).unwrap().is::<MetalWideSdrOutput>());
            assert!(first.prepare(&ctx, size).unwrap().is::<MetalWideSdrOutput>());
        });
    }
}

#[cfg(all(test, feature = "test-support"))]
mod control_tests {
    use super::*;
    #[test]
    fn freeform_p3_resets_adjustments_and_preserves_field_transforms() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.handle_top_tabs_event(&TabsEvent::Activate { tab_id: "freeform".into(), label: "Freeform".into() }, cx);
            view.point_transforms[0] = [70., 0.5];
            view.image_adjustments.saturation = 50.;
            view.image_adjustments.vibrance = -30.;
            view.saturation_slider.update(cx, |slider, cx| slider.set_value(50., cx));
            view.vibrance_slider.update(cx, |slider, cx| slider.set_value(-30., cx));
            view.freeform_p3 = true;
            view.sync_adjustment_support(cx);
            assert!(!view.adjustments_supported());
            assert!(view.image_adjustments.is_neutral());
            assert_eq!(view.saturation_slider.read(cx).value(), 0.);
            assert_eq!(view.vibrance_slider.read(cx).value(), 0.);
            assert_eq!(view.point_transforms[0], [70., 0.5]);
            view.freeform_p3 = false;
            view.sync_adjustment_support(cx);
            assert!(view.adjustments_supported());
        });
    }
}
