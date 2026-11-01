use gpui::{Context, Entity, FocusHandle, Render, Window, div, prelude::*, px};
use gpui_luma_color::style::{ColorControlTheme, set_active_color_control_theme};
use gpui_luma::focus::LumaFocusScopeExt;
use gpui_luma::theme::ThemeMode;
use gpui_luma_look_radix::Look;
use crate::theme::ColorVizLookExt;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::{cell::RefCell, rc::Rc};
use gpui_luma::controls::{
    resizable_panels::{
        ResizablePanels, ResizablePanelSpec, ResizablePanelsEvent, PanelHideMode, ResizeHandleVisibility,
        ResizeCollapseBehavior, ResizeCollapseMode, ResizeCollapseDirection,
    },
    accordion::{Accordion, AccordionContent, AccordionItem, AccordionTrigger},
};
use gpui_luma::prelude::HasPresenter;
use gpui_luma::controls::button::ButtonEventExt;
use gpui_luma::controls::resizable_panels::ResizablePanelsEventExt;
use gpui_luma::theme::LumaThemeSyncExt;

use crate::gradient_builder::GradientBuilder;
use crate::compositions::ColorCompositions;
use crate::extension_pane::ExtensionPane;

mod nav_accordion;
mod shell_style;
mod titlebar;
use shell_style::*;
use titlebar::TitlebarControls;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Workspace {
    Gradients,
    Compositions,
    ColorGamut,
    Color,
    Gamut,
    Media,
    Parley,
    Shader,
    Svg,
}

type Section = (Workspace, &'static str, &'static [(&'static str, &'static str)]);

const SECTIONS: &[Section] = &[
    (
        Workspace::Gradients,
        "Gradients",
        &[("gradients", "Linear"), ("mesh", "Mesh"), ("freeform", "Freeform")],
    ),
    (
        Workspace::Compositions,
        "Compositions",
        &[
            ("picker", "Color Picker"),
            ("hsv-plane", "HSV Plane"),
            ("hsv-wheel", "HSV Wheel"),
            ("sv-triangle", "SV Triangle"),
            ("split-ring", "Split Ring"),
            ("harmonies", "Color Harmonies"),
        ],
    ),
    (Workspace::ColorGamut, "Color & Gamut", &[("Color & Gamut", "Color & Gamut")]),
    (
        Workspace::Color,
        "xgpui-color",
        &[
            ("Color Spaces", "Color Spaces"),
            ("Interactive Color Spaces", "Interactive Color Spaces"),
            ("Conversions", "Conversions"),
            ("Interpolation", "Interpolation"),
            ("Gamut Mapping", "Gamut Mapping"),
            ("CPU Gradients", "CPU Gradients"),
            ("Italy Photo Comparison", "Italy Photo Comparison"),
        ],
    ),
    (
        Workspace::Gamut,
        "xgpui-gamut",
        &[
            ("Wide Color Fills", "Wide Color Fills"),
            ("Wide Gradients", "Wide Gradients"),
            ("Chromaticity & Gamut", "Chromaticity & Gamut"),
            ("Gamut Mapping", "Gamut Mapping"),
            ("Presentation", "Presentation"),
            ("Benchmarks", "Benchmarks"),
        ],
    ),
    (
        Workspace::Media,
        "xgpui-media",
        &[
            ("Images", "Images"),
            ("Textures & Mipmaps", "Textures & Mipmaps"),
            ("Image Filters", "Image Filters"),
            ("Linear Adjustments", "Linear Adjustments"),
            ("Float / P3", "Float / P3"),
            ("Tiled Canvas", "Tiled Canvas"),
            ("Video Frames", "Video Frames"),
            ("Resource Limits", "Resource Limits"),
            ("Benchmarks", "Benchmarks"),
        ],
    ),
    (
        Workspace::Parley,
        "xgpui-parley",
        &[
            ("Text Shaping", "Text Shaping"),
            ("Line Layout", "Line Layout"),
            ("Paragraphs", "Paragraphs"),
            ("Fonts & Fallback", "Fonts & Fallback"),
            ("Rasterization", "Rasterization"),
            ("Scrolling", "Scrolling"),
            ("Benchmarks", "Benchmarks"),
        ],
    ),
    (
        Workspace::Shader,
        "xgpui-shader",
        &[
            ("Gradients", "Gradients"),
            ("Mesh Editor", "Mesh Editor"),
            ("Gradient Paints", "Gradient Paints"),
            ("WGSL Paints", "WGSL Paints"),
            ("Shader Depot: Grid", "Shader Depot: Grid"),
            ("Shadertoy: Seascape", "Shadertoy: Seascape"),
            ("Instancing", "Instancing"),
            ("Typography", "Typography"),
            ("Brushes", "Brushes"),
            ("Layer Effects", "Layer Effects"),
            ("Shadows", "Shadows"),
            ("Borders", "Borders"),
            ("Benchmarks", "Benchmarks"),
        ],
    ),
    (
        Workspace::Svg,
        "xgpui-svg",
        &[
            ("Rendering", "Rendering"),
            ("Scaling", "Scaling"),
            ("Path Effects", "Path Effects"),
            ("Scheduling & Cache", "Scheduling & Cache"),
            ("Benchmarks", "Benchmarks"),
        ],
    ),
];

