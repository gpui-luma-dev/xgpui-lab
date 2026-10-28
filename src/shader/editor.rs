//! Gradient stop/geometry model adapted from xgpui/apps/paint-lab/src/editor.rs.
use super::{Effect, gradient_stops};
use xgpui_shader::{GradientKind, GradientStop};

#[derive(Clone)]
pub(super) struct Settings {
    pub stops: Vec<GradientStop>,
    pub selected: usize,
    pub angle: f32,
    pub center: [f32; 2],
    pub radius: [f32; 2],
}
impl Settings {
    pub fn new(effect: Effect) -> Self {
        Self {
            stops: gradient_stops(),
            selected: 0,
            angle: if effect == Effect::Linear { 90. } else { 0. },
            center: [0.5; 2],
            radius: [0.5; 2],
        }
    }
    pub fn kind(&self, effect: Effect, time: f32) -> Option<GradientKind> {
        let angle = (self.angle + time * 20.).rem_euclid(360.);
        match effect {
            Effect::Mesh => None,
            Effect::Linear => Some(GradientKind::Linear { angle }),
            Effect::Radial => Some(GradientKind::Radial {
                center: self.center,
                radius: self.radius.map(|r| (r * (1. + 0.15 * time.sin())).clamp(0.01, 2.)),
            }),
            Effect::Conic => Some(GradientKind::Conic { center: self.center, angle }),
            Effect::Diamond => Some(GradientKind::Diamond { center: self.center, radius: self.radius, angle }),
        }
    }
    /// Commit the displayed animated geometry before editing or pausing.
    pub fn freeze(&mut self, effect: Effect, time: f32) {
        match self.kind(effect, time) {
            Some(
                GradientKind::Linear { angle }
                | GradientKind::Conic { angle, .. }
                | GradientKind::Diamond { angle, .. },
            ) => self.angle = angle,
            Some(GradientKind::Radial { radius, .. }) => self.radius = radius,
            None => {}
        }
    }
    pub fn move_stop(&mut self, offset: f32) {
        let offset = offset.clamp(0., 1.);
        if self.stops[self.selected].offset == offset {
            return;
        }
        let mut stop = self.stops.remove(self.selected);
        stop.offset = offset;
        self.selected = self.stops.partition_point(|s| s.offset <= offset);
        self.stops.insert(self.selected, stop);
    }
    /// Insert a color-interpolated stop in the largest gap.
    pub fn add(&mut self) {
        if self.stops.len() >= 32 {
            return;
        }
        let i = self
            .stops
            .windows(2)
            .enumerate()
            .max_by(|(_, a), (_, b)| (a[1].offset - a[0].offset).total_cmp(&(b[1].offset - b[0].offset)))
            .unwrap()
            .0;
        let a = self.stops[i];
        let b = self.stops[i + 1];
        let alpha = (a.color[3] + b.color[3]) * 0.5;
        let mut color = [0.; 4];
        color[3] = alpha;
        for (c, value) in color[..3].iter_mut().enumerate() {
            *value = if alpha > 0. {
                (a.color[c] * a.color[3] + b.color[c] * b.color[3]) * 0.5 / alpha
            } else {
                0.
            };
        }
        self.selected = i + 1;
        self.stops.insert(self.selected, GradientStop::new((a.offset + b.offset) * 0.5, color));
    }
    pub fn duplicate(&mut self) {
        if self.stops.len() >= 32 {
            return;
        }
        let stop = self.stops[self.selected];
        self.selected += 1;
        self.stops.insert(self.selected, stop);
    }
    pub fn remove(&mut self) {
        if self.stops.len() <= 2 {
            return;
        }
        self.stops.remove(self.selected);
        self.selected = self.selected.min(self.stops.len() - 1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use xgpui_shader::Gradient;
    #[test]
    fn stop_edits_preserve_selection_order_and_limits() {
        let mut s = Settings::new(Effect::Linear);
        let color = s.stops[0].color;
        s.move_stop(0.8);
        assert_eq!(s.stops[s.selected].color, color);
        assert_eq!(s.stops[s.selected].offset, 0.8);
        assert!(s.stops.windows(2).all(|s| s[0].offset <= s[1].offset));
        s.duplicate();
        assert_eq!(s.stops[s.selected], s.stops[s.selected - 1]);
        for _ in 0..40 {
            s.add();
        }
        assert_eq!(s.stops.len(), 32);
        assert!(Gradient::new(s.kind(Effect::Linear, 0.).unwrap(), s.stops.clone()).is_ok());
        for _ in 0..40 {
            s.remove();
        }
        assert_eq!(s.stops.len(), 2);
        assert!(s.selected < 2);
    }
    #[test]
    fn freezing_animation_preserves_displayed_geometry() {
        for effect in [Effect::Linear, Effect::Radial, Effect::Conic, Effect::Diamond] {
            let mut s = Settings::new(effect);
            let before = s.kind(effect, 2.3).unwrap();
            s.freeze(effect, 2.3);
            assert_eq!(s.kind(effect, 0.).unwrap(), before);
        }
    }
    #[test]
    fn edited_geometry_and_transparent_colors_are_valid() {
        let mut s = Settings::new(Effect::Radial);
        s.center = [0.2, 0.8];
        s.radius = [0.1, 1.5];
        s.stops[0].color = [1., 0., 1., 0.25];
        s.add();
        for effect in [Effect::Linear, Effect::Radial, Effect::Conic, Effect::Diamond] {
            assert!(Gradient::new(s.kind(effect, 0.).unwrap(), s.stops.clone()).is_ok());
        }
        let other = Settings::new(Effect::Radial);
        assert_ne!(s.stops, other.stops);
    }
}
