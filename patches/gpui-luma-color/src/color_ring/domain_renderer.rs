use std::sync::Arc;

use gpui::{Bounds, Hsla, Image, Pixels, Window, px, size};

use gpui_luma::controls::slider::{DomainTrackRenderer, SliderOrientation};

use crate::domain_renderer::LockedDomain;
use super::track_context::ColorRingTrackContext;
use super::types::ColorRingTrackDelegate;

pub struct ColorRingDomainRenderer {
    inner: LockedDomain<dyn ColorRingTrackDelegate, ColorRingTrackContext>,
}

impl ColorRingDomainRenderer {
    pub fn new(delegate: Arc<dyn ColorRingTrackDelegate>, context: ColorRingTrackContext) -> Self {
        Self { inner: LockedDomain::new(delegate, context) }
    }

    pub fn context(&self) -> ColorRingTrackContext {
        self.inner.context()
    }

    pub fn set_context(&self, context: ColorRingTrackContext) {
        self.inner.set_context(context);
    }

    pub fn set_delegate(&self, delegate: Arc<dyn ColorRingTrackDelegate>) {
        self.inner.set_delegate(delegate);
    }
}

impl DomainTrackRenderer for ColorRingDomainRenderer {
    fn paint(&self, bounds: Bounds<Pixels>, _orientation: SliderOrientation, reversed: bool, window: &mut Window) {
        self.inner.with_context_mut(|context| context.reversed = reversed);
        let context = self.inner.context();
        self.inner.read_delegate().paint_domain_track(&context, bounds, window);
    }

    fn get_color_at_position(&self, position: f32) -> Option<Hsla> {
        Some(self.inner.read_delegate().get_color_for_context(&self.inner.context(), position))
    }

    fn raster_image(
        &self,
        bounds: Bounds<Pixels>,
        _orientation: SliderOrientation,
        reversed: bool,
    ) -> Option<Arc<Image>> {
        self.inner.with_context_mut(|context| context.reversed = reversed);
        let context = self.inner.context();
        let image_size = if bounds.size.width > px(0.0) && bounds.size.height > px(0.0) {
            bounds.size
        } else {
            let side = context.size_px().max(1.0);
            size(px(side), px(side))
        };
        self.inner.read_delegate().raster_cached_image(&context, image_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::hsla;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    struct StubDelegate;

    impl ColorRingTrackDelegate for StubDelegate {
        fn paint_domain_track(&self, _context: &ColorRingTrackContext, _bounds: Bounds<Pixels>, _window: &mut Window) {}

        fn get_color_for_context(&self, _context: &ColorRingTrackContext, _position: f32) -> Hsla {
            hsla(0.5, 0.5, 0.5, 1.0)
        }
    }

    #[test]
    fn poisoned_locks_do_not_abort_color_lookup() {
        let renderer = ColorRingDomainRenderer::new(Arc::new(StubDelegate), ColorRingTrackContext::default());
        let expected = renderer.get_color_at_position(0.75);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = renderer.inner.context_lock().write().unwrap();
            panic!("poison");
        }));
        assert_eq!(renderer.get_color_at_position(0.75), expected);
    }
}