struct NavigationEntry {
    workspace: Workspace,
    page: &'static str,
    button: Entity<gpui_luma::controls::button::Button>,
    selected: Arc<AtomicBool>,
}

pub struct XgpuiLabApp {
    focus_scope: FocusHandle,
    look: Arc<Look>,
    nav_look: Arc<Look>,
    gradient_builder: Entity<GradientBuilder>,
    compositions: Option<Entity<ColorCompositions>>,
    color_gamut: Option<Entity<crate::color_gamut_page::Page>>,
    navigation: Vec<NavigationEntry>,
    panels: Entity<ResizablePanels>,
    rail: Entity<gpui_luma::controls::frame::FrameControl>,
    sidebar_width: gpui::Pixels,
    titlebar: TitlebarControls,
    content: Rc<RefCell<gpui::AnyView>>,
    active_page: &'static str,
    extensions: Vec<(Workspace, Entity<ExtensionPane>)>,
    active: Workspace,
    captured_color_mode: Option<gpui::WindowColorMode>,
    presentation_pending: bool,
}

impl XgpuiLabApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let p3_error = window.set_color_mode(gpui::WindowColorMode::WideGamutSdr).err().map(|error| error.to_string());
        let focus_scope = cx.focus_handle();
        let look = crate::theme::default_look();
        let nav_look = look.clone();
        sync_color_control_theme(&look);
        let gradient_builder = cx.new(|cx| GradientBuilder::new(look.clone(), cx));
        let extensions = SECTIONS
            .iter()
            .filter_map(|&(workspace, _, _)| {
                let id = match workspace {
                    Workspace::Gradients | Workspace::Compositions | Workspace::ColorGamut => return None,
                    Workspace::Color => "xgpui-color-tabs",
                    Workspace::Gamut => "xgpui-gamut-tabs",
                    Workspace::Media => "xgpui-media-tabs",
                    Workspace::Parley => "xgpui-parley-tabs",
                    Workspace::Shader => "xgpui-shader-tabs",
                    Workspace::Svg => "xgpui-svg-tabs",
                };
                Some((workspace, cx.new(|cx| ExtensionPane::new(id, look.clone(), p3_error.clone(), cx))))
            })
            .collect::<Vec<_>>();
        let mut navigation = Vec::new();
        let mut items = Vec::new();
        for &(workspace, label, entries) in SECTIONS {
            let mut rows = Vec::new();
            for &(page, title) in entries {
                let selected = Arc::new(AtomicBool::new(workspace == Workspace::Gradients && page == "gradients"));
                let paint_selected = selected.clone();
                let entry_look = nav_look.clone();
                let label_look = nav_look.clone();
                let label_selected = selected.clone();
                let button = gpui_luma_look_radix::Button::new(format!("index-{label}-{page}"))
                    .look(nav_look.as_ref())
                    .ghost()
                    .gray()
                    .size(gpui_luma_look_radix::ButtonSize::One)
                    .content(move |_, _| {
                        let step = if label_selected.load(Ordering::Relaxed) { 12 } else { 11 };
                        div()
                            .text_color(label_look.resolve_step(gpui_luma_look_radix::ScaleFamily::Gray, step).hsla())
                            .child(title)
                    })
                    .with_template_modifier(|row, _| row.w_full())
                    .on_click(cx, move |this: &mut Self, cx| this.activate(workspace, page, cx))
                    .spawn(cx);
                let template = gpui_luma::controls::button::DefaultButtonTemplate::new(
                    gpui_luma_look_radix::button_family_theme_with(
                        &nav_look,
                        gpui_luma_look_radix::ButtonVariant::Ghost,
                        gpui_luma_look_radix::Paint::gray(),
                    ),
                )
                .with_modifier(move |row, model| {
                    let colors = entry_look.interaction_colors();
                    row.w_full()
                        .justify_start()
                        .text_align(gpui::TextAlign::Left)
                        .when_some(colors.background(paint_selected.load(Ordering::Relaxed), model.state), |row, color| row.bg(color))
                });
                button.update(cx, |button, cx| button.set_template(Arc::new(template), cx));
                rows.push(button.clone());
                navigation.push(NavigationEntry { workspace, page, button, selected });
            }
            items.push(
                AccordionItem::new(
                    label,
                    AccordionTrigger::new(label),
                    AccordionContent::custom(move |_, _| {
                        div().w_full().flex().flex_col().pl(px(20.)).gap_1().children(rows.clone()).into_any_element()
                    }),
                )
                .expanded(workspace == Workspace::Gradients),
            );
        }
        let accordion: Accordion = gpui_luma_look_radix::accordion_builder(&nav_look, "lab-explorer")
            .size(gpui_luma::theme::ControlSize::Sm)
            .template(nav_accordion::template(&nav_look))
            .multiple()
            .collapsible(true)
            .item_dividers(false)
            .content_padding_y(4.)
            .items(items)
            .spawn(cx);
        let content: Rc<RefCell<gpui::AnyView>> = Rc::new(RefCell::new(gradient_builder.clone().into()));
        let content_view = content.clone();
        let sidebar_look = nav_look.clone();
        let content_look = look.clone();
        let panels = gpui_luma_look_radix::resizable_panels_builder(&look, "shell-2026-panels")
            .show_border(false)
            .motion_enabled(true)
            .handle_visibility(ResizeHandleVisibility::Hover)
            .double_click_collapse(Some(ResizeCollapseBehavior::new(
                ResizeCollapseMode::Completely,
                ResizeCollapseDirection::Left,
            )))
            .panel(
                ResizablePanelSpec::new_render(move || {
                    div()
                        .id("lab-explorer-scroll")
                        .size_full()
                        .overflow_y_scroll()
                        .p(px(8.))
                        .text_color(sidebar_look.chrome().body_text)
                        .child(
                            div()
                                .px_2()
                                .py_3()
                                .text_size(px(11.))
                                .text_color(sidebar_look.chrome().muted_text)
                                .child("LAB EXPLORER"),
                        )
                        .child(accordion.clone())
                })
                .size(INITIAL_SIDEBAR_WIDTH)
                .min(MIN_SIDEBAR_WIDTH)
                .max(MAX_SIDEBAR_WIDTH),
            )
            .panel(
                ResizablePanelSpec::new_render(move || {
                    div()
                        .debug_selector(|| "shell-2026-workspace".into())
                        .size_full()
                        .min_w_0()
                        .bg(content_look.chrome().content_background)
                        .child(content_view.borrow().clone())
                })
                .weight(1.)
                .min(MIN_WORKSPACE_WIDTH),
            )
            .spawn(cx);
        let rail = spawn_rail(&nav_look, cx);
        let titlebar = TitlebarControls::new(&look, cx);
        panels.clone().on_event(cx, |this: &mut Self, event, cx| {
            if let ResizablePanelsEvent::SizesChanged { sizes_px } = event
                && let Some(width) = sizes_px.first()
            {
                this.sidebar_width = px(*width);
                cx.notify();
            }
        });
        titlebar.sidebar_toggle.clone().on_click(cx, |this: &mut Self, cx| {
            this.panels.update(cx, |panels, cx| panels.toggle_panel_hidden(0, PanelHideMode::Completely, cx));
        });
        titlebar.theme_toggle.clone().on_click(cx, |this: &mut Self, cx| {
            let mode = match this.look.mode() {
                ThemeMode::Light => ThemeMode::Dark,
                ThemeMode::Dark => ThemeMode::Light,
            };
            this.look.set_mode(mode);
            sync_color_control_theme(&this.look);
            let icon = if mode == ThemeMode::Dark {
                lucide_svg_static::Icon::Sun
            } else {
                lucide_svg_static::Icon::Moon
            };
            this.titlebar.theme_toggle.update(cx, |button, cx| {
                button.set_presenter(
                    Arc::new(move |model, _| {
                        gpui_luma::infra::icon::lucide_icon(icon, model.look.foreground, model.look.icon_size)
                    }),
                    cx,
                );
            });
            cx.bump_luma_theme_revision();
            cx.notify();
        });
        Self {
            focus_scope,
            look,
            nav_look,
            gradient_builder,
            compositions: None,
            color_gamut: None,
            navigation,
            extensions,
            active: Workspace::Gradients,
            captured_color_mode: None,
            presentation_pending: false,
            panels,
            rail,
            titlebar,
            sidebar_width: INITIAL_SIDEBAR_WIDTH,
            content,
            active_page: "gradients",
        }
    }
    fn activate(&mut self, workspace: Workspace, page: &'static str, cx: &mut Context<Self>) {
        for (id, pane) in &self.extensions {
            if *id != workspace {
                pane.update(cx, |pane, cx| pane.deactivate(cx));
            }
        }
        self.active = workspace;
        self.active_page = page;
        let view: gpui::AnyView = match workspace {
            Workspace::ColorGamut => self.color_gamut.as_ref()
                .map(|view| view.clone().into())
                .unwrap_or_else(|| self.content.borrow().clone()),
            Workspace::Gradients => {
                self.gradient_builder.update(cx, |view, cx| view.activate_page(page, cx));
                self.gradient_builder.clone().into()
            }
            Workspace::Compositions => {
                let view =
                    self.compositions.get_or_insert_with(|| cx.new(|cx| ColorCompositions::new(self.look.clone(), cx)));
                view.update(cx, |view, cx| view.activate_page(page, cx));
                view.clone().into()
            }
            _ => {
                let (_, pane) = self.extensions.iter().find(|(id, _)| *id == workspace).expect("registered workspace");
                pane.update(cx, |pane, cx| pane.activate_page(page, cx));
                pane.clone().into()
            }
        };
        *self.content.borrow_mut() = view;
        for entry in &self.navigation {
            entry.selected.store(entry.workspace == workspace && entry.page == page, Ordering::Relaxed);
            entry.button.update(cx, |_, cx| cx.notify());
        }
        self.panels.update(cx, |_, cx| cx.notify());
        cx.notify();
    }
}

