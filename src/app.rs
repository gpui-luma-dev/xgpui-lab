use gpui::{Context, Entity, FocusHandle, Render, Subscription, Window, div, prelude::*, px};
use gpui_luma_color::style::{ColorControlTheme, set_active_color_control_theme};
use gpui_luma::focus::LumaFocusScopeExt;
use gpui_luma::shell::TitleBar;
use gpui_luma::theme::ThemeMode;
use gpui_luma_look_radix::Look;
use crate::theme::ColorVizLookExt;
use std::sync::Arc;
use gpui_luma::prelude::TooltipEntityExt;

use crate::gradient_builder::GradientBuilder;
use crate::compositions::ColorCompositions;
use crate::extension_pane::ExtensionPane;

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

pub struct XgpuiLabApp {
    focus_scope: FocusHandle,
    look: Arc<Look>,
    gradient_builder: Entity<GradientBuilder>,
    compositions: Option<Entity<ColorCompositions>>,
    color_gamut: Option<Entity<crate::color_gamut_page::Page>>,
    navigation: Vec<(Workspace, Entity<gpui_luma::controls::button::Button>)>,
    extensions: Vec<(Workspace, Entity<ExtensionPane>)>,
    active: Workspace,
    captured_color_mode: Option<gpui::WindowColorMode>,
    presentation_pending: bool,
    _subscriptions: Vec<Subscription>,
}

