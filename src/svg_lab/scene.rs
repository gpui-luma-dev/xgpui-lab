use gpui::*;

pub const WIDTH: f32 = 320.;
pub const HEIGHT: f32 = 240.;

// One source for SVG path data and native GPUI geometry.
#[derive(Clone, Copy)]
enum Segment {
    Move(f32, f32),
    Line(f32, f32),
    Cubic([f32; 6]),
    Close,
}
use Segment::*;
const RIBBON: &[Segment] = &[
    Move(-15., 165.),
    Cubic([45., 25., 105., 225., 160., 90.]),
    Cubic([210., -20., 265., 155., 340., 40.]),
    Line(340., 125.),
    Cubic([255., 235., 215., 65., 165., 165.]),
    Cubic([100., 290., 45., 95., -15., 220.]),
    Close,
];
const LEAF: &[Segment] = &[
    Move(105., 165.),
    Cubic([80., 55., 170., 35., 225., 55.]),
    Cubic([235., 125., 170., 190., 105., 165.]),
    Close,
];
const VEIN: &[Segment] = &[
    Move(108., 162.),
    Cubic([133., 122., 173., 92., 213., 65.]),
    Move(139., 122.),
    Cubic([129., 101., 130., 83., 139., 70.]),
    Move(162., 100.),
    Cubic([180., 107., 193., 109., 207., 104.]),
];
fn svg_path(segments: &[Segment]) -> String {
    let mut result = String::new();
    for segment in segments {
        result.push_str(&match segment {
            Move(x, y) => format!("M{x} {y}"),
            Line(x, y) => format!("L{x} {y}"),
            Cubic([a, b, c, d, x, y]) => format!("C{a} {b} {c} {d} {x} {y}"),
            Close => "Z".into(),
        });
    }
    result
}

pub fn svg(clipped: bool, shadow: bool) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="320" height="240" viewBox="0 0 320 240">
<defs>
 <linearGradient id="r"><stop stop-color="#7761ff"/><stop offset="1" stop-color="#f478b1"/></linearGradient>
 <linearGradient id="l"><stop stop-color="#3ce6c0"/><stop offset="1" stop-color="#187ea5"/></linearGradient>
 <clipPath id="clip"><rect x="30" y="30" width="260" height="180"/></clipPath>
 <filter id="shadow" x="-50%" y="-50%" width="200%" height="200%"><feDropShadow dx="0" dy="8" stdDeviation="5" flood-color="#020617" flood-opacity="0.65"/></filter>
</defs>
<g {}>
 <path d="{}" fill="url(#r)"/>
 <path d="{}" fill="url(#l)" {}/>
 <path d="{}" fill="none" stroke="#d6fff4" stroke-width="1.3"/>
</g></svg>"##,
        if clipped { "clip-path=\"url(#clip)\"" } else { "" },
        svg_path(RIBBON),
        svg_path(LEAF),
        if shadow { "filter=\"url(#shadow)\"" } else { "" },
        svg_path(VEIN),
    )
}
fn path(segments: &[Segment], origin: Point<Pixels>, zoom: f32, stroke: bool) -> Path<Pixels> {
    let mut p = if stroke {
        PathBuilder::stroke(px(1.3 * zoom))
    } else {
        PathBuilder::fill()
    };
    let pt = |x, y| origin + point(px(x * zoom), px(y * zoom));
    for segment in segments {
        match *segment {
            Move(x, y) => p.move_to(pt(x, y)),
            Line(x, y) => p.line_to(pt(x, y)),
            Cubic([a, b, c, d, x, y]) => p.cubic_bezier_to(pt(x, y), pt(a, b), pt(c, d)),
            Close => p.close(),
        }
    }
    p.build().expect("bounded fixture geometry")
}
pub fn native(origin: Point<Pixels>, zoom: f32, clipped: bool, shadow: bool, window: &mut Window) {
    // Match the SVG root viewport even when its inner clipPath is disabled.
    let (x, y, width, height) = if clipped {
        (30., 30., 260., 180.)
    } else {
        (0., 0., WIDTH, HEIGHT)
    };
    let mask = Some(ContentMask {
        bounds: Bounds::new(origin + point(px(x * zoom), px(y * zoom)), size(px(width * zoom), px(height * zoom))),
    });
    window.with_content_mask(mask, |window| {
        window.paint_path(path(RIBBON, origin, zoom, false), gradient(0x7761ff, 0xf478b1));
        if shadow {
            // Native GPUI paths have no SVG filter graph. Layered strokes approximate blur.
            for radius in (1..=10).rev() {
                let mut p = PathBuilder::stroke(px(radius as f32 * zoom));
                let offset = origin + point(px(0.), px(8. * zoom));
                for segment in LEAF {
                    let pt = |x, y| offset + point(px(x * zoom), px(y * zoom));
                    match *segment {
                        Move(x, y) => p.move_to(pt(x, y)),
                        Cubic([a, b, c, d, x, y]) => p.cubic_bezier_to(pt(x, y), pt(a, b), pt(c, d)),
                        Close => p.close(),
                        _ => (),
                    }
                }
                window.paint_path(p.build().unwrap(), rgba(0x0206170b));
            }
            window.paint_path(path(LEAF, origin + point(px(0.), px(8. * zoom)), zoom, false), rgba(0x02061760));
        }
        window.paint_path(path(LEAF, origin, zoom, false), gradient(0x3ce6c0, 0x187ea5));
        window.paint_path(path(VEIN, origin, zoom, true), rgb(0xd6fff4));
    });
}
fn gradient(a: u32, b: u32) -> Background {
    linear_gradient(90., linear_color_stop(rgb(a), 0.), linear_color_stop(rgb(b), 1.))
}