// Preserve shell-2026's complete destination rail and footer.
fn spawn_rail(look: &Look, cx: &mut Context<XgpuiLabApp>) -> Entity<gpui_luma::controls::frame::FrameControl> {
    use gpui_luma::controls::sidebar::*;
    use lucide_svg_static::Icon;
    let mut menu = SidebarMenuBuilder::new("shell-2026-destinations");
    for (id, label, icon) in [
        ("home", "Home", Icon::House),
        ("history", "History", Icon::Clock),
        ("library", "Library", Icon::BookOpen),
        ("gallery", "Gallery", Icon::Image),
        ("activity", "Activity", Icon::AtSign),
        ("more", "More", Icon::Ellipsis),
    ] {
        menu = menu.item(SidebarMenuItemBuilder::new(id, label).icon(icon).active(id == "home"));
    }
    let navigation = gpui_luma_look_radix::sidebar_builder(look, "shell-2026-rail")
        .presentation(SidebarPresentation::Icons)
        .motion_enabled(false)
        .auto_hide_scrollbar(true)
        .sidebar(
            SidebarBuilder::new("shell-2026-rail-panel")
                .content(SidebarContentBuilder::new().group(SidebarGroupBuilder::new().menu(menu)))
                .footer(
                    SidebarFooterBuilder::new()
                        .child(SidebarMenuItemBuilder::new("help", "Help").icon(Icon::CircleQuestionMark))
                        .child(SidebarMenuItemBuilder::new("profile", "Profile").icon(Icon::CircleUser)),
                ),
        )
        .spawn(cx);
    // shell-2026's transparent, square sidebar frame has an 8 px inset.
    gpui_luma::controls::frame::FrameBuilder::new("shell-2026-rail-frame")
        .p(px(8.0))
        .bg(gpui::transparent_black())
        .rounded(px(0.0))
        .child(navigation)
        .spawn(cx)
}

