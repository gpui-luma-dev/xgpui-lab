struct Parameters {
    strip_width: f32,
    radius: f32,
    source_space: f32,
    padding1: f32,
}
fn shade(input: ShaderInput, p: Parameters) -> vec4<f32> {
    let uv = clamp(input.position / max(input.size, vec2(1.0)), vec2(0.0), vec2(1.0));
    let hue = abs(fract(vec3(uv.x) + vec3(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0);
    let saturation = min(2.0 * uv.y, 1.0);
    let value = min(2.0 * (1.0 - uv.y), 1.0);
    let encoded = value * mix(vec3(1.0), clamp(hue - 1.0, vec3(0.0), vec3(1.0)), saturation);
    if input.position.x >= input.size.x - min(p.strip_width, input.size.x) {
        return vec4(vec3(select(0.0, 1.0, uv.y < 0.5)), 1.0);
    }
    let source = u32(p.source_space);
    let linear = gamut_convert(gamut_decode(encoded, source), source, 0u);
    let rgb = gamut_encode(gamut_map(linear, 0u, 0u), 0u);
    let radius = min(p.radius, min(input.size.x, input.size.y) * 0.5);
    let q = abs(input.position - input.size * 0.5) - (input.size * 0.5 - vec2(radius));
    let distance = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;
    let aa = 0.5 / max(input.scale_factor, 1.0);
    let coverage = 1.0 - smoothstep(-aa, aa, distance);
    // Encoded-sRGB, straight alpha. The shader host handles composition encoding.
    return vec4(rgb, coverage);
}