impl XgpuiLabApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let p3_error = window.set_color_mode(gpui::WindowColorMode::WideGamutSdr).err().map(|error| error.to_string());
        let focus_scope = cx.focus_handle();
        let look = crate::theme::default_look();
        sync_color_control_theme(&look);
        let gradient_builder = cx.new(|cx| GradientBuilder::new(look.clone(), cx));
        let extensions = [
            (
                Workspace::Color,
                "xgpui-color-tabs",
                &["Color Spaces", "Conversions", "Interpolation", "Gamut Mapping", "CPU Gradients"][..],
            ),
            (
                Workspace::Gamut,
                "xgpui-gamut-tabs",
                &["Wide Color Fills", "Wide Gradients", "Gamut Mapping", "Presentation", "Benchmarks"][..],
            ),
            (
                Workspace::Media,
                "xgpui-media-tabs",
                &[
                    "Images",
                    "Textures & Mipmaps",
                    "Image Filters",
                    "Linear Adjustments",
                    "Float / P3",
                    "Tiled Canvas",
                    "Video Frames",
                    "Resource Limits",
                    "Benchmarks",
                ][..],
            ),
            (
                Workspace::Parley,
                "xgpui-parley-tabs",
                &[
                    "Text Shaping",
                    "Line Layout",
                    "Paragraphs",
                    "Fonts & Fallback",
                    "Rasterization",
                    "Scrolling",
                    "Benchmarks",
                ][..],
            ),
            (
                Workspace::Shader,
                "xgpui-shader-tabs",
                &[
                    "Gradients",
                    "Mesh Editor",
                    "Gradient Paints",
                    "WGSL Paints",
                    "Shader Depot: Grid",
                    "Shadertoy: Seascape",
                    "Instancing",
                    "Typography",
                    "Brushes",
                    "Layer Effects",
                    "Shadows",
                    "Borders",
                    "Benchmarks",
                ][..],
            ),
            (
                Workspace::Svg,
                "xgpui-svg-tabs",
                &["Rendering", "Scaling", "Path Effects", "Scheduling & Cache", "Benchmarks"][..],
            ),
        ]
        .map(|(workspace, id, labels)| {
            (workspace, cx.new(|cx| ExtensionPane::new(id, labels, look.clone(), p3_error.clone(), cx)))
        })
        .to_vec();
        let navigation = [
            (Workspace::Gradients, "Gradients", lucide_svg_static::Icon::Blend),
            (Workspace::Compositions, "Compositions", lucide_svg_static::Icon::Palette),
            (Workspace::ColorGamut, "Color & Gamut", lucide_svg_static::Icon::ChartNoAxesCombined),
            (Workspace::Color, "xgpui-color", lucide_svg_static::Icon::Pipette),
            (Workspace::Gamut, "xgpui-gamut", lucide_svg_static::Icon::SwatchBook),
            (Workspace::Media, "xgpui-media", lucide_svg_static::Icon::Film),
            (Workspace::Parley, "xgpui-parley", lucide_svg_static::Icon::TypeIcon),
            (Workspace::Shader, "xgpui-shader", lucide_svg_static::Icon::Sparkles),
            (Workspace::Svg, "xgpui-svg", lucide_svg_static::Icon::Shapes),
        ]
        .map(|(workspace, label, icon)| {
            let button = crate::theme::icon_button(format!("xgpui-nav-{label}"), icon)
                .look(&look)
                .content_only()
                .spawn(cx)
                .tooltip(gpui_luma::controls::tooltip::Tooltip::new(label), cx);
            (workspace, button)
        })
        .to_vec();
        let mut subscriptions = Vec::new();
        for (workspace, button) in &navigation {
            let workspace = *workspace;
            subscriptions.push(cx.subscribe(
                button,
                move |this, _, event: &gpui_luma::controls::button::ButtonEvent, cx| {
                    if event.is_click() {
                        for (id, pane) in &this.extensions {
                            if *id != workspace {
                                pane.update(cx, |pane, cx| pane.deactivate(cx));
                            }
                        }
                        this.active = workspace;
                        if workspace == Workspace::Compositions && this.compositions.is_none() {
                            this.compositions = Some(cx.new(|cx| ColorCompositions::new(this.look.clone(), cx)));
                        }
                        cx.notify();
                    }
                },
            ));
        }
        Self {
            focus_scope,
            look,
            gradient_builder,
            compositions: None,
            color_gamut: None,
            navigation,
            extensions,
            active: Workspace::Gradients,
            captured_color_mode: None,
            presentation_pending: false,
            _subscriptions: subscriptions,
        }
    }
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
        if self.active == Workspace::ColorGamut && self.color_gamut.is_none() {
            self.color_gamut = Some(_cx.new(|cx| crate::color_gamut_page::Page::new(_window, self.look.clone(), cx)));
        }
        let chrome = self.look.chrome();
        let sans_family = "System UI".to_string();

        let export_button = self.gradient_builder.read(_cx).export_button();
        let title_bar = TitleBar::new()
            .height(crate::app_shell::SHELL_TITLEBAR_HEIGHT)
            .background_color(chrome.app_background)
            .border_color(chrome.app_background)
            .text_color(chrome.title_text)
            .child(
                div()
                    .id("color-viz-titlebar")
                    .h_full()
                    .w_full()
                    .flex()
                    .items_center()
                    .px_2()
                    .text_color(chrome.title_text)
                    .font_family(sans_family.clone())
                    .child(div().text_size(px(14.0)).font_weight(gpui::FontWeight::SEMIBOLD).child("XGPUI Lab"))
                    .child(div().flex_1())
                    .when(self.active == Workspace::Gradients, |this| {
                        this.child(
                            div()
                                .mr(px(12.0))
                                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .child(export_button),
                        )
                    }),
            );

        div()
            .luma_focus_scope(&self.focus_scope)
            .size_full()
            .flex()
            .flex_col()
            .font_family(sans_family)
            .bg(chrome.app_background)
            .text_color(chrome.body_text)
            .child(title_bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .pt(px(2.0))
                    .pr(px(3.0))
                    .pb(px(3.0))
                    .child(
                        div()
                            .w(px(52.0))
                            .flex_shrink_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .pt_2()
                            .children(self.navigation.iter().map(|(workspace, button)| {
                                div()
                                    .rounded(px(6.0))
                                    .when(self.active == *workspace, |rail| rail.bg(chrome.border))
                                    .child(button.clone())
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .rounded(px(10.0))
                            .border_1()
                            .border_color(chrome.border)
                            .overflow_hidden()
                            .bg(chrome.content_background)
                            .child(match (self.active, &self.compositions) {
                                (Workspace::Compositions, Some(view)) => view.clone().into_any_element(),
                                (Workspace::ColorGamut, _) => self
                                    .color_gamut
                                    .as_ref()
                                    .map(|view| view.clone().into_any_element())
                                    .unwrap_or_else(|| div().into_any_element()),
                                (Workspace::Gradients, _) => self.gradient_builder.clone().into_any_element(),
                                _ => self
                                    .extensions
                                    .iter()
                                    .find(|(workspace, _)| *workspace == self.active)
                                    .map(|(_, pane)| pane.clone().into_any_element())
                                    .unwrap_or_else(|| div().size_full().into_any_element()),
                            }),
                    ),
            )
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[test]
    fn workspace_navigation_loads_compositions_once_and_retains_both_workspaces() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(XgpuiLabApp::new);
        cx.run_until_parked();
        let (gradient_nav, composition_nav, gradients) = cx.update(|_, cx| {
            let view = view.read(cx);
            assert!(view.compositions.is_none());
            (
                view.navigation.iter().find(|(id, _)| *id == Workspace::Gradients).unwrap().1.clone(),
                view.navigation.iter().find(|(id, _)| *id == Workspace::Compositions).unwrap().1.clone(),
                view.gradient_builder.clone(),
            )
        });
        let mut first_compositions = None;
        for id in ["compositions", "gradients", "compositions"] {
            let button = if id == "compositions" {
                &composition_nav
            } else {
                &gradient_nav
            };
            button.update(cx, |_, cx| cx.emit(gpui_luma::controls::button::ButtonEvent::Click));
            cx.run_until_parked();
            cx.update(|_, cx| {
                let view = view.read(cx);
                assert_eq!(view.active == Workspace::Compositions, id == "compositions");
                assert_eq!(view.gradient_builder, gradients);
                let compositions = view.compositions.as_ref().expect("loaded compositions");
                if let Some(first) = &first_compositions {
                    assert_eq!(compositions, first);
                } else {
                    first_compositions = Some(compositions.clone());
                }
            });
        }
    }
    #[test]
    fn shell_rail_routes_to_retained_workspaces() {
        let mut app = TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).expect("initialize SDK"));
        let (view, cx) = app.add_window_view(XgpuiLabApp::new);
        cx.run_until_parked();
        let navigation = cx.update(|_, cx| view.read(cx).navigation.clone());
        let mut retained_color_gamut = None;
        for (expected, button) in navigation.iter().chain(navigation.iter().rev()) {
            button.update(cx, |_, cx| cx.emit(gpui_luma::controls::button::ButtonEvent::Click));
            cx.run_until_parked();
            cx.update(|_, cx| {
                let state = view.read(cx);
                assert_eq!(state.active, *expected);
                if *expected == Workspace::ColorGamut {
                    let color = state.color_gamut.as_ref().expect("color workspace loaded");
                    if let Some(first) = &retained_color_gamut {
                        assert_eq!(color, first);
                    } else {
                        retained_color_gamut = Some(color.clone());
                    }
                }
            });
        }
    }
}
