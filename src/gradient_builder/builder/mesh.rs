use gpui::{Corners, Pixels, SharedString, Size, div, prelude::*, px, size};
use gpui_luma::controls::selector::SelectorItem;
use gpui_luma::controls::tabs::TabsItem;
use gpui_luma::theme::LumaTextStyle;
use crate::theme::TypographyExt;

use super::super::paint::{GradientType, MeshPoint};
use super::super::paint::color_at_position;

pub(super) const SHELL_RADIUS: f32 = 0.0;
pub(super) const SHELL_BORDER: f32 = 1.0;
pub(super) const MESH_HANDLE_SIZE: f32 = 44.0;
pub(super) const MESH_POINT_GAP: f32 = 0.08;
pub(super) const MESH_ASPECT_INSET: f32 = 12.0;
pub(super) const MESH_PREVIEW_PADDING: f32 = 12.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum BuilderTab {
    #[default]
    Gradients,
    Mesh,
    Freeform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MeshColorTarget {
    Point(usize),
    Background,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MeshGridPreset {
    SampleThreeByFour,
    TwoByTwo,
    ThreeByThree,
    FourByFour,
}

impl MeshGridPreset {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::SampleThreeByFour => "3 x 4",
            Self::TwoByTwo => "2 x 2",
            Self::ThreeByThree => "3 x 3",
            Self::FourByFour => "4 x 4",
        }
    }

    pub(super) fn dimensions(self) -> (usize, usize) {
        match self {
            Self::SampleThreeByFour => (3, 4),
            Self::TwoByTwo => (2, 2),
            Self::ThreeByThree => (3, 3),
            Self::FourByFour => (4, 4),
        }
    }

    pub(super) fn from_item_id(item_id: &str) -> Self {
        match item_id {
            "2x2" => Self::TwoByTwo,
            "3x3" => Self::ThreeByThree,
            "4x4" => Self::FourByFour,
            _ => Self::SampleThreeByFour,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MeshAspectRatioPreset {
    Fill,
    NineByNineteen,
    ThreeByFour,
    OneByOne,
    TwoByThree,
}

impl MeshAspectRatioPreset {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Fill => "Full",
            Self::NineByNineteen => "9:19",
            Self::ThreeByFour => "3:4",
            Self::OneByOne => "1:1",
            Self::TwoByThree => "2:3",
        }
    }

    pub(super) fn ratio(self) -> Option<f32> {
        match self {
            Self::Fill => None,
            Self::NineByNineteen => Some(9.0 / 19.0),
            Self::ThreeByFour => Some(3.0 / 4.0),
            Self::OneByOne => Some(1.0),
            Self::TwoByThree => Some(2.0 / 3.0),
        }
    }

    pub(super) fn from_item_id(item_id: &str) -> Self {
        match item_id {
            "fill" => Self::Fill,
            "9:19" => Self::NineByNineteen,
            "1:1" => Self::OneByOne,
            "2:3" => Self::TwoByThree,
            _ => Self::ThreeByFour,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PreviewStopKey {
    pub(super) position_millis: u16,
    pub(super) red: u8,
    pub(super) green: u8,
    pub(super) blue: u8,
    pub(super) alpha: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PreviewMeshPointKey {
    pub(super) row: u8,
    pub(super) col: u8,
    pub(super) u_millis: u16,
    pub(super) v_millis: u16,
    pub(super) red: u8,
    pub(super) green: u8,
    pub(super) blue: u8,
    pub(super) alpha: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PreviewImageCacheKey {
    Gradient {
        gradient_type: GradientType,
        width_px: u16,
        height_px: u16,
        rotation_tenths: u16,
        stops: Vec<PreviewStopKey>,
    },
    Mesh {
        width_px: u16,
        height_px: u16,
        background_red: u8,
        background_green: u8,
        background_blue: u8,
        background_alpha: u8,
        points: Vec<PreviewMeshPointKey>,
    },
}

fn shell_inner_corner_radius() -> Pixels {
    px((SHELL_RADIUS - SHELL_BORDER).max(0.0))
}

pub(super) fn controls_panel_corner_radii() -> Corners<Pixels> {
    let inner = shell_inner_corner_radius();
    Corners { top_left: px(0.0), top_right: px(0.0), bottom_left: inner, bottom_right: px(0.0) }
}

pub(super) fn preview_panel_corner_radii() -> Corners<Pixels> {
    let inner = shell_inner_corner_radius();
    Corners { top_left: px(0.0), top_right: px(0.0), bottom_left: px(0.0), bottom_right: inner }
}

pub(super) fn mesh_preview_content_size(container_size: Size<Pixels>) -> Size<Pixels> {
    size(
        px((container_size.width.as_f32() - MESH_PREVIEW_PADDING * 2.0).max(0.0)),
        px((container_size.height.as_f32() - MESH_PREVIEW_PADDING * 2.0).max(0.0)),
    )
}

pub(super) fn type_items() -> Vec<SelectorItem> {
    vec![
        SelectorItem::new("linear").label("Linear"),
        SelectorItem::new("radial").label("Radial"),
        SelectorItem::new("conic").label("Conic"),
    ]
}

pub(super) fn renderer_items() -> Vec<SelectorItem> {
    vec![
        SelectorItem::new("quads").label("Quads"),
        SelectorItem::new("render").label("Render (Sync)"),
        SelectorItem::new("render_async").label("Render (Async)"),
        SelectorItem::new("shader_paint").label("Shader paint"),
        SelectorItem::new("shader_brush").label("Shader brush"),
    ]
}

pub(super) fn mesh_grid_items() -> Vec<SelectorItem> {
    vec![
        SelectorItem::new("3x4").label("3 x 4"),
        SelectorItem::new("2x2").label("2 x 2"),
        SelectorItem::new("3x3").label("3 x 3"),
        SelectorItem::new("4x4").label("4 x 4"),
    ]
}

pub(super) fn mesh_aspect_ratio_items() -> Vec<SelectorItem> {
    vec![
        SelectorItem::new("fill").label("Full"),
        SelectorItem::new("9:19").label("9:19"),
        SelectorItem::new("3:4").label("3:4"),
        SelectorItem::new("1:1").label("1:1"),
        SelectorItem::new("2:3").label("2:3"),
    ]
}

pub(super) fn builder_tab_items() -> Vec<TabsItem> {
    vec![
        TabsItem::new("gradients").label("Linear"),
        TabsItem::new("mesh").label("Mesh"),
        TabsItem::new("freeform").label("Freeform"),
    ]
}

pub(super) fn preview_gradient_cache_key(
    gradient_type: GradientType,
    preview_size: Size<Pixels>,
    rotation_deg: f32,
    stops: &[(f32, gpui::Hsla)],
) -> PreviewImageCacheKey {
    PreviewImageCacheKey::Gradient {
        gradient_type,
        width_px: preview_size.width.as_f32().round().clamp(0.0, u16::MAX as f32) as u16,
        height_px: preview_size.height.as_f32().round().clamp(0.0, u16::MAX as f32) as u16,
        rotation_tenths: (rotation_deg.rem_euclid(360.0) * 10.0).round().clamp(0.0, u16::MAX as f32) as u16,
        stops: stops.iter().map(|(position, color)| preview_stop_key(*position, *color)).collect(),
    }
}

pub(super) fn preview_mesh_cache_key(
    preview_size: Size<Pixels>,
    points: &[MeshPoint],
    background: gpui::Hsla,
) -> PreviewImageCacheKey {
    let background_rgb = background.to_rgb();
    PreviewImageCacheKey::Mesh {
        width_px: preview_size.width.as_f32().round().clamp(0.0, u16::MAX as f32) as u16,
        height_px: preview_size.height.as_f32().round().clamp(0.0, u16::MAX as f32) as u16,
        background_red: (background_rgb.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        background_green: (background_rgb.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        background_blue: (background_rgb.b.clamp(0.0, 1.0) * 255.0).round() as u8,
        background_alpha: (background.a.clamp(0.0, 1.0) * 255.0).round() as u8,
        points: points
            .iter()
            .map(|point| {
                let rgb = point.color.to_rgb();
                PreviewMeshPointKey {
                    row: point.row,
                    col: point.col,
                    u_millis: (point.u.clamp(0.0, 1.0) * 1000.0).round() as u16,
                    v_millis: (point.v.clamp(0.0, 1.0) * 1000.0).round() as u16,
                    red: (rgb.r.clamp(0.0, 1.0) * 255.0).round() as u8,
                    green: (rgb.g.clamp(0.0, 1.0) * 255.0).round() as u8,
                    blue: (rgb.b.clamp(0.0, 1.0) * 255.0).round() as u8,
                    alpha: (point.color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
                }
            })
            .collect(),
    }
}

fn preview_stop_key(position: f32, color: gpui::Hsla) -> PreviewStopKey {
    let rgb = color.to_rgb();
    PreviewStopKey {
        position_millis: (position.clamp(0.0, 1.0) * 1000.0).round() as u16,
        red: (rgb.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        green: (rgb.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        blue: (rgb.b.clamp(0.0, 1.0) * 255.0).round() as u8,
        alpha: (color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    }
}

pub(super) fn mesh_point_index(row: usize, col: usize, cols: usize) -> usize {
    row * cols + col
}

// Shared starting colors for structured and freeform gradients.
pub(super) fn default_point_palette() -> [gpui::Hsla; 3] {
    [
        gpui::hsla(46.0 / 360.0, 1.0, 0.51, 1.0),
        gpui::hsla(349.0 / 360.0, 1.0, 0.58, 1.0),
        gpui::hsla(212.0 / 360.0, 0.86, 0.49, 1.0),
    ]
}

pub(super) const MESH_PALETTE_NAMES: [&str; 5] = ["Sunset", "Lagoon", "Orchid", "Citrus", "Aurora"];

pub(super) fn default_mesh_palette(preset: MeshGridPreset) -> usize {
    match preset {
        MeshGridPreset::SampleThreeByFour => 0,
        MeshGridPreset::TwoByTwo => 1,
        MeshGridPreset::ThreeByThree => 2,
        MeshGridPreset::FourByFour => 3,
    }
}

pub(super) fn apply_mesh_palette(points: &mut [MeshPoint], palette: usize) {
    const COLORS: [[u32; 4]; 5] = [
        [0xffcb69, 0xf28482, 0x7b2cbf, 0x240046],
        [0x023e8a, 0x00b4d8, 0x007f5f, 0xcaffbf],
        [0x3c096c, 0xf72585, 0x4361ee, 0xffafcc],
        [0x386641, 0xa7c957, 0xff7b00, 0xffe169],
        [0x10002b, 0x00f5d4, 0x7b2cbf, 0xfee440],
    ];
    let colors = COLORS[palette % COLORS.len()].map(|hex| gpui::Hsla::from(gpui::rgb(hex)));
    let Some((rows, cols)) = super::super::paint::mesh_dimensions(points) else { return; };
    for point in points {
        let u = point.col as f32 / (cols - 1) as f32;
        let v = point.row as f32 / (rows - 1) as f32;
        let top = color_at_position(&[(0., colors[0]), (1., colors[1])], u);
        let bottom = color_at_position(&[(0., colors[2]), (1., colors[3])], u);
        point.color = color_at_position(&[(0., top), (1., bottom)], v);
    }
}

pub(super) fn blend_mesh_palettes(points: &mut [MeshPoint], palette: usize, phase: f32) {
    apply_mesh_palette(points, palette);
    if phase <= 0. { return; }
    let mut next = points.to_vec();
    apply_mesh_palette(&mut next, (palette + 1) % MESH_PALETTE_NAMES.len());
    for (point, next) in points.iter_mut().zip(next) {
        point.color = color_at_position(&[(0., point.color), (1., next.color)], phase.clamp(0., 1.));
    }
}

pub(super) fn default_mesh_points(preset: MeshGridPreset) -> Vec<MeshPoint> {
    let (rows, cols) = preset.dimensions();
    let mut points: Vec<_> = (0..rows)
        .flat_map(|row| {
            (0..cols).map(move |col| MeshPoint {
                row: row as u8, col: col as u8,
                u: col as f32 / (cols - 1) as f32,
                v: row as f32 / (rows - 1) as f32,
                color: gpui::white(),
            })
        })
        .collect();
    apply_mesh_palette(&mut points, default_mesh_palette(preset));
    points
}

pub(super) fn fit_aspect_ratio(container: Size<Pixels>, aspect_preset: MeshAspectRatioPreset) -> Size<Pixels> {
    let container_width = container.width.as_f32().max(0.0);
    let container_height = container.height.as_f32().max(0.0);
    if container_width <= 0.0 || container_height <= 0.0 {
        return size(px(0.0), px(0.0));
    }

    let Some(ratio) = aspect_preset.ratio() else {
        let fill_margin = (MESH_ASPECT_INSET - MESH_HANDLE_SIZE * 0.5).max(0.0);
        return size(
            px((container_width - MESH_HANDLE_SIZE - fill_margin * 2.0).max(0.0)),
            px((container_height - MESH_HANDLE_SIZE - fill_margin * 2.0).max(0.0)),
        );
    };
    if ratio <= f32::EPSILON {
        return size(px(0.0), px(0.0));
    }

    let inset_width = (container_width - MESH_ASPECT_INSET * 2.0).max(0.0);
    let inset_height = (container_height - MESH_ASPECT_INSET * 2.0).max(0.0);
    if inset_width <= 0.0 || inset_height <= 0.0 {
        return size(px(0.0), px(0.0));
    }

    let container_ratio = inset_width / inset_height.max(f32::EPSILON);
    if container_ratio > ratio {
        size(px(inset_height * ratio), px(inset_height))
    } else {
        size(px(inset_width), px(inset_width / ratio))
    }
}

pub(super) fn default_mesh_background() -> gpui::Hsla {
    gpui::hsla(222.0 / 360.0, 0.22, 0.12, 1.0)
}

pub(super) fn render_info_row(
    label: &'static str,
    value: impl Into<SharedString>,
    label_style: LumaTextStyle,
    value_style: LumaTextStyle,
    chrome: gpui_luma::theme::LumaChrome,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(div().typography_style(label_style).text_color(chrome.muted_text).child(label))
        .child(div().typography_style(value_style).text_color(chrome.body_text).child(value.into()))
}

#[cfg(test)]
mod palette_tests {
    use super::*;
    #[test]
    fn grid_defaults_differ_and_five_palettes_preserve_geometry() {
        let presets = [MeshGridPreset::SampleThreeByFour, MeshGridPreset::TwoByTwo, MeshGridPreset::ThreeByThree, MeshGridPreset::FourByFour];
        let defaults: Vec<_> = presets.iter().map(|preset| default_mesh_points(*preset)[0].color).collect();
        for (i, color) in defaults.iter().enumerate() {
            assert!(!defaults[..i].contains(color));
        }
        for preset in presets {
            let mut points = default_mesh_points(preset);
            points[0].u = 0.09;
            let positions: Vec<_> = points.iter().map(|p| (p.row, p.col, p.u, p.v)).collect();
            let first = points.clone();
            let mut palettes = Vec::new();
            for step in 1..=5 {
                apply_mesh_palette(&mut points, (default_mesh_palette(preset) + step) % 5);
                assert_eq!(points.iter().map(|p| (p.row, p.col, p.u, p.v)).collect::<Vec<_>>(), positions);
                assert!(points.iter().all(|p| p.color.a == 1.));
                let colors: Vec<_> = points.iter().map(|p| p.color).collect();
                assert!(!palettes.contains(&colors));
                palettes.push(colors);
            }
            assert_eq!(points, first);
        }
    }
}

#[cfg(all(test, feature = "test-support"))]
mod animation_tests {
    use super::super::*;
    #[test]
    fn palette_animation_blends_wraps_and_preserves_mesh_edits() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (view, cx) = app.add_window_view(|_, cx| GradientBuilder::new(crate::theme::default_look(), cx));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.selected_tab = BuilderTab::Mesh;
            view.mesh_palette = 4;
            view.mesh_palette_speed = 0.5;
            view.mesh_points[0].u = 0.08;
            let positions: Vec<_> = view.mesh_points.iter().map(|p| (p.u, p.v)).collect();
            let ids = view.mesh_point_ids.clone();
            let mut start = view.mesh_points.clone();
            mesh::apply_mesh_palette(&mut start, 4);
            let mut end = start.clone();
            mesh::apply_mesh_palette(&mut end, 0);
            view.advance_mesh_palette(1.);
            assert_eq!(view.mesh_palette, 4);
            assert_eq!(view.mesh_palette_phase, 0.5);
            let expected = super::super::super::paint::color_at_position(&[(0., start[0].color), (1., end[0].color)], 0.5);
            assert_eq!(view.mesh_points[0].color, expected);
            view.advance_mesh_palette(1.);
            assert_eq!(view.mesh_palette, 0);
            assert_eq!(view.mesh_palette_phase, 0.);
            assert_eq!(view.mesh_points, end);
            assert_eq!(view.mesh_point_ids, ids);
            assert_eq!(view.mesh_points.iter().map(|p| (p.u, p.v)).collect::<Vec<_>>(), positions);
        });
    }
}
