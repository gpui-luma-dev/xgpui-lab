// The extension supplies PaintInput and the `parameters` value. No bindings.
struct Parameters {
    progress: f32,
    tint: vec3<f32>,
    alpha: f32,
}

fn paint(input: PaintInput) -> vec4<f32> {
    let blend = clamp(input.uv.x + parameters.progress, 0.0, 1.0);
    let accent = vec3(0.1, clamp(input.position.y / 160.0, 0.0, 1.0), input.size.x / 400.0);
    return vec4(mix(parameters.tint, accent, blend), parameters.alpha);
}