fn sync_color_control_theme(look: &Look) {
    let chrome = look.chrome();

    set_active_color_control_theme(ColorControlTheme::new(
        chrome.border,
        chrome.panel_background,
        matches!(look.mode(), ThemeMode::Dark),
    ));
}

impl Render for XgpuiLabApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if self.active == Workspace::ColorGamut && self.color_gamut.is_none() {
            let page = _cx.new(|cx| crate::color_gamut_page::Page::new(_window, self.look.clone(), cx));
            *self.content.borrow_mut() = page.clone().into();
            self.color_gamut = Some(page);
            self.panels.update(_cx, |_, cx| cx.notify());
        }
        // Captured effects currently require SDR. Save the user's negotiated
        // request once and restore it when leaving a capture-dependent view.
        let needs_sdr = self
            .extensions
            .iter()
            .any(|(workspace, pane)| *workspace == self.active && pane.read(_cx).requires_sdr(_cx));
        if !self.presentation_pending && needs_sdr == self.captured_color_mode.is_none() {
            self.presentation_pending = true;
            _cx.defer_in(_window, move |this, window, cx| {
                if needs_sdr {
                    let previous = window.color_status().requested();
                    if window.set_color_mode(gpui::WindowColorMode::Default).is_ok() {
                        this.captured_color_mode = Some(previous);
                    }
                } else if let Some(previous) = this.captured_color_mode.take() {
                    let _ = window.set_color_mode(previous);
                }
                this.presentation_pending = false;
                cx.notify();
            });
        }
        let chrome = self.look.chrome();
        let sans_family = "System UI".to_string();

        let title_bar = self.titlebar.render(&self.look, self.sidebar_width, _window);
        let body = div()
            .size_full()
            .bg(shell_background(&self.look))
            .flex()
            .child(
                div()
                    .debug_selector(|| "shell-2026-rail-bounds".into())
                    .w(RAIL_WIDTH)
                    .flex_none()
                    .h_full()
                    .child(self.rail.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .mt(CANVAS_TOP_INSET)
                    .mr(CANVAS_EDGE_INSET)
                    .mb(CANVAS_EDGE_INSET)
                    .relative()
                    .rounded(CANVAS_RADIUS)
                    .overflow_hidden()
                    .border(CANVAS_BORDER)
                    .border_color(canvas_border_color(&self.look))
                    .bg(chrome.content_background)
                    .child(
                        div()
                            .debug_selector(|| "shell-2026-sidebar-fill".into())
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left_0()
                            .w(self.sidebar_width)
                            .rounded_tl(CANVAS_RADIUS - CANVAS_BORDER)
                            .rounded_bl(CANVAS_RADIUS - CANVAS_BORDER)
                            .bg(self.nav_look.chrome().panel_background),
                    )
                    .child(div().relative().size_full().child(self.panels.clone())),
            );
        div()
            .luma_focus_scope(&self.focus_scope)
            .size_full()
            .flex()
            .flex_col()
            .font_family(sans_family)
            .bg(shell_background(&self.look))
            .text_color(chrome.body_text)
            .child(title_bar)
            .child(div().flex_1().min_h_0().child(body))
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[test]
    fn sidebar_routes_duplicate_labels_and_retains_workspaces() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(XgpuiLabApp::new);
        cx.run_until_parked();
        let gradients = cx.update(|_, cx| view.read(cx).gradient_builder.clone());
        let mut compositions = None;
        let mut color_gamut = None;
        for (workspace, page) in [
            (Workspace::Compositions, "harmonies"),
            (Workspace::Gradients, "mesh"),
            (Workspace::Color, "Gamut Mapping"),
            (Workspace::Color, "Italy Photo Comparison"),
            (Workspace::Color, "Interactive Color Spaces"),
            (Workspace::Gamut, "Gamut Mapping"),
            (Workspace::Gamut, "Chromaticity & Gamut"),
            (Workspace::Compositions, "picker"),
            (Workspace::Gradients, "freeform"),
            (Workspace::ColorGamut, "Color & Gamut"),
            (Workspace::Gradients, "mesh"),
            (Workspace::ColorGamut, "Color & Gamut"),
        ] {
            let button = cx.update(|_, cx| {
                view.read(cx)
                    .navigation
                    .iter()
                    .find(|entry| entry.workspace == workspace && entry.page == page)
                    .unwrap()
                    .button
                    .clone()
            });
            button.update(cx, |_, cx| cx.emit(gpui_luma::controls::button::ButtonEvent::Click));
            cx.run_until_parked();
            cx.update(|_, cx| {
                let shell = view.read(cx);
                assert_eq!((shell.active, shell.active_page), (workspace, page));
                assert_eq!(shell.gradient_builder, gradients);
                if workspace == Workspace::ColorGamut {
                    let retained = shell.color_gamut.as_ref().expect("color workspace loaded");
                    if let Some(first) = &color_gamut {
                        assert_eq!(retained, first);
                    } else {
                        color_gamut = Some(retained.clone());
                    }
                }
                assert_eq!(shell.navigation.iter().filter(|entry| entry.selected.load(Ordering::Relaxed)).count(), 1);
                let retained = shell.compositions.as_ref().unwrap();
                if let Some(first) = &compositions {
                    assert_eq!(retained, first);
                } else {
                    compositions = Some(retained.clone());
                }
            });
        }
    }

    use super::titlebar::{CLOSED_TITLEBAR_WIDTH, titlebar_compartment_width};
    use gpui_luma::shell::TITLE_BAR_LEFT_PADDING;
    use gpui::{MouseButton, MouseDownEvent, VisualTestContext, point};

    fn assert_aligned(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        let separator = cx.debug_bounds("shell-2026-separator").unwrap();
        let workspace = cx.debug_bounds("shell-2026-workspace").unwrap();
        assert!(
            (separator.origin.x - workspace.origin.x).abs() <= px(1.0),
            "separator {separator:?}, workspace {workspace:?}"
        );
        assert_eq!(cx.debug_bounds("shell-2026-sidebar-fill").unwrap().right(), workspace.left());
        assert!(separator.bottom() < workspace.top(), "the two separators must remain disconnected");
        assert_eq!(cx.debug_bounds("shell-2026-rail-bounds").unwrap().size.width, RAIL_WIDTH);
        assert_eq!(cx.debug_bounds("shell-2026-rail-bounds").unwrap().top(), SHELL_TITLEBAR_HEIGHT);
    }

    #[test]
    fn rendered_separator_tracks_drag_and_restores_from_both_collapse_paths() {
        let mut app = TestAppContext::single();
        let (view, cx) = app.add_window_view(|window, cx| {
            gpui_luma::init(cx).unwrap();
            XgpuiLabApp::new(window, cx)
        });
        assert_aligned(cx);
        let workspace = cx.debug_bounds("shell-2026-workspace").unwrap();
        let start = point(workspace.left(), workspace.top() + px(100.0));
        let end = start + point(px(60.0), px(0.0));
        cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
        cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
        cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
        cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
        assert_aligned(cx);
        let resized_width = cx.update(|_, cx| view.read(cx).sidebar_width);
        assert!(resized_width > INITIAL_SIDEBAR_WIDTH, "drag must resize the sidebar");
        let samples = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let _subscription = cx.update(|_, cx| {
            let panels = view.read(cx).panels.clone();
            let samples = samples.clone();
            cx.subscribe(&panels, move |_, event: &ResizablePanelsEvent, _| {
                if let ResizablePanelsEvent::SizesChanged { sizes_px } = event {
                    samples.borrow_mut().push(sizes_px[0]);
                }
            })
        });
        let toggle = cx.debug_bounds("shell-2026-toggle-sidebar").unwrap().center();
        cx.simulate_click(toggle, Default::default());
        cx.run_until_parked();
        assert!(
            samples.borrow().iter().any(|&width| width > 0.0 && width < resized_width.as_f32()),
            "collapse must publish intermediate animated widths"
        );
        let closed_separator = cx.debug_bounds("shell-2026-separator").unwrap();
        assert_eq!(closed_separator.left(), TITLE_BAR_LEFT_PADDING + CLOSED_TITLEBAR_WIDTH - px(1.0));
        assert_eq!(cx.debug_bounds("shell-2026-workspace").unwrap().left(), RAIL_WIDTH + CANVAS_BORDER);
        assert_eq!(cx.debug_bounds("shell-2026-rail-bounds").unwrap().size.width, RAIL_WIDTH);
        cx.simulate_click(toggle, Default::default());
        cx.run_until_parked();
        assert_aligned(cx);
        assert_eq!(cx.update(|_, cx| view.read(cx).sidebar_width), resized_width);

        let workspace = cx.debug_bounds("shell-2026-workspace").unwrap();
        let seam = point(workspace.left(), workspace.top() + px(100.0));
        samples.borrow_mut().clear();
        cx.simulate_event(MouseDownEvent {
            position: seam,
            button: MouseButton::Left,
            click_count: 2,
            ..Default::default()
        });
        cx.simulate_mouse_up(seam, MouseButton::Left, Default::default());
        assert!(
            samples.borrow().iter().any(|&width| width > 0.0 && width < resized_width.as_f32()),
            "double-click collapse must also animate"
        );
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds("shell-2026-separator").unwrap(), closed_separator);
        cx.simulate_click(toggle, Default::default());
        cx.run_until_parked();
        assert_aligned(cx);
        assert_eq!(cx.update(|_, cx| view.read(cx).sidebar_width), resized_width);
    }

    #[test]
    fn separator_tracks_the_body_seam_including_native_and_fullscreen_insets() {
        for origin in [px(12.0), px(80.0), px(92.0)] {
            for width in [px(200.0), px(260.0), px(379.5), px(480.0)] {
                let separator_x = origin + titlebar_compartment_width(width, origin) - px(1.0);
                assert_eq!(separator_x, RAIL_WIDTH + CANVAS_BORDER + width);
            }
        }
    }

    #[test]
    fn closed_separator_has_a_fixed_position_after_the_navigation_controls() {
        assert_eq!(titlebar_compartment_width(px(0.0), px(80.0)), CLOSED_TITLEBAR_WIDTH);
        assert_eq!(titlebar_compartment_width(px(0.0), px(92.0)), CLOSED_TITLEBAR_WIDTH);
    }

    #[test]
    fn folder_icon_precedes_label_and_tracks_expansion_without_changing_page() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(XgpuiLabApp::new);
        cx.run_until_parked();
        let label = cx.debug_bounds("nav-label-Gradients").unwrap();
        let folder = cx.debug_bounds("nav-folder-Gradients-open").unwrap();
        assert!((label.left() - folder.right() - px(6.)).abs() < px(1.));
        let header = cx.debug_bounds("accordion-trigger-Gradients").unwrap();
        cx.simulate_click(header.center(), Default::default());
        cx.run_until_parked();
        assert!(cx.debug_bounds("nav-folder-Gradients-closed").is_some());
        cx.simulate_click(header.center(), Default::default());
        cx.run_until_parked();
        assert!(cx.debug_bounds("nav-folder-Gradients-open").is_some());
        cx.update(|_, cx| {
            let shell = view.read(cx);
            assert_eq!((shell.active, shell.active_page), (Workspace::Gradients, "gradients"));
        });
    }

    #[test]
    fn theme_toggle_switches_radix_and_keeps_selected_page() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(XgpuiLabApp::new);
        cx.run_until_parked();
        let toggle = cx.update(|_, cx| view.read(cx).titlebar.theme_toggle.clone());
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            toggle.update(cx, |_, cx| cx.emit(gpui_luma::controls::button::ButtonEvent::Click));
            cx.run_until_parked();
            cx.update(|_, cx| {
                let shell = view.read(cx);
                assert_eq!(shell.look.mode(), mode);
                assert_eq!((shell.active, shell.active_page), (Workspace::Gradients, "gradients"));
            });
        }
    }

    #[test]
    fn sidebar_toggle_retains_width_and_selected_content() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(XgpuiLabApp::new);
        cx.run_until_parked();
        let (split, toggle) = cx.update(|_, cx| {
            let shell = view.read(cx);
            (shell.panels.clone(), shell.titlebar.sidebar_toggle.clone())
        });
        let width = cx.update(|_, cx| split.read(cx).panel_sizes_px()[0]);
        for collapsed in [true, false] {
            toggle.update(cx, |_, cx| cx.emit(gpui_luma::controls::button::ButtonEvent::Click));
            cx.run_until_parked();
            cx.update(|_, cx| {
                assert_eq!(split.read(cx).is_panel_hidden(0), collapsed);
                if !collapsed {
                    assert!((split.read(cx).panel_sizes_px()[0] - width).abs() < 1.);
                }
                let shell = view.read(cx);
                assert_eq!((shell.active, shell.active_page), (Workspace::Gradients, "gradients"));
                assert!(shell.compositions.is_none());
            });
        }
    }
}
