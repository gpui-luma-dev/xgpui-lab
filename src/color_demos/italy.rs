use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use gpui_luma::{controls::button::Button, prelude::HasPresenter};

use gpui::{prelude::*, *};
use gpui_luma_look_radix::Look;
use xgpui_media::{ImageJob, LoadStatus, MediaImage, MediaLimits, MediaLoader, UploadStatus};
use crate::{
    lab_controls::{Choice, labels},
    lab_ui,
    theme::ColorVizLookExt,
};
use super::italy_model as model;

const NAMES: [&str; 3] = ["sRGB", "Display P3", "Out of sRGB areas"];

struct ThumbnailState {
    images: RwLock<[Option<MediaImage>; 3]>,
    errors: RwLock<[Option<String>; 3]>,
    selected: AtomicUsize,
    native: AtomicBool,
}

pub(crate) struct ItalyLab {
    look: Arc<Look>,
    selected: usize,
    preview: bool,
    presentation: Choice,
    thumbnails: Vec<Entity<Button>>,
    thumbnail_state: Arc<ThumbnailState>,
    loader: anyhow::Result<MediaLoader>,
    images: [Option<MediaImage>; 3],
    jobs: [Option<ImageJob>; 3],
    error: Option<String>,
    presentation_requested: bool,
}

impl ItalyLab {
    pub(crate) fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let presentation = Choice::new(
            "italy-mode",
            "Presentation",
            &look,
            labels(&["Native Display P3", "sRGB preview"]),
            0,
            cx,
            |this: &mut Self, index, cx| {
                this.preview = index == 1;
                if !this.preview {
                    this.presentation_requested = false;
                }
                cx.notify();
            },
        );
        let thumbnail_state = Arc::new(ThumbnailState {
            images: RwLock::new(std::array::from_fn(|_| None)),
            errors: RwLock::new(std::array::from_fn(|_| None)),
            selected: AtomicUsize::new(1),
            native: AtomicBool::new(false),
        });
        let thumbnails = NAMES
            .iter()
            .enumerate()
            .map(|(index, &label)| {
                let state = thumbnail_state.clone();
                let geometry = gpui_luma_look_radix::button_look_for::<()>(
                    &look,
                    gpui_luma_look_radix::ButtonVariant::Ghost,
                    gpui_luma_look_radix::Paint::gray(),
                    gpui_luma_look_radix::ButtonSize::One,
                    Default::default(),
                );
                let button = Button::new(format!("italy-thumb-{index}"))
                    .with_look(move |model| {
                        let mut resolved = geometry(model);
                        // Selection and keyboard focus use the card fill, without an outer ring.
                        resolved.border = None;
                        resolved
                    })
                    .content(move |_, _| {
                        let image = state.images.read().unwrap_or_else(|error| error.into_inner())[index].clone();
                        let mut preview = div()
                            .w(px(174.))
                            .h(px(116.))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .debug_selector(move || format!("italy-thumb-image-{index}"));
                        if let Some(image) = image {
                            let view = xgpui_media::image_view(image).size_full();
                            preview = if state.native.load(Ordering::Relaxed) {
                                match view.wide_gamut() {
                                    Ok(view) => preview.child(view),
                                    Err(error) => preview.child(error.to_string()),
                                }
                            } else {
                                preview.child(view)
                            };
                        } else {
                            let error = state.errors.read().unwrap_or_else(|error| error.into_inner())[index].clone();
                            preview = preview.child(
                                error
                                    .map(|error| format!("Unable to load: {error}"))
                                    .unwrap_or_else(|| "Loading…".to_owned()),
                            );
                        }
                        div()
                            .flex()
                            .flex_col()
                            .w(px(174.))
                            .flex_shrink_0()
                            .gap_3()
                            .child(preview)
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(label))
                    })
                    .on_click(cx, move |this: &mut Self, cx| this.select_photo(index, cx))
                    .spawn(cx);
                let state = thumbnail_state.clone();
                let paint_look = look.clone();
                let template = gpui_luma::controls::button::DefaultButtonTemplate::new(
                    gpui_luma_look_radix::button_family_theme_with(
                        &look,
                        gpui_luma_look_radix::ButtonVariant::Ghost,
                        gpui_luma_look_radix::Paint::gray(),
                    ),
                )
                .with_modifier(move |row, model| {
                    let colors = paint_look.interaction_colors();
                    let selected = state.selected.load(Ordering::Relaxed) == index;
                    lab_ui::comparison_surface(row, &paint_look)
                        .w(px(200.))
                        .max_w_full()
                        .min_w_0()
                        .debug_selector(move || format!("italy-thumb-{index}"))
                        .h_auto()
                        .when_some(colors.background(selected, model.state), |row, color| row.bg(color))
                });
                button.update(cx, |button, cx| button.set_template(Arc::new(template), cx));
                button
            })
            .collect();
        let mut this = Self {
            look,
            selected: 1,
            preview: false,
            presentation,
            thumbnails,
            thumbnail_state,
            loader: MediaLoader::new(MediaLimits {
                max_jobs: 3,
                max_resident_bytes: 32 * 1024 * 1024,
                max_image_bytes: 8 * 1024 * 1024,
                max_encoded_bytes: 8 * 1024 * 1024,
            }),
            images: std::array::from_fn(|_| None),
            jobs: std::array::from_fn(|_| None),
            error: None,
            presentation_requested: false,
        };
        for index in 0..3 {
            this.load_image(index);
        }
        this
    }

    pub(crate) fn activate(&mut self, cx: &mut Context<Self>) {
        self.presentation_requested = false;
        cx.notify();
    }

    fn select_photo(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        cx.notify();
    }

    fn load_image(&mut self, index: usize) {
        if self.images[index].is_some() || self.jobs[index].is_some() {
            return;
        }
        self.error = None;
        let result = (|| {
            let loader = self.loader.as_ref().map_err(|error| anyhow::anyhow!("{error}"))?;
            loader.load_pixel_buffer(
                600,
                400,
                model::pixels(index)?,
                model::source_color(),
                xgpui_media::pipeline::ImagePrimaries::DisplayP3,
                xgpui_color::GamutMapping::Clip,
            )
        })();
        match result {
            Ok(job) => self.jobs[index] = Some(job),
            Err(error) => {
                let message = format!("{}: {error}", NAMES[index]);
                self.thumbnail_state.errors.write().unwrap_or_else(|error| error.into_inner())[index] =
                    Some(message.clone());
                self.error = Some(message);
            }
        }
    }
}

