use gpui::{Bounds, Pixels, Point, point, px};

/// Shared logical-pixel camera, relative to each panel's center.
#[derive(Clone, Copy)]
pub struct Camera {
    pub zoom: f32,
    pub pan: Point<Pixels>,
    pub scene_size: gpui::Size<f32>,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            scene_size: gpui::size(super::scene::WIDTH, super::scene::HEIGHT),
            zoom: 1.,
            pan: point(px(0.), px(0.)),
        }
    }
}
impl Camera {
    pub fn zoom_at(&mut self, anchor: Point<Pixels>, factor: f32) {
        let zoom = (self.zoom * factor).clamp(0.25, 16.);
        self.pan = anchor - (anchor - self.pan) * (zoom / self.zoom);
        self.zoom = zoom;
    }
    pub fn origin(self, bounds: Bounds<Pixels>) -> Point<Pixels> {
        bounds.center() + self.pan
            - point(px(self.scene_size.width * self.zoom / 2.), px(self.scene_size.height * self.zoom / 2.))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn zoom_keeps_pointer_on_same_scene_point_even_at_limits() {
        let mut camera = Camera { zoom: 2., pan: point(px(43.), px(-27.)), ..Camera::default() };
        let anchor = point(px(-105.), px(75.));
        let scene_point = (anchor - camera.pan) / camera.zoom;
        for factor in [1.3, 100., 0.0001, 2.] {
            camera.zoom_at(anchor, factor);
            let actual = (anchor - camera.pan) / camera.zoom;
            assert!((actual.x - scene_point.x).abs() < px(0.001));
            assert!((actual.y - scene_point.y).abs() < px(0.001));
            assert!((0.25..=16.).contains(&camera.zoom));
        }
    }
    #[::core::prelude::v1::test]
    fn pan_and_zoom_share_the_same_local_transform_in_every_panel() {
        let mut camera = Camera::default();
        camera.pan += point(px(76.), px(-35.));
        camera.zoom_at(point(px(30.), px(20.)), 3.);
        let bounds = |x| Bounds::new(point(px(x), px(200.)), gpui::size(px(450.), px(450.)));
        let first = camera.origin(bounds(10.)) - bounds(10.).origin;
        for x in [480., 950.] {
            assert_eq!(camera.origin(bounds(x)) - bounds(x).origin, first);
        }
    }
}
