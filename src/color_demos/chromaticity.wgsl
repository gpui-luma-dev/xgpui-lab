// CIE locus geometry is generated from the attributed CSV before compilation.
// Color conversion and fixed-L/h chroma reduction come from xgpui-color.
struct Parameters {
    simulated: f32,
    luminance: f32,
    reduce_chroma: f32,
    show_fill: f32,
    source_x: f32,
    source_y: f32,
    source_visible: f32,
    overlays: f32,
}
fn segment_distance(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let edge = b - a;
    return length(p - a - edge * clamp(dot(p - a, edge) / max(dot(edge, edge), 1e-20), 0.0, 1.0));
}
fn locus_distance(p: vec2<f32>) -> f32 {
    var nearest = 1e10;
    var inside = false;
    var previous = LOCUS[LOCUS_COUNT - 1u];
    for (var i = 0u; i < LOCUS_COUNT; i++) {
        let current = LOCUS[i];
        nearest = min(nearest, segment_distance(p, previous, current));
        if (current.y > p.y) != (previous.y > p.y) {
            let crossing = (previous.x - current.x) * (p.y - current.y) / (previous.y - current.y) + current.x;
            if p.x < crossing { inside = !inside; }
        }
        previous = current;
    }
    return select(nearest, -nearest, inside);
}
fn triangle_distance(p: vec2<f32>, index: u32) -> f32 {
    let a = PRIMARIES[index * 3u];
    let b = PRIMARIES[index * 3u + 1u];
    let c = PRIMARIES[index * 3u + 2u];
    return min(segment_distance(p, a, b), min(segment_distance(p, b, c), segment_distance(p, c, a)));
}
fn stroke_coverage(distance: f32, width: f32, aa: f32) -> f32 {
    return 1.0 - smoothstep(0.5 * width - 0.5 * aa, 0.5 * width + 0.5 * aa, distance);
}
fn cross_distance(pixel: vec2<f32>, center: vec2<f32>) -> f32 {
    return min(segment_distance(pixel, center - vec2(5.0, 0.0), center + vec2(5.0, 0.0)),
        segment_distance(pixel, center - vec2(0.0, 5.0), center + vec2(0.0, 5.0)));
}
// The display order puts Adobe RGB before Rec.2020; xgpui-color's IDs do not.
fn color_space(index: f32) -> u32 {
    if index == 2.0 { return 3u; }
    if index == 3.0 { return 2u; }
    return u32(index);
}
fn chart_rgb(xy: vec2<f32>, p: Parameters) -> vec3<f32> {
    let y = max(xy.y, 0.00001);
    let xyz = vec3(xy.x * p.luminance / y, p.luminance, (1.0 - xy.x - xy.y) * p.luminance / y);
    let space = color_space(p.simulated);
    let linear = gamut_from_xyz(xyz, space);
    let mapped = gamut_map(linear, space, u32(p.reduce_chroma));
    let srgb = clamp(gamut_convert(mapped, space, 0u), vec3(0.0), vec3(1.0));
    return gamut_encode(srgb, 0u);
}
// Straight-alpha source-over keeps antialiased edges correct on any enclosure.
fn over(base: vec4<f32>, rgb: vec3<f32>, coverage: f32) -> vec4<f32> {
    let alpha = coverage + base.a * (1.0 - coverage);
    let premultiplied = rgb * coverage + base.rgb * base.a * (1.0 - coverage);
    return vec4(premultiplied / max(alpha, 0.000001), alpha);
}
fn shade(input: ShaderInput, p: Parameters) -> vec4<f32> {
    let scale = max(1.0, min((input.size.x - 48.0) / 0.8, (input.size.y - 48.0) / 0.9));
    let left = (input.size.x - scale * 0.8) * 0.5;
    let bottom = input.size.y - 24.0;
    let xy = vec2((input.position.x - left) / scale, (bottom - input.position.y) / scale);
    // Distances are in logical pixels. Coverage spans one physical pixel at
    // every backing scale; no low-resolution texture or cell tessellation.
    let aa = 1.0 / max(input.scale_factor, 1.0);
    let distance = locus_distance(xy) * scale;
    let coverage = 1.0 - smoothstep(-0.5 * aa, 0.5 * aa, distance);
    var color = vec4(0.0);
    if p.show_fill > 0.5 && coverage > 0.0 {
        let fill = chart_rgb(xy, p);
        color = vec4(fill, coverage);
        if p.overlays > 0.5 {
            let y = max(xy.y, 0.00001);
            let xyz = vec3(xy.x * p.luminance / y, p.luminance, (1.0 - xy.x - xy.y) * p.luminance / y);
            let linear = gamut_from_xyz(xyz, color_space(p.simulated));
            if !gamut_in_bounds(linear) {
                let hatch_distance = abs(fract((input.position.x + input.position.y) / 8.0) - 0.5) * 8.0;
                let hatch = stroke_coverage(hatch_distance, 0.85, aa);
                color = vec4(mix(color.rgb, vec3(16.0, 24.0, 35.0) / 255.0, hatch * 0.55), color.a);
            }
        }
    }
    if p.overlays > 0.5 {
        let plot_distance = max(abs(xy.x - 0.4) - 0.4, abs(xy.y - 0.45) - 0.45) * scale;
        let plot_coverage = 1.0 - smoothstep(-0.5 * aa, 0.5 * aa, plot_distance);
        let grid = abs(xy / vec2(0.1, 0.1125) - round(xy / vec2(0.1, 0.1125))) * vec2(0.1, 0.1125) * scale;
        color = over(color, vec3(51.0, 65.0, 85.0) / 255.0,
            stroke_coverage(min(grid.x, grid.y), 1.0, aa) * plot_coverage);
        color = over(color, vec3(168.0, 180.0, 196.0) / 255.0, stroke_coverage(abs(distance), 1.5, aa));
        for (var i = 0u; i < 4u; i++) {
            color = over(color, OUTLINE_COLORS[i], stroke_coverage(triangle_distance(xy, i) * scale, 2.0, aa));
        }
        let white = vec2(left + 0.3127 * scale, bottom - 0.3290 * scale);
        color = over(color, vec3(1.0), stroke_coverage(cross_distance(input.position, white), 2.0, aa));
        if p.source_visible > 0.5 {
            let selected = vec2(left + p.source_x * scale, bottom - p.source_y * scale);
            color = over(color, vec3(1.0, 223.0 / 255.0, 98.0 / 255.0),
                stroke_coverage(cross_distance(input.position, selected), 2.0, aa));
        }
    }
    // Preserve rounded enclosure clipping while leaving empty regions transparent.
    let corner = abs(input.position - input.size * 0.5) - (input.size * 0.5 - vec2(6.0));
    let corner_distance = length(max(corner, vec2(0.0))) + min(max(corner.x, corner.y), 0.0) - 6.0;
    return vec4(color.rgb, color.a * (1.0 - smoothstep(-0.5 * aa, 0.5 * aa, corner_distance)));
}
