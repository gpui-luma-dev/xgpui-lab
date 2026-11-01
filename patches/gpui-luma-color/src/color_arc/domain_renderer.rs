use std::sync::{Arc, RwLock};

use gpui::{Bounds, Hsla, Image, Pixels, Window};

use gpui_luma::controls::slider::{DomainTrackRenderer, SliderOrientation};

use crate::domain_renderer::LockedDomain;
use super::track_context::ColorArcTrackContext;
use super::types::ColorArcDelegate;

pub struct ColorArcDomainRenderer {
    inner: LockedDomain<dyn ColorArcDelegate, ColorArcTrackContext>,
}

impl ColorArcDomainRenderer {
    pub fn new(delegate: Arc<dyn ColorArcDelegate>, context: ColorArcTrackContext) -> Self {
        Self { inner: LockedDomain::new(delegate, context) }
    }

    pub fn context(&self) -> ColorArcTrackContext {
        self.inner.context()
    }

    pub fn set_context(&self, context: ColorArcTrackContext) {
        self.inner.set_context(context);
    }

    pub fn set_delegate(&self, delegate: Arc<dyn ColorArcDelegate>) {
        self.inner.set_delegate(delegate);
    }

    pub fn shared_context(&self) -> Arc<RwLock<ColorArcTrackContext>> {
        self.inner.shared_context()
    }
}

impl DomainTrackRenderer for ColorArcDomainRenderer {
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
        let image_size = if bounds.size.width > gpui::px(0.0) && bounds.size.height > gpui::px(0.0) {
            bounds.size
        } else {
            let side = context.dial_size_px().max(1.0);
            gpui::size(gpui::px(side), gpui::px(side))
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

    impl ColorArcDelegate for StubDelegate {
        fn paint_domain_track(&self, _context: &ColorArcTrackContext, _bounds: Bounds<Pixels>, _window: &mut Window) {}

        fn get_color_for_context(&self, _context: &ColorArcTrackContext, _position: f32) -> Hsla {
            hsla(0.1, 0.2, 0.3, 1.0)
        }
    }

    #[test]
    fn poisoned_locks_do_not_abort_color_lookup() {
        let renderer = ColorArcDomainRenderer::new(Arc::new(StubDelegate), ColorArcTrackContext::default());
        let expected = renderer.get_color_at_position(0.25);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = renderer.inner.context_lock().write().unwrap();
            panic!("poison");
        }));
        assert_eq!(renderer.get_color_at_position(0.25), expected);
    }
}