impl Render for ItalyLab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.presentation_requested {
            self.presentation_requested = true;
            cx.defer_in(window, |this, window, cx| {
                if let Err(error) = window.set_color_mode(WindowColorMode::WideGamutSdr) {
                    this.error = Some(format!("Native P3 unavailable: {error}"));
                }
                cx.notify();
            });
        }
        for index in 0..3 {
            if let Some(job) = &mut self.jobs[index] {
                if let Some(image) = job.try_take() {
                    self.images[index] = Some(image.clone());
                    self.thumbnail_state.images.write().unwrap_or_else(|error| error.into_inner())[index] = Some(image);
                    self.thumbnails[index].update(cx, |_, cx| cx.notify());
                    self.jobs[index] = None;
                } else if let LoadStatus::Failed(error) = job.status() {
                    let message = format!("{}: {error}", NAMES[index]);
                    self.thumbnail_state.errors.write().unwrap_or_else(|error| error.into_inner())[index] =
                        Some(message.clone());
                    self.error = Some(message);
                    self.thumbnails[index].update(cx, |_, cx| cx.notify());
                    self.jobs[index] = None;
                }
            }
        }
        if self.jobs.iter().any(Option::is_some) {
            window.request_animation_frame();
        }
        let chrome = self.look.chrome();
        let native = !self.preview && window.color_status().effective() == WindowColorMode::WideGamutSdr;
        let selected_changed = self.thumbnail_state.selected.swap(self.selected, Ordering::Relaxed) != self.selected;
        let mode_changed = self.thumbnail_state.native.swap(native, Ordering::Relaxed) != native;
        if selected_changed || mode_changed {
            for thumbnail in &self.thumbnails {
                thumbnail.update(cx, |_, cx| cx.notify());
            }
        }
        let mut photo = div()
            .debug_selector(|| "italy-selected-photo".into())
            .w_full()
            .max_w(px(600.))
            .aspect_ratio(1.5)
            .flex_shrink_0()
            .bg(chrome.panel_background);
        if let Some(image) = &self.images[self.selected] {
            let view = xgpui_media::image_view(image.clone()).size_full();
            photo = if native {
                match view.wide_gamut() {
                    Ok(view) => photo.child(view),
                    Err(error) => photo.child(format!("Native P3 unavailable: {error}")),
                }
            } else {
                photo.child(view)
            };
            if let UploadStatus::Failed(error) = image.upload_status() {
                self.error = Some(format!("Photo upload failed: {error}"));
            }
        } else {
            let error =
                self.thumbnail_state.errors.read().unwrap_or_else(|error| error.into_inner())[self.selected].clone();
            photo = photo.child(
                error
                    .map(|error| format!("Unable to load: {error}"))
                    .unwrap_or_else(|| "Loading Italy photograph…".to_owned()),
            );
        }
        lab_ui::page("italy-lab-scroll", &self.look).child(
            lab_ui::container()
                .child(lab_ui::caption(&self.look, "xgpui-color  /  Italy Photo Comparison"))
                .child(lab_ui::header(&self.look, "Italy Photo Comparison",
                    "Compare native Display P3 and sRGB with the same photograph."))
                .child(div().flex().flex_col().gap_4()
                    .child(div().flex().flex_col().w_full().max_w(px(600.)).mx_auto().gap_2()
                        .child(photo)
                        .child(lab_ui::caption(&self.look, format!("{} · {} · 600 × 400 · Effective window mode: {:?}", NAMES[self.selected],
                        if native { "Native Display P3" } else { "sRGB preview" }, window.color_status().effective())).debug_selector(|| "italy-photo-description".into())))
                    .child(lab_ui::control_row().justify_center().child(lab_ui::control(&self.look, self.presentation.clone().into_element())))
                    .when(!self.preview && !native, |view| view.child(lab_ui::caption(&self.look,
                        "Wide-gamut output is unavailable on this window; showing the sRGB fallback.")))
                    .child(div().flex().flex_wrap().justify_center().gap_4().children(self.thumbnails.clone()))
                    .when_some(self.error.clone(), |view, error| view.child(lab_ui::caption(&self.look, error)))
                    .child(lab_ui::caption(&self.look,
                        "Look at the orange and red umbrellas. Visible differences depend on display capability."))
                    .child(lab_ui::caption(&self.look,
                        "Italy images courtesy of WebKit’s Wide Gamut demo (webkit.org/blog-files/color-gamut/). Prepared 600 × 400 linear-P3 fixtures use the original embedded profiles. The highlighted version contains WebKit’s diagnostic markings. This is not display calibration."))),
        )
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::{ItalyLab};
    use gpui::TestAppContext;

    #[test]
    fn three_thumbnail_buttons_select_the_large_photo() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(|_, cx| ItalyLab::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        for index in [0, 2, 1] {
            let thumb = cx
                .debug_bounds(["italy-thumb-0", "italy-thumb-1", "italy-thumb-2"][index])
                .expect("thumbnail button");
            let photo = cx.debug_bounds("italy-selected-photo").expect("large selected photo");
            assert!(photo.size.width > thumb.size.width * 2.);
            cx.simulate_click(thumb.center(), Default::default());
            cx.run_until_parked();
            cx.update(|_, cx| {
                let lab = view.read(cx);
                assert_eq!(lab.thumbnails.len(), 3);
                assert_eq!(lab.selected, index);
                assert_eq!(lab.thumbnail_state.selected.load(std::sync::atomic::Ordering::Relaxed), index);
            });
        }
    }

    #[test]
    fn photo_and_cards_preserve_aspect_ratio_and_wrap_in_both_themes() {
        for mode in [gpui_luma::theme::ThemeMode::Light, gpui_luma::theme::ThemeMode::Dark] {
            let mut app = TestAppContext::single();
            app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
            let look = crate::theme::default_look();
            look.set_mode(mode);
            let (_view, cx) = app.add_window_view(|_, cx| ItalyLab::new(look, cx));
            for width in [1000., 420.] {
                cx.simulate_resize(gpui::size(gpui::px(width), gpui::px(1400.)));
                cx.run_until_parked();
                let photo = cx.debug_bounds("italy-selected-photo").unwrap();
                assert!((photo.size.width.as_f32() / photo.size.height.as_f32() - 1.5).abs() < 0.01);
                assert!(photo.size.width <= gpui::px(600.));
                assert!((photo.center().x.as_f32() - width / 2.).abs() < 1.);
                let previews = ["italy-thumb-image-0", "italy-thumb-image-1", "italy-thumb-image-2"]
                    .map(|id| cx.debug_bounds(id).unwrap());
                for preview in previews {
                    assert_eq!(preview.size, gpui::size(gpui::px(174.), gpui::px(116.)));
                }
                let cards: Vec<_> = ["italy-thumb-0", "italy-thumb-1", "italy-thumb-2"]
                    .map(|id| cx.debug_bounds(id).unwrap())
                    .into_iter()
                    .collect();
                for card in &cards {
                    assert!(card.left() >= gpui::px(0.) && card.right() <= gpui::px(width));
                    assert!(card.top() >= photo.bottom());
                }
                if width < 600. {
                    assert!(cards[1].top() > cards[0].top(), "photo cards must wrap");
                } else {
                    assert_eq!(cards[0].top(), cards[2].top());
                    assert!(((cards[0].left() + cards[2].right()).as_f32() / 2. - width / 2.).abs() < 1.);
                }
            }
        }
    }

    #[test]
    fn native_photo_and_presentation_selectors_keep_the_same_selected_source() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(|_, cx| ItalyLab::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        view.update(cx, |lab, cx| lab.select_photo(2, cx));
        let presentation = cx.update(|_, cx| view.read(cx).presentation.clone());
        cx.update(|_, cx| presentation.select(1, cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let lab = view.read(cx);
            assert_eq!(lab.selected, 2);
            assert!(lab.preview);
        });
        cx.update(|_, cx| presentation.select(0, cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let lab = view.read(cx);
            assert_eq!(lab.selected, 2);
            assert!(!lab.preview);
        });
    }
}
