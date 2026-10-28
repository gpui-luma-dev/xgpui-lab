//! Live controls for the shared-point direct/legacy Hermite comparison.
use super::mesh::{Control, MeshTangent};
use std::sync::Arc;
use xgpui_shader::{Brush, BrushMeshPoint, ControlPoint, MeshGradient, MeshGrid};

pub(super) struct HermiteEditor {
    pub points: Vec<BrushMeshPoint>,
    pub side: u32,
    pub tangents: Vec<[MeshTangent; 2]>,
    pub selected: usize,
    pub preset: u8,
    pub edited: bool,
    pub dirty: bool,
    pub legacy: Arc<MeshGradient>,
    pub brush: Result<Brush, String>,
}
impl HermiteEditor {
    pub fn new(preset: u8) -> Self {
        Self::with_side(preset, 2)
    }
    fn with_side(preset: u8, side: u32) -> Self {
        let points = preset_points(preset, side);
        let tangents = points
            .iter()
            .map(|p| [MeshTangent::from_vector(p.u_tangent), MeshTangent::from_vector(p.v_tangent)])
            .collect();
        let legacy = Arc::new(MeshGradient::new(grid(&points, side), 64).expect("bounded editor mesh"));
        let brush = Brush::hermite_mesh(side, side, points.clone()).map_err(|e| e.to_string());
        Self { points, tangents, side, selected: 0, preset, edited: false, dirty: false, legacy, brush }
    }
    pub fn preset_count(&self) -> u8 {
        if self.side == 3 { 5 } else { 4 }
    }
    pub fn set_side(&mut self, side: u32) {
        let selected = self.selected.min((side * side - 1) as usize);
        *self = Self::with_side(self.preset.min(3), side);
        self.selected = selected;
    }
    pub fn import_legacy(&mut self, points: &[ControlPoint], tangents: &[[MeshTangent; 2]]) {
        let incoming: Vec<_> = points
            .iter()
            .map(|p| {
                BrushMeshPoint::new(p.position, p.u_tangent, p.v_tangent, [p.color[0], p.color[1], p.color[2], 1.])
            })
            .collect();
        if self.points != incoming {
            self.points = incoming;
            self.edited = true;
            self.dirty = true;
        }
        self.tangents = tangents.to_vec();
    }
    pub fn legacy_points(&self) -> Vec<ControlPoint> {
        self.points
            .iter()
            .map(|p| ControlPoint {
                position: p.position,
                u_tangent: p.u_tangent,
                v_tangent: p.v_tangent,
                color: [p.color[0], p.color[1], p.color[2]],
            })
            .collect()
    }
    pub fn load_preset(&mut self, preset: u8) {
        let selected = self.selected;
        *self = Self::with_side(preset, self.side);
        self.selected = selected;
    }
    pub fn reset_selected(&mut self) {
        let point = preset_points(self.preset, self.side)[self.selected];
        self.points[self.selected] = point;
        self.tangents[self.selected] =
            [MeshTangent::from_vector(point.u_tangent), MeshTangent::from_vector(point.v_tangent)];
        self.edited = true;
        self.dirty = true;
    }
    pub fn value(&self, control: Control) -> f32 {
        let p = self.points[self.selected];
        let t = self.tangents[self.selected];
        match control {
            Control::HermiteX => p.position[0],
            Control::HermiteY => p.position[1],
            Control::Red => p.color[0],
            Control::Green => p.color[1],
            Control::Blue => p.color[2],
            Control::HermiteUAngle => t[0].angle,
            Control::HermiteULength => t[0].length,
            Control::HermiteVAngle => t[1].angle,
            Control::HermiteVLength => t[1].length,
        }
    }
    pub fn set(&mut self, control: Control, value: f32) {
        let (min, max, _) = control.range();
        let value = value.clamp(min, max);
        if value == self.value(control) {
            return;
        }
        let p = &mut self.points[self.selected];
        match control {
            Control::HermiteX => p.position[0] = value,
            Control::HermiteY => p.position[1] = value,
            Control::Red => p.color[0] = value,
            Control::Green => p.color[1] = value,
            Control::Blue => p.color[2] = value,
            _ => {
                let axis = usize::from(matches!(control, Control::HermiteVAngle | Control::HermiteVLength));
                let t = &mut self.tangents[self.selected][axis];
                if matches!(control, Control::HermiteUAngle | Control::HermiteVAngle) {
                    t.angle = value;
                } else {
                    t.length = value;
                }
                if axis == 0 {
                    p.u_tangent = t.vector();
                } else {
                    p.v_tangent = t.vector();
                }
            }
        }
        self.edited = true;
        self.dirty = true;
    }
    /// Coalesce input events into one material rebuild per rendered frame.
    pub fn refresh(&mut self) {
        if !self.dirty {
            return;
        }
        self.legacy = Arc::new(self.legacy.with_grid(grid(&self.points, self.side)).expect("bounded editor mesh"));
        self.brush = Brush::hermite_mesh(self.side, self.side, self.points.clone()).map_err(|e| e.to_string());
        self.dirty = false;
    }
}
fn grid(points: &[BrushMeshPoint], side: u32) -> MeshGrid {
    MeshGrid::new(
        side,
        side,
        points
            .iter()
            .map(|p| ControlPoint {
                position: p.position,
                u_tangent: p.u_tangent,
                v_tangent: p.v_tangent,
                color: [p.color[0], p.color[1], p.color[2]],
            })
            .collect(),
    )
    .expect("finite editor mesh")
}
fn preset_points(kind: u8, side: u32) -> Vec<BrushMeshPoint> {
    let colors = [[1., 0.05, 0.1, 1.], [0.05, 0.2, 1., 1.], [0., 0.9, 0.5, 1.], [1., 0.65, 0., 1.]];
    let mut points: Vec<_> = if side == 3 {
        super::initial_mesh_grid()
            .unwrap()
            .points()
            .iter()
            .map(|p| {
                BrushMeshPoint::new(p.position, p.u_tangent, p.v_tangent, [p.color[0], p.color[1], p.color[2], 1.])
            })
            .collect()
    } else {
        (0..4)
            .map(|i| BrushMeshPoint::new([(i % 2) as f32, (i / 2) as f32], [1., 0.], [0., 1.], colors[i]))
            .collect()
    };
    let step = 1. / (side - 1) as f32;
    match kind {
        1 => {
            points[0].u_tangent[1] = 0.2 * step;
            points[(side - 1) as usize].u_tangent[1] = -0.2 * step;
        }
        2 => {
            points[0].v_tangent[0] = 0.2 * step;
            points[(side * (side - 1)) as usize].v_tangent[0] = -0.2 * step;
        }
        3 => {
            for p in &mut points {
                p.position = p.position.map(|v| 0.1 + v * 0.8);
                p.u_tangent = [step * 0.8, 0.];
                p.v_tangent = [0., step * 0.8];
            }
        }
        4 if side == 3 => points[4].position = [0.53, 0.47],
        _ => {}
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_edits_keep_legacy_current_and_recover_without_a_stale_direct_brush() {
        let mut editor = HermiteEditor::new(0);
        editor.set(Control::HermiteUAngle, 180.);
        editor.refresh();
        assert!(editor.brush.is_err());
        assert_eq!(editor.legacy.grid().points()[0].u_tangent, [-1., 0.]);
        editor.set(Control::HermiteUAngle, 0.);
        editor.refresh();
        assert!(editor.brush.is_ok());
        assert_eq!(editor.legacy.grid().points()[0].u_tangent, [1., 0.]);
    }
    #[test]
    fn zero_length_preserves_direction_and_selected_reset_preserves_other_edits() {
        let mut editor = HermiteEditor::new(0);
        editor.set(Control::HermiteULength, 0.);
        editor.set(Control::HermiteUAngle, 10.);
        editor.selected = 1;
        editor.set(Control::HermiteVLength, 0.9);
        editor.reset_selected();
        editor.selected = 0;
        assert_eq!(editor.value(Control::HermiteUAngle), 10.);
        assert_eq!(editor.value(Control::HermiteULength), 0.);
        editor.set(Control::HermiteULength, 1.);
        editor.refresh();
        assert!(editor.brush.is_ok());
        assert!(editor.points[0].u_tangent[1] > 0.17);
        assert_eq!(editor.points[1], preset_points(0, 2)[1]);
        editor.load_preset(3);
        assert!(!editor.edited);
        assert_eq!(editor.points, preset_points(3, 2));
    }
    #[test]
    fn three_by_three_presets_and_legacy_roundtrip_keep_shared_points() {
        let mut editor = HermiteEditor::new(0);
        editor.set_side(3);
        for preset in 0..editor.preset_count() {
            editor.load_preset(preset);
            assert!(editor.brush.is_ok(), "preset {preset}: {:?}", editor.brush.as_ref().err());
            assert_eq!(editor.points.len(), 9);
        }
        editor.selected = 4;
        editor.set(Control::HermiteX, 0.525);
        editor.refresh();
        assert!(editor.brush.is_ok());
        let points = editor.legacy_points();
        assert_eq!(points[4].position, editor.points[4].position);
        let mut imported = HermiteEditor::new(0);
        imported.set_side(3);
        imported.import_legacy(&points, &editor.tangents);
        imported.refresh();
        assert_eq!(imported.points, editor.points);
        assert_eq!(imported.legacy.grid().points(), points.as_slice());
        assert!(imported.brush.is_ok());
        editor.set_side(2);
        assert!(editor.selected < 4);
        assert_eq!(editor.points.len(), 4);
    }
}

#[cfg(test)]
mod restored_controls_tests {
    use super::*;
    #[test]
    fn edits_survive_legacy_round_trip_and_refresh_both_previews() {
        let mut editor = HermiteEditor::new(0);
        editor.set_side(3);
        editor.selected = 4;
        editor.set(Control::Red, 0.27);
        editor.set(Control::HermiteUAngle, 125.);
        editor.set(Control::HermiteULength, 0.4);
        let points = editor.legacy_points();
        let tangents = editor.tangents.clone();
        editor.import_legacy(&points, &tangents);
        editor.refresh();
        assert_eq!(editor.value(Control::Red), 0.27);
        assert_eq!(editor.value(Control::HermiteUAngle), 125.);
        assert_eq!(editor.legacy_points()[4].color[0], points[4].color[0]);
        assert!(!editor.dirty);
        editor.reset_selected();
        assert_ne!(editor.value(Control::Red), 0.27);
    }
}
