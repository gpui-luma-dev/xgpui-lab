//! Application-side portable paint example. No OS or backend conditionals.
use anyhow::Result;
use xgpui_shader::shader_paint::{ShaderPaint, ShaderParameters, ShaderProgram};

pub fn program() -> Result<ShaderProgram> {
    ShaderProgram::new(include_str!("../portable-wash.wgsl"))
}
pub fn paint(program: &ShaderProgram, progress: f32, alpha: f32) -> Result<ShaderPaint> {
    program.paint(
        ShaderParameters::new()
            .float("progress", progress)
            .vec3("tint", [0.95, 0.15, 0.35])
            .float("alpha", alpha),
    )
}
