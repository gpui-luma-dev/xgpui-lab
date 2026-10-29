// Grid Shader by Christian Helgeson, WGSL Shader Depot.
// Source blob: 8a3ea8340a7573f832f4b81917f90924c60c857a
// https://github.com/cmhhelgeson/WGSL_Shader_Depot
// Copyright 2019 WebGPU Samples Contributors. BSD-3-Clause; see licenses/ShaderDepot-BSD-3-Clause.txt.
// Adaptation: host-owned inputs/resources, snake_case parameters, shared grid function.
struct Parameters {
    grid_dimensions: f32,
    cell_origin_x: f32,
    cell_origin_y: f32,
    line_width: f32,
}

fn grid_color(uv: vec2<f32>, p: Parameters) -> vec4<f32> {
    let scaled_cell = uv * p.grid_dimensions;
    let cell = abs(fract(scaled_cell) - vec2(p.cell_origin_x, p.cell_origin_y) - vec2(0.5));
    let scaled_dist = 1.0 - (1.0 + p.line_width) * max(cell.x, cell.y);
    let line = smoothstep(0.0, 0.05, scaled_dist);
    return vec4(vec3(line), 1.0);
}
