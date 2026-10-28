//! Path fixtures and comparison panels adapted from media-lab/src/paths.rs.
use anyhow::Result;
use gpui::{prelude::*, *};
use xgpui_svg::path_effects::{Contour, MeasuredPath, PathEffect};

pub fn samples() -> Result<Vec<MeasuredPath>> {
    let p = |x, y| point(px(x), px(y));
    [
        vec![
            Contour::new(p(20., 100.))
                .cubic_to(p(60., -30.), p(140., 210.), p(190., 85.))
                .quadratic_to(p(250., -20.), p(300., 100.)),
        ],
        vec![
            Contour::new(p(160., 150.))
                .cubic_to(p(120., 115.), p(20., 60.), p(70., 30.))
                .cubic_to(p(105., 5.), p(145., 20.), p(160., 50.))
                .cubic_to(p(175., 20.), p(215., 5.), p(250., 30.))
                .cubic_to(p(300., 60.), p(200., 115.), p(160., 150.))
                .close(),
        ],
        vec![
            Contour::new(p(20., 45.)).line_to(p(110., 45.)).line_to(p(110., 120.)).line_to(p(20., 120.)).close(),
            Contour::new(p(170., 120.)).quadratic_to(p(170., 5.), p(230., 45.)).cubic_to(
                p(300., -5.),
                p(320., 120.),
                p(250., 135.),
            ),
        ],
    ]
    .into_iter()
    .map(|c| MeasuredPath::new(&c, px(0.05)))
    .collect()
}
pub(super) fn effect(trim: f32, dash: usize, phase: f32) -> Result<PathEffect> {
    let e = PathEffect::new().trim(0. ..trim)?;
    match dash {
        0 => Ok(e),
        1 => e.dash(&[px(16.), px(8.)], px(phase)),
        2 => e.dash(&[px(4.), px(7.)], px(phase)),
        _ => e.dash(&[px(18.), px(5.), px(3.)], px(phase)),
    }
}
fn place(mut path: Path<Pixels>, offset: Point<Pixels>, scale: f32) -> Path<Pixels> {
    for v in &mut path.vertices {
        v.xy_position = offset + v.xy_position * scale;
    }
    path.bounds.origin = offset + path.bounds.origin * scale;
    path.bounds.size = path.bounds.size.map(|v| v * scale);
    path
}
pub(super) fn panel(path: &MeasuredPath, effect: PathEffect, width: f32, color: u32, height: f32) -> Div {
    let built = path.stroke(&effect, px(width));
    let error = built.as_ref().err().map(ToString::to_string);
    let built = built.ok();
    div()
        .flex_1()
        .min_w_0()
        .h(px(height))
        .rounded_lg()
        .bg(rgb(0x191f23))
        .overflow_hidden()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    if let Some(path) = &built {
                        let scale = (f32::from(bounds.size.width) / 340.).min(f32::from(bounds.size.height) / 180.);
                        let offset = bounds.origin
                            + point(
                                (bounds.size.width - px(320. * scale)) * 0.5,
                                (bounds.size.height - px(160. * scale)) * 0.5,
                            );
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            window.paint_path(place(path.clone(), offset, scale), rgb(color))
                        });
                    }
                },
            )
            .size_full(),
        )
        .children(error.map(|e| div().text_sm().text_color(rgb(0xffa99c)).child(e)))
}
