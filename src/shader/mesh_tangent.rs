/// Polar controls are retained independently of the vector so editing the angle
/// at zero length still takes effect when the handle is extended again.
#[derive(Clone, Copy, Debug)]
pub struct MeshTangent {
    pub angle: f32,
    pub length: f32,
}
impl MeshTangent {
    pub fn from_vector(vector: [f32; 2]) -> Self {
        Self { angle: vector[1].atan2(vector[0]).to_degrees().rem_euclid(360.), length: vector[0].hypot(vector[1]) }
    }
    pub fn vector(self) -> [f32; 2] {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        [cos, sin].map(|v| if v.abs() < 1e-6 { 0. } else { v * self.length })
    }
}
