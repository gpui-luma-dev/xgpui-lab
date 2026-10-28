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
        SelectorItem::new("angular").label("Angular"),
    ]
}

pub(super) fn renderer_items() -> Vec<SelectorItem> {
    vec![
        SelectorItem::new("quads").label("Quads"),
        SelectorItem::new("render").label("Render (Sync)"),
        SelectorItem::new("render_async").label("Render (Async)"),
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

pub(super) fn default_mesh_points(preset: MeshGridPreset) -> Vec<MeshPoint> {
    let (rows, cols) = preset.dimensions();
    let [top, middle, bottom] = default_point_palette();
    let row_colors = [(0.0, top), (0.5, middle), (1.0, bottom)];

    (0..rows)
        .flat_map(|row| {
            let v = if rows == 1 { 0.0 } else { row as f32 / (rows - 1) as f32 };
            let color = color_at_position(&row_colors, v);
            (0..cols).map(move |col| {
                let u = if cols == 1 { 0.0 } else { col as f32 / (cols - 1) as f32 };
                MeshPoint { row: row as u8, col: col as u8, u, v, color }
            })
        })
        .collect()
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
