/*
 * "Seascape" by Alexander Alekseev aka TDM - 2014
 * License Creative Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License.
 * Contact: tdmaav@gmail.com
 * https://www.shadertoy.com/view/Ms2SD1
 * https://creativecommons.org/licenses/by-nc-sa/3.0/
 * Author's source blob: 1db1492a27e6f97059edeb75eeda03f3521be451
 * Modified: GLSL translated to WGSL; explicit parameters, top-left UV conversion,
 * out argument replaced by trace state, defined reversed smoothstep, safe epsilons.
 * The app expands bounded loops for the loop-free shader-paint contract; instanced shaders retain loops.
 */
struct Parameters {
    time: f32,
    sea_height: f32,
    choppiness: f32,
    sea_speed: f32,
    frequency: f32,
    camera: f32,
    exposure: f32,
    detail: f32,
}
const PI: f32 = 3.141592;
const SEA_BASE = vec3<f32>(0.0, 0.09, 0.18);
const WATER = vec3<f32>(0.8, 0.9, 0.6) * 0.6;

fn from_euler(ang: vec3<f32>) -> mat3x3<f32> {
    let a1 = vec2(sin(ang.x), cos(ang.x));
    let a2 = vec2(sin(ang.y), cos(ang.y));
    let a3 = vec2(sin(ang.z), cos(ang.z));
    return mat3x3<f32>(
        vec3(a1.y*a3.y+a1.x*a2.x*a3.x, a1.y*a2.x*a3.x+a3.y*a1.x, -a2.y*a3.x),
        vec3(-a2.y*a1.x, a1.y*a2.y, a2.x),
        vec3(a3.y*a1.x*a2.x+a1.y*a3.x, a1.x*a3.x-a1.y*a3.y*a2.x, a2.y*a3.y));
}
fn sea_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}
fn sea_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f*f*(vec2(3.0)-2.0*f);
    return -1.0 + 2.0 * mix(
        mix(sea_hash(i), sea_hash(i+vec2(1.0,0.0)), u.x),
        mix(sea_hash(i+vec2(0.0,1.0)), sea_hash(i+vec2(1.0)), u.x), u.y);
}
fn sea_octave(coords: vec2<f32>, choppy: f32) -> f32 {
    let uv = coords + vec2(sea_noise(coords));
    let wave = vec2(1.0) - abs(sin(uv));
    let swave = abs(cos(uv));
    let wv = mix(wave, swave, wave);
    return pow(max(1.0 - pow(max(wv.x*wv.y, 0.0), 0.65), 0.0), choppy);
}
fn sea_map(position: vec3<f32>, detailed: bool, p: Parameters) -> f32 {
    var freq = p.frequency;
    var amp = p.sea_height;
    var choppy = p.choppiness;
    var uv = position.xz * vec2(0.75, 1.0);
    var height = 0.0;
    let sea_time = 1.0 + p.time * p.sea_speed;
    let iterations = select(3u, 5u, detailed && p.detail > 0.5);
    // BEGIN_OCTAVES
    for (var iteration = 0u; iteration < 5u; iteration += 1u) {
        if iteration < iterations {
            let d = sea_octave((uv+vec2(sea_time))*freq, choppy)
                  + sea_octave((uv-vec2(sea_time))*freq, choppy);
            height += d * amp;
            // GLSL uv *= octave_m is row-vector multiplication.
            uv = uv * mat2x2<f32>(vec2(1.6,1.2), vec2(-1.2,1.6));
            freq *= 1.9;
            amp *= 0.22;
            choppy = mix(choppy, 1.0, 0.2);
        }
    }
    // END_OCTAVES
    return position.y - height;
}
fn sky_color(direction: vec3<f32>) -> vec3<f32> {
    let y = (max(direction.y, 0.0)*0.8+0.2)*0.8;
    return vec3(pow(1.0-y, 2.0), 1.0-y, 0.6+(1.0-y)*0.4) * 1.1;
}
struct TraceState {
    near_t: f32,
    far_t: f32,
    near_h: f32,
    far_h: f32,
    position: vec3<f32>,
    done: bool,
}
fn trace_step(previous: TraceState, origin: vec3<f32>, direction: vec3<f32>, p: Parameters) -> TraceState {
    var state = previous;
    if state.done { return state; }
    let denominator = state.near_h - state.far_h;
    if abs(denominator) < 0.000001 { state.done = true; return state; }
    let mid = mix(state.near_t, state.far_t, clamp(state.near_h / denominator, 0.0, 1.0));
    state.position = origin + direction * mid;
    let height = sea_map(state.position, false, p);
    if height < 0.0 { state.far_t = mid; state.far_h = height; }
    else { state.near_t = mid; state.near_h = height; }
    state.done = abs(height) < 0.001;
    return state;
}
fn trace_height(origin: vec3<f32>, direction: vec3<f32>, p: Parameters) -> vec3<f32> {
    let far_position = origin + direction * 1000.0;
    let far_h = sea_map(far_position, false, p);
    var state = TraceState(0.0, 1000.0, sea_map(origin, false, p), far_h, far_position, far_h > 0.0);
    // BEGIN_TRACE
    for (var iteration = 0u; iteration < 32u; iteration += 1u) {
        state = trace_step(state, origin, direction, p);
    }
    // END_TRACE
    return state.position;
}
fn sea_normal(position: vec3<f32>, epsilon: f32, p: Parameters) -> vec3<f32> {
    let eps = max(epsilon, 0.0001);
    let h = sea_map(position, true, p);
    return normalize(vec3(
        sea_map(position+vec3(eps,0.0,0.0), true, p)-h, eps,
        sea_map(position+vec3(0.0,0.0,eps), true, p)-h));
}
fn sea_color(position: vec3<f32>, normal: vec3<f32>, light: vec3<f32>, eye: vec3<f32>, distance: vec3<f32>, p: Parameters) -> vec3<f32> {
    let f = clamp(1.0-dot(normal, -eye), 0.0, 1.0);
    let fresnel = min(f*f*f, 0.5);
    let reflected = sky_color(reflect(eye, normal));
    let diffuse = pow(max(dot(normal, light)*0.4+0.6, 0.0), 80.0);
    let refracted = SEA_BASE + diffuse * WATER * 0.12;
    var color = mix(refracted, reflected, fresnel);
    let atten = max(1.0-dot(distance,distance)*0.001, 0.0);
    color += WATER * (position.y-p.sea_height) * 0.18 * atten;
    let shininess = 600.0 * inverseSqrt(max(dot(distance,distance), 0.0001));
    color += vec3(pow(max(dot(reflect(eye,normal),light), 0.0), shininess) * (shininess+8.0)/(PI*8.0));
    return color;
}
fn seascape_color(input_uv: vec2<f32>, extent: vec2<f32>, p: Parameters) -> vec4<f32> {
    // GPUI UVs start at top-left; Shadertoy fragCoord starts at bottom-left.
    var uv = vec2(input_uv.x, 1.0-input_uv.y)*2.0-vec2(1.0);
    uv.x *= extent.x / max(extent.y, 1.0);
    let time = p.time * 0.3 + p.camera;
    let angles = vec3(sin(time*3.0)*0.1, sin(time)*0.2+0.3, time);
    let origin = vec3(0.0, 3.5, time*5.0);
    var direction = normalize(vec3(uv, -2.0));
    direction.z += length(uv)*0.14;
    direction = normalize(direction) * from_euler(angles);
    let position = trace_height(origin, direction, p);
    let distance = position-origin;
    let normal = sea_normal(position, dot(distance,distance)*0.1/max(extent.x,1.0), p);
    let light = normalize(vec3(0.0,1.0,0.8));
    // Equivalent to GLSL's reversed-edge smoothstep, with defined WGSL edges.
    let horizon = pow(1.0-smoothstep(-0.02,0.0,direction.y), 0.2);
    let color = mix(sky_color(direction), sea_color(position,normal,light,direction,distance,p), horizon);
    return vec4(pow(max(color*p.exposure,vec3(0.0)),vec3(0.65)), 1.0);
}
