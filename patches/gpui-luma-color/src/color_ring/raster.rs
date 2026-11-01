use gpui_luma::color::gpui_bridge;
use super::common::{mirrored_lightness, mirrored_saturation};
use super::track_context::ColorRingTrackContext;
use super::types::ColorRingTrackDelegate;
use crate::color_slider::color_spec::Hsv;
use crate::style::Size as ComponentSize;
use gpui::*;
use std::f32::consts::TAU;
use std::sync::{Arc, Mutex};
use tiny_skia::{Pixmap, PremultipliedColorU8};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorRingRenderer {
    Vector,
    #[default]
    Raster,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorRingRasterMode {
    Hue,
    Saturation,
    Lightness,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RingCacheKey {
    mode: u16,
    hue: u16,
    value: u16,
    saturation: u16,
    lightness: u16,
    ring_thickness: u16,
    rotation: u16,
}

type RingImageCache = Option<(Size<Pixels>, RingCacheKey, Arc<Image>)>;

pub struct RasterRingDelegate {
    mode: ColorRingRasterMode,
    hue: f32,
    hsv_value: f32,
    saturation: f32,
    lightness: f32,
    image_cache: Mutex<RingImageCache>,
}

impl RasterRingDelegate {
    pub fn hue(saturation: f32, lightness: f32) -> Self {
        Self {
            mode: ColorRingRasterMode::Hue,
            hue: 0.0,
            hsv_value: 1.0,
            saturation: saturation.clamp(0.0, 1.0),
            lightness: lightness.clamp(0.0, 1.0),
            image_cache: Mutex::new(None),
        }
    }

    pub fn saturation(hue: f32, hsv_value: f32) -> Self {
        Self {
            mode: ColorRingRasterMode::Saturation,
            hue: normalize_hue_degrees(hue),
            hsv_value: hsv_value.clamp(0.0, 1.0),
            saturation: 1.0,
            lightness: 0.5,
            image_cache: Mutex::new(None),
        }
    }

    pub fn lightness(hue: f32, saturation: f32) -> Self {
        Self {
            mode: ColorRingRasterMode::Lightness,
            hue: normalize_hue_degrees(hue),
            hsv_value: 1.0,
            saturation: saturation.clamp(0.0, 1.0),
            lightness: 1.0,
            image_cache: Mutex::new(None),
        }
    }

    fn cache_scale(size: ComponentSize) -> f32 {
        match size {
            ComponentSize::XSmall | ComponentSize::Small => 4.0,
            ComponentSize::Medium => 3.0,
            ComponentSize::Large => 2.0,
            ComponentSize::Size(px) => {
                let size = px.as_f32();
                if size <= 180.0 {
                    4.0
                } else if size <= 260.0 {
                    3.0
                } else {
                    2.0
                }
            }
        }
    }

    fn color_at_position(&self, position: f32) -> Hsla {
        match self.mode {
            ColorRingRasterMode::Hue => {
                hsla(position.rem_euclid(1.0), self.saturation.clamp(0.0, 1.0), self.lightness.clamp(0.0, 1.0), 1.0)
            }
            ColorRingRasterMode::Saturation => Hsv {
                h: self.hue.rem_euclid(360.0),
                s: mirrored_saturation(position),
                v: self.hsv_value.clamp(0.0, 1.0),
                a: 1.0,
            }
            .to_hsla_ext(),
            ColorRingRasterMode::Lightness => hsla(
                self.hue.rem_euclid(360.0) / 360.0,
                self.saturation.clamp(0.0, 1.0),
                mirrored_lightness(position),
                1.0,
            ),
        }
    }
}

impl ColorRingTrackDelegate for RasterRingDelegate {
    fn paint_domain_track(&self, _context: &ColorRingTrackContext, _bounds: Bounds<Pixels>, _window: &mut Window) {}

    fn get_color_for_context(&self, _context: &ColorRingTrackContext, position: f32) -> Hsla {
        self.color_at_position(position)
    }

    fn raster_cached_image(&self, context: &ColorRingTrackContext, image_size: Size<Pixels>) -> Option<Arc<Image>> {
        let key = RingCacheKey {
            mode: match self.mode {
                ColorRingRasterMode::Hue => 0,
                ColorRingRasterMode::Saturation => 1,
                ColorRingRasterMode::Lightness => 2,
            },
            hue: (self.hue.rem_euclid(360.0) * 10.0).round() as u16,
            value: (self.hsv_value.clamp(0.0, 1.0) * 1000.0).round() as u16,
            saturation: (self.saturation.clamp(0.0, 1.0) * 1000.0).round() as u16,
            lightness: (self.lightness.clamp(0.0, 1.0) * 1000.0).round() as u16,
            ring_thickness: (context.ring_thickness_px().clamp(1.0, 200.0) * 10.0).round() as u16,
            rotation: (context.rotation_turns() * 3600.0).round() as u16,
        };
        if let Some((cached_size, cached_key, image)) = self.image_cache.lock().expect("ring cache lock").as_ref()
            && *cached_size == image_size
            && *cached_key == key
        {
            return Some(image.clone());
        }

        let cache_scale = Self::cache_scale(context.size);
        let width = (image_size.width.as_f32() * cache_scale).round() as u32;
        let height = (image_size.height.as_f32() * cache_scale).round() as u32;
        if width == 0 || height == 0 {
            return None;
        }

        let mut pixmap = Pixmap::new(width, height)?;
        let pixels = pixmap.pixels_mut();

        let center_x = width as f32 / 2.0;
        let center_y = height as f32 / 2.0;
        let outer_radius = width.min(height) as f32 / 2.0;
        let ring_thickness = context.ring_thickness_px() * cache_scale;
        let inner_radius = (outer_radius - ring_thickness).max(0.0);
        let rotation_turns = context.rotation_turns();

        for y in 0..height {
            for x in 0..width {
                let mut r_sum = 0.0_f32;
                let mut g_sum = 0.0_f32;
                let mut b_sum = 0.0_f32;
                let mut covered = 0_u8;

                for sy in [0.25_f32, 0.75_f32] {
                    for sx in [0.25_f32, 0.75_f32] {
                        let dx = x as f32 + sx - center_x;
                        let dy = y as f32 + sy - center_y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist > outer_radius || dist < inner_radius {
                            continue;
                        }

                        let theta = dy.atan2(dx);
                        let position = (0.25 + (theta / TAU) - rotation_turns).rem_euclid(1.0);
                        let rgb = gpui_bridge::preview_rgba(self.color_at_position(position));
                        r_sum += rgb.r;
                        g_sum += rgb.g;
                        b_sum += rgb.b;
                        covered += 1;
                    }
                }

                if covered == 0 {
                    continue;
                }

                let inv = 1.0 / covered as f32;
                let r_u8 = (r_sum * inv * 255.0).round() as u8;
                let g_u8 = (g_sum * inv * 255.0).round() as u8;
                let b_u8 = (b_sum * inv * 255.0).round() as u8;
                let a_u8 = ((covered as f32 / 4.0) * 255.0).round() as u8;

                if let Some(pixel) = PremultipliedColorU8::from_rgba(r_u8, g_u8, b_u8, a_u8) {
                    pixels[(y * width + x) as usize] = pixel;
                }
            }
        }

        let png_data = pixmap.encode_png().ok()?;
        let image = Arc::new(Image::from_bytes(ImageFormat::Png, png_data));
        *self.image_cache.lock().expect("ring cache lock") = Some((image_size, key, image.clone()));
        Some(image)
    }
}

fn normalize_hue_degrees(hue: f32) -> f32 {
    hue.rem_euclid(360.0)
}
