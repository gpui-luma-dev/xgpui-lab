use std::sync::{Arc, RwLock};

use gpui::{Bounds, Hsla, Pixels, Window};

use gpui_luma::controls::slider::{DomainTrackRenderer, SliderOrientation};
use gpui_luma::infra::lock;

use crate::domain_renderer::LockedDomain;
use super::template::ColorSliderTemplateConfig;
use super::track_context::{axis_from_orientation, ColorSliderTrackContext};
use super::types::ColorSliderDelegate;

pub struct ColorSliderDomainRenderer {
    inner: LockedDomain<dyn ColorSliderDelegate, ColorSliderTrackContext>,
    template_config: Arc<RwLock<ColorSliderTemplateConfig>>,
}

impl ColorSliderDomainRenderer {
    pub fn new(
        delegate: Arc<dyn ColorSliderDelegate>,
        context: ColorSliderTrackContext,
        template_config: Arc<RwLock<ColorSliderTemplateConfig>>,
    ) -> Self {
        Self { inner: LockedDomain::new(delegate, context), template_config }
    }

    pub fn context(&self) -> ColorSliderTrackContext {
        self.inner.context()
    }

    pub fn set_context(&self, context: ColorSliderTrackContext) {
        self.inner.set_context(context);
    }

    pub fn set_delegate(&self, delegate: Arc<dyn ColorSliderDelegate>) {
        self.inner.set_delegate(delegate);
    }

    pub fn shared_context(&self) -> Arc<RwLock<ColorSliderTrackContext>> {
        self.inner.shared_context()
    }
}

impl DomainTrackRenderer for ColorSliderDomainRenderer {
    fn paint(&self, bounds: Bounds<Pixels>, orientation: SliderOrientation, reversed: bool, window: &mut Window) {
        self.inner.with_context_mut(|context| {
            context.axis = axis_from_orientation(orientation);
            context.reversed = reversed;
        });
        let mut paint_context = self.inner.context();
        paint_context.corner_radii = lock::read(&self.template_config).corner_radii.clone();
        self.inner.read_delegate().paint_domain_track(&paint_context, bounds, window);
    }

    fn get_color_at_position(&self, position: f32) -> Option<Hsla> {
        Some(self.inner.read_delegate().get_color_for_context(&self.inner.context(), position))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_slider::types::{Axis, ColorInterpolation};
    use gpui::hsla;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    struct StubDelegate;

    impl ColorSliderDelegate for StubDelegate {
        fn paint_domain_track(
            &self,
            _context: &ColorSliderTrackContext,
            _bounds: Bounds<Pixels>,
            _window: &mut Window,
        ) {
        }

        fn get_color_for_context(&self, _context: &ColorSliderTrackContext, _position: f32) -> Hsla {
            hsla(0.2, 0.4, 0.6, 1.0)
        }
    }

    fn sample_context() -> ColorSliderTrackContext {
        ColorSliderTrackContext {
            range: 0.0..1.0,
            reversed: false,
            axis: Axis::Horizontal,
            interpolation: ColorInterpolation::Rgb,
            corner_radii: Default::default(),
            theme_is_dark: false,
        }
    }

    fn sample_renderer() -> ColorSliderDomainRenderer {
        ColorSliderDomainRenderer::new(
            Arc::new(StubDelegate),
            sample_context(),
            Arc::new(RwLock::new(ColorSliderTemplateConfig::default())),
        )
    }

    #[test]
    fn poisoned_locks_do_not_abort_color_lookup() {
        let renderer = sample_renderer();
        let expected = renderer.get_color_at_position(0.5);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = renderer.inner.context_lock().write().unwrap();
            panic!("poison");
        }));
        assert_eq!(renderer.get_color_at_position(0.5), expected);
        assert_eq!(renderer.context().range.start, 0.0);
    }
}
