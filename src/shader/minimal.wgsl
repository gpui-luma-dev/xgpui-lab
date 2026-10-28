// A complete custom fragment shader. Vertex geometry, clipping and opacity are host-owned.
struct Parameters { color: vec4<f32>, effect: vec4<f32>, }
fn shade(input: ShaderInput, p: Parameters) -> vec4<f32> {
    let radius = p.effect.x;
    let q = abs(input.position - input.size * 0.5) - input.size * 0.5 + radius;
    let distance = min(max(q.x, q.y), 0.) + length(max(q, vec2(0.))) - radius;
    let coverage = 1. - smoothstep(-1. / input.scale_factor, 1. / input.scale_factor, distance);
    let wave = 0.65 + 0.35 * sin(input.uv.x * 6.283 + p.effect.y);
    return vec4(p.color.rgb * wave, p.color.a * coverage);
}
