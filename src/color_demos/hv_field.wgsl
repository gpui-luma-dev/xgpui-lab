struct Parameters {
    color: vec4<f32>,
    source_space: f32,
    kind: f32,
    channel: f32,
    vertical: f32,
    radius: f32,
    strip_width: f32,
    reversed: f32,
    sdr_fallback: f32,
}
fn shade(input: ShaderInput, p: Parameters) -> vec4<f32> {
    let uv = clamp(input.position / max(input.size, vec2(1.0)), vec2(0.0), vec2(1.0));
    var encoded = p.color.rgb;
    if p.kind == 0.0 {
        let hue = abs(fract(vec3(uv.x) + vec3(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0);
        let saturation = p.color.r;
        let value = 1.0 - uv.y;
        encoded = value * mix(vec3(1.0), clamp(hue - 1.0, vec3(0.0), vec3(1.0)), saturation);
        if input.position.x >= input.size.x - min(p.strip_width, input.size.x) {
            encoded = vec3(select(0.0, 1.0, uv.y < 0.5));
        }
    } else if p.kind == 2.0 {
        var position = select(uv.x, uv.y, p.vertical > 0.5);
        position = select(position, 1.0 - position, p.reversed > 0.5);
        if p.channel == 3.0 {
            let value = max(encoded.r, max(encoded.g, encoded.b));
            encoded = mix(vec3(value), encoded, position);
        } else {
            encoded[u32(p.channel)] = position;
        }
    }
    // The selected program declares the output encoding. No sRGB gamut mapping.
    var rgb = select(encoded, gamut_decode(encoded, u32(p.source_space)), p.source_space > 0.5);
    var contour = vec2(0.0);
    var is_out_of_gamut = false;
    if p.kind == 0.0 && p.source_space > 0.5 && p.color.g > 0.5 && p.color.r > 0.00001 {
        let srgb = gamut_convert(rgb, u32(p.source_space), 0u);
        let margin = min(min(srgb.r, min(srgb.g, srgb.b)), 1.0 - max(srgb.r, max(srgb.g, srgb.b)));
        is_out_of_gamut = margin < -0.000001
            && input.position.x < input.size.x - p.strip_width;
        let gradient = length(vec2(dpdx(margin), dpdy(margin)));
        let pixels = abs(margin) / max(gradient, 0.0000001);
        let inset = 2.0 / max(input.scale_factor, 1.0);
        let interior = input.position.y > inset && input.position.y < input.size.y - inset
            && input.position.x > inset && input.position.x < input.size.x - p.strip_width - inset;
        if interior && gradient > 0.0000001 {
            contour = vec2(1.0 - smoothstep(0.75, 1.5, pixels), 1.0 - smoothstep(0.0, 1.0, pixels));
        }
    }
    if is_out_of_gamut {
        let stripe = fract((input.position.x + input.position.y) * input.scale_factor / 12.0);
        if stripe < 0.25 {
            rgb = mix(rgb, vec3(0.0), 0.35);
        }
    }
    if p.sdr_fallback > 0.5 {
        rgb = gamut_encode(clamp(gamut_convert(rgb, u32(p.source_space), 0u), vec3(0.0), vec3(1.0)), 0u);
    }
    // Adobe RGB has no output tag; Rec2020 contains its gamut without clipping to P3.
    if p.source_space == 3.0 && p.sdr_fallback < 0.5 {
        rgb = gamut_convert(rgb, 3u, 2u);
    }
    rgb = mix(rgb, vec3(0.0), contour.x);
    rgb = mix(rgb, vec3(1.0), contour.y);
    let radius = min(p.radius, min(input.size.x, input.size.y) * 0.5);
    let q = abs(input.position - input.size * 0.5) - (input.size * 0.5 - vec2(radius));
    let distance = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;
    let aa = 0.5 / max(input.scale_factor, 1.0);
    return vec4(rgb, 1.0 - smoothstep(-aa, aa, distance));
}
