//! Generated pixels, not an imported image: an original ring/X gamut-reveal motif.
use super::*;

pub(super) struct Reveal {
    jobs: Vec<ImageJob>,
    images: Vec<Option<MediaImage>>,
    error: Option<String>,
}
fn symbol(x: f64, y: f64) -> bool {
    let radius = x.hypot(y);
    (0.66..0.75).contains(&radius) || (x.abs() < 0.44 && y.abs() < 0.44 && (x.abs() - y.abs()).abs() < 0.09)
}
fn colors() -> anyhow::Result<([f64; 3], [f64; 3])> {
    let background = [1., 0., 0.]; // Linear Display P3 red, outside sRGB.
    let foreground = LinearRgb::new(RgbSpace::Srgb, [1., 0., 0.])?.convert(RgbSpace::DisplayP3)?.components(); // Same sRGB red expressed in P3; no reinterpretation.
    Ok((background, foreground))
}
impl Reveal {
    pub(super) fn new() -> anyhow::Result<Self> {
        let (background, foreground) = colors()?;
        let side = 384u32;
        let mut native = Vec::with_capacity((side * side) as usize);
        let mut mask = Vec::with_capacity(native.capacity());
        for y in 0..side {
            for x in 0..side {
                let coverage = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)]
                    .into_iter()
                    .filter(|(dx, dy)| {
                        symbol(
                            2. * (f64::from(x) + dx) / f64::from(side) - 1.,
                            2. * (f64::from(y) + dy) / f64::from(side) - 1.,
                        )
                    })
                    .count() as f64
                    / 4.;
                let rgb: [f64; 3] = std::array::from_fn(|c| background[c] * (1. - coverage) + foreground[c] * coverage);
                native.push([rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.]);
                let srgb = LinearRgb::new(RgbSpace::DisplayP3, rgb)?.convert(RgbSpace::Srgb)?.components();
                let outside = srgb.into_iter().any(|v| !(-1e-10..=1. + 1e-10).contains(&v));
                mask.push(if outside { [1., 0., 1., 1.] } else { [1., 0., 0., 1.] });
            }
        }
        let loader = MediaLoader::default();
        Ok(Self {
            jobs: vec![
                loader.load_pixel_buffer(
                    side,
                    side,
                    native,
                    ImageColor::LINEAR_P3.into(),
                    ImagePrimaries::DisplayP3,
                    GamutMapping::Clip,
                )?,
                loader.load_pixel_buffer(
                    side,
                    side,
                    mask,
                    ImageColor::SRGB.into(),
                    ImagePrimaries::DisplayP3,
                    GamutMapping::Clip,
                )?,
            ],
            images: vec![None, None],
            error: None,
        })
    }
    fn poll(&mut self, window: &mut Window) {
        for (job, image) in self.jobs.iter_mut().zip(&mut self.images) {
            if let Some(ready) = job.try_take() {
                *image = Some(ready);
            }
            match job.status() {
                xgpui_media::LoadStatus::Loading => window.request_animation_frame(),
                xgpui_media::LoadStatus::Failed(error) => self.error = Some(error),
                _ => {}
            }
        }
        for image in self.images.iter().flatten() {
            if let xgpui_media::UploadStatus::Failed(error) = image.upload_status() {
                self.error = Some(error);
            }
        }
    }
    fn picture(&self, selection: usize, wide: bool) -> impl IntoElement {
        let image = self.images[usize::from(selection == 2)].clone();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                if let Some(image) = &image {
                    wake_for_upload(image, bounds, window);
                    let geometry = CustomGeometry::RoundedRect { bounds, corner_radii: Default::default() };
                    if selection == 1 && wide {
                        if let Ok(image) = image.wide(None) {
                            window.paint_custom(geometry, Arc::new(image));
                        }
                    } else {
                        window.paint_custom(geometry, Arc::new(image.clone()));
                    }
                }
            },
        )
        .size_full()
    }
}
impl Lab {
    pub(super) fn render_reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(reveal) = &mut self.reveal {
            reveal.poll(window);
        }
        let labels = ["sRGB preview", "Display P3 — native wide", "Out-of-sRGB mask"];
        let mut body = div().id("gamut-reveal-scroll").size_full().overflow_y_scroll().p_6().bg(rgb(0x858585)).text_color(rgb(0x101010)).flex().flex_col().gap_4()
            .child(div().flex().flex_wrap().gap_2()
                .when(self.standalone_navigation, |row| row.child(self.button("← Chromaticity & metadata".into(), 18, cx)))
                .child(self.window_mode_controls(window, cx)))
            .child(div().text_2xl().child("Hidden wide-gamut symbol"))
            .child(div().id("webkit-gamut-attribution").cursor_pointer().text_color(rgb(0x003d76))
                .child("Inspired by WebKit — Examples of various wide-gamut images ↗")
                .on_click(|_, _, cx| cx.open_url("https://webkit.org/blog-files/color-gamut/")))
            .child("Choose a thumbnail below. In native P3, look for a ring and X in the red field; in the clipped sRGB preview, the symbol should disappear.")
            .child(format!("Currently showing: {} • effective {:?} • display wide gamut {:?}", labels[self.reveal_selection], window.color_status().effective(), window.color_status().display_wide_gamut()));
        if self.reveal_selection == 1 && !self.wide {
            body = body.child("Native wide output is unavailable or disabled: showing the sRGB fallback. The symbol is not expected to appear.");
        }
        if let Some(reveal) = &self.reveal {
            body = body
                .child(
                    div()
                        .w_full()
                        .max_w(px(620.))
                        .aspect_ratio(1.)
                        .flex_shrink_0()
                        .child(reveal.picture(self.reveal_selection, self.wide)),
                )
                .child(div().flex().flex_wrap().gap_4().children((0..3).map(|selection| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .id(ElementId::Name(format!("reveal-thumbnail-{selection}").into()))
                                .w(px(150.))
                                .h(px(150.))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.reveal_selection = selection;
                                    cx.notify();
                                }))
                                .child(reveal.picture(selection, self.wide)),
                        )
                        .child(self.button(labels[selection].into(), 19 + selection, cx))
                })))
                .child(reveal.error.clone().unwrap_or_default());
        }
        body.child("What this shows: the background is P3 red; the symbol is sRGB red converted into P3 coordinates. Clipping both to sRGB makes them the same red. The mask marks source colors outside sRGB in magenta; red is inside. Edge coverage is generated in linear light.")
            .child("Use a wide-gamut display and native wide output. The OS display profile and panel determine the visible contrast. A screenshot, an sRGB preview, or a capability flag alone cannot establish physical wide-gamut delivery. No HDR brightness is involved.")
            .child("Original generated ring/X motif and implementation; WebKit is credited for the demonstration concept. No WebKit artwork is embedded, and no image file or ICC decoder is used.")
            .child(self.error.clone().unwrap_or_default())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;
    #[test]
    fn native_colors_differ_but_srgb_clipping_hides_the_symbol() {
        let (background, foreground) = colors().unwrap();
        assert!((background[0] - foreground[0]).abs() > 0.15);
        for color in [background, foreground] {
            let preview = LinearRgb::new(RgbSpace::DisplayP3, color)
                .unwrap()
                .map_gamut(RgbSpace::Srgb, GamutMapping::Clip)
                .unwrap()
                .components();
            for (value, expected) in preview.into_iter().zip([1., 0., 0.]) {
                assert!((value - expected).abs() < 1e-12);
            }
        }
        // Check the actual half-float storage boundary, not just f64 conversion.
        let pixels: Vec<[f32; 4]> =
            [background, foreground].into_iter().map(|v| [v[0] as f32, v[1] as f32, v[2] as f32, 1.]).collect();
        let packed = pipeline::pixels_rgba16f_data(
            2,
            1,
            pixels,
            ImageColor::LINEAR_P3.into(),
            ImagePrimaries::DisplayP3,
            GamutMapping::Clip,
        )
        .unwrap();
        let data = &packed.levels()[0];
        for pixel in data.as_chunks::<8>().0 {
            let rgb = std::array::from_fn(|i| half(u16::from_le_bytes([pixel[2 * i], pixel[2 * i + 1]])));
            let preview = LinearRgb::new(RgbSpace::DisplayP3, rgb)
                .unwrap()
                .map_gamut(RgbSpace::Srgb, GamutMapping::Clip)
                .unwrap()
                .components();
            for (value, expected) in preview.into_iter().zip([1., 0., 0.]) {
                assert!((value - expected).abs() < 0.001);
            }
        }
        let bg = LinearRgb::new(RgbSpace::DisplayP3, background)
            .unwrap()
            .convert(RgbSpace::Srgb)
            .unwrap()
            .components();
        let fg = LinearRgb::new(RgbSpace::DisplayP3, foreground)
            .unwrap()
            .convert(RgbSpace::Srgb)
            .unwrap()
            .components();
        assert!(bg[0] > 1.2 && bg[1] < 0.);
        assert!(fg.into_iter().all(|v| (-1e-10..=1. + 1e-10).contains(&v)));
        assert!(symbol(0.7, 0.));
        assert!(symbol(0., 0.));
        assert!(!symbol(0.9, 0.));
    }
}
