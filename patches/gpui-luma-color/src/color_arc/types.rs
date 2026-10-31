use std::sync::Arc;

use gpui::{Bounds, Hsla, Image, Pixels, Size, Window};

use super::track_context::ColorArcTrackContext;

pub trait ColorArcDelegate: Send + Sync + 'static {
    fn paint_domain_track(&self, context: &ColorArcTrackContext, bounds: Bounds<Pixels>, window: &mut Window);

    fn get_color_for_context(&self, context: &ColorArcTrackContext, position: f32) -> Hsla;

    fn raster_cached_image(&self, _context: &ColorArcTrackContext, _image_size: Size<Pixels>) -> Option<Arc<Image>> {
        None
    }
}
