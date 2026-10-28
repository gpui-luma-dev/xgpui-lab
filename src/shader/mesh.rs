//! Luma owner for the original direct/legacy Hermite editor model.
use super::*;
pub(super) use super::mesh_tangent::MeshTangent;
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug)]
pub(super) enum Control {
    HermiteX,
    HermiteY,
    Red,
    Green,
    Blue,
    HermiteUAngle,
    HermiteULength,
    HermiteVAngle,
    HermiteVLength,
}
impl Control {
    pub fn range(self) -> (f32, f32, f32) {
        match self {
            Self::HermiteUAngle | Self::HermiteVAngle => (0., 360., 1.),
            Self::HermiteULength | Self::HermiteVLength => (0., 2., 0.01),
            _ => (0., 1., 0.01),
        }
    }
}
pub(super) struct MeshPane {
    editor: super::hermite::HermiteEditor,
    buttons: Vec<Entity<Button>>,
    sliders: Vec<(Control, Slider)>,
}
impl MeshPane {
    pub fn new(look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let buttons = [
            "Switch 2×2 / 3×3",
            "Next preset",
            "Next point",
            "Reset selected point",
            "Reset all points",
            "Round-trip legacy grid",
        ]
        .into_iter()
        .enumerate()
        .map(|(id, label)| {
            radix::Button::new(format!("hermite-{id}"))
                .look(look.as_ref())
                .outline()
                .label(label)
                .on_click(cx, move |this: &mut Self, cx| {
                    match id {
                        0 => this.editor.set_side(if this.editor.side == 2 { 3 } else { 2 }),
                        1 => this.editor.load_preset((this.editor.preset + 1) % this.editor.preset_count()),
                        2 => this.editor.selected = (this.editor.selected + 1) % this.editor.points.len(),
                        3 => this.editor.reset_selected(),
                        4 => this.editor.load_preset(this.editor.preset),
                        _ => {
                            let points = this.editor.legacy_points();
                            let tangents = this.editor.tangents.clone();
                            this.editor.import_legacy(&points, &tangents);
                        }
                    }
                    this.sync(cx);
                    cx.notify();
                })
                .spawn(cx)
        })
        .collect();
        let sliders = [
            Control::HermiteX,
            Control::HermiteY,
            Control::Red,
            Control::Green,
            Control::Blue,
            Control::HermiteUAngle,
            Control::HermiteULength,
            Control::HermiteVAngle,
            Control::HermiteVLength,
        ]
        .into_iter()
        .map(|control| {
            let (min, max, step) = control.range();
            let slider = radix::Slider::new(format!("hermite-{control:?}"))
                .look(look.as_ref())
                .range(min..max)
                .step(step)
                .on_value_changed(cx, move |this: &mut Self, _, value, cx| {
                    this.editor.set(control, value);
                    cx.notify();
                })
                .spawn(cx);
            (control, slider)
        })
        .collect();
        let mut pane = Self { editor: super::hermite::HermiteEditor::new(0), buttons, sliders };
        pane.sync(cx);
        pane
    }
    fn sync(&mut self, cx: &mut Context<Self>) {
        for (control, slider) in &self.sliders {
            slider.update(cx, |slider, cx| slider.set_value(self.editor.value(*control), cx));
        }
    }
}
impl Render for MeshPane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        use xgpui_shader::{painted, Paint};
        self.editor.refresh();
        let mut root=div().id("hermite-editor-scroll").size_full().overflow_y_scroll().p_4().flex().flex_col().gap_4()
            .child("Hermite mesh · shared-point direct / legacy comparison")
            .child(div().flex().flex_wrap().gap_2().children(self.buttons.clone()))
            .child(format!("{}×{} · preset {} · point {} (row {}, column {}) · edited {}",self.editor.side,self.editor.side,self.editor.preset,self.editor.selected+1,self.editor.selected/self.editor.side as usize+1,self.editor.selected%self.editor.side as usize+1,self.editor.edited))
            .child(div().flex().flex_wrap().gap_4().children(self.sliders.iter().map(|(control,slider)|div().w(px(200.)).flex().flex_col().gap_2().child(format!("{control:?}: {:.3}",self.editor.value(*control))).child(slider.clone()))))
            .child("Angles are clockwise from right. Lengths use normalized mesh space; angle changes at zero length are retained.");
        let mut panels = div().flex().flex_wrap().gap_4().child(
            div()
                .child("Legacy tessellation · current edit")
                .child(painted().w(px(288.)).h(px(176.)).bg_paint(Paint::from(self.editor.legacy.clone()))),
        );
        match &self.editor.brush {
            Ok(brush) => {
                panels = panels.child(
                    div()
                        .child("Direct Hermite brush · same points")
                        .child(painted().w(px(288.)).h(px(176.)).bg_brush(brush.clone())),
                );
                root = root
                    .child(panels)
                    .child(
                        painted()
                            .w(px(592.))
                            .h(px(72.))
                            .border_8()
                            .border_brush(brush.clone())
                            .child("Same patch on a border"),
                    )
                    .child(
                        painted()
                            .w(px(592.))
                            .text_size(px(36.))
                            .text_brush(brush.clone())
                            .child("Hermite patch across glyphs"),
                    );
            }
            Err(error) => {
                root = root.child(panels).child(format!(
                    "Direct brush rejected this edit: {error}. Legacy preview stays current; adjust controls or reset."
                ))
            }
        }
        root
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::{MeshPane, Control};
    use gpui_luma::controls::button::ButtonEvent;
    #[test]
    fn mounted_luma_mesh_controls_edit_the_direct_and_legacy_model() {
        let mut app = gpui::TestAppContext::single();
        app.update(|cx| gpui_luma::init(cx).unwrap());
        let (page, cx) = app.add_window_view(|_, cx| MeshPane::new(crate::theme::default_look(), cx));
        let (side, point, red) = cx.update(|_, cx| {
            let page = page.read(cx);
            (
                page.buttons[0].clone(),
                page.buttons[2].clone(),
                page.sliders.iter().find(|(control, _)| matches!(control, Control::Red)).unwrap().1.clone(),
            )
        });
        side.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        point.update(cx, |_, cx| cx.emit(ButtonEvent::Click));
        red.update(cx, |slider, cx| slider.set_thumb_position(slider.primary_thumb_id(), 0.23, cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let editor = &page.read(cx).editor;
            assert_eq!(editor.side, 3);
            assert_eq!(editor.selected, 1);
            assert!((editor.points[1].color[0] - 0.23).abs() < 0.001);
            assert!(!editor.dirty);
            assert!(editor.brush.is_ok());
        });
    }
}
