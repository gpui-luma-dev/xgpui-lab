//! Read-only line-breaking and hit-test probe, separate from the paragraph editor.
use super::*;
struct Snapshot {
    key: (usize, u32, u32, u32),
    layout: Arc<dyn PlatformParagraphLayout>,
    image: Arc<RenderImage>,
}
pub(super) struct LinePage {
    engine: Arc<Engine>,
    sample: usize,
    width: f32,
    size: f32,
    buttons: Vec<Entity<Button>>,
    sample_choice: crate::lab_controls::Choice,
    sliders: Vec<Slider>,
    snapshot: Option<Snapshot>,
    bounds: Bounds<Pixels>,
    hit: Option<ParagraphPosition>,
}
impl LinePage {
    pub fn new(engine: Arc<Engine>, look: Arc<Look>, cx: &mut Context<Self>) -> Self {
        let sample_choice = crate::lab_controls::Choice::new(
            "text-line-sample",
            "Sample",
            &look,
            paragraph::TEXTS.iter().map(|(label, _)| label.to_string()).collect(),
            0,
            cx,
            |this: &mut Self, index, cx| {
                this.sample = index;
                this.hit = None;
                this.snapshot = None;
                cx.notify();
            },
        );
        let buttons = vec![
            radix::Button::new("line-reset")
                .look(look.as_ref())
                .outline()
                .label("Reset layout")
                .on_click(cx, |this: &mut Self, cx| {
                    this.width = 320.;
                    this.size = 24.;
                    for (id, slider) in this.sliders.iter().enumerate() {
                        slider.update(cx, |slider, cx| slider.set_value(if id == 0 { 320. } else { 24. }, cx));
                    }
                    this.hit = None;
                    cx.notify();
                })
                .spawn(cx),
        ];
        let sliders = [(120., 720., 320.), (12., 48., 24.)]
            .into_iter()
            .enumerate()
            .map(|(id, (min, max, value))| {
                radix::Slider::new(format!("line-value-{id}"))
                    .look(look.as_ref())
                    .range(min..max)
                    .step(1.)
                    .value(value)
                    .on_value_changed(cx, move |this: &mut Self, _, value, cx| {
                        if id == 0 {
                            this.width = value;
                        } else {
                            this.size = value;
                        }
                        this.hit = None;
                        cx.notify();
                    })
                    .spawn(cx)
            })
            .collect();
        Self {
            engine,
            sample: 0,
            width: 320.,
            size: 24.,
            buttons,
            sample_choice,
            sliders,
            snapshot: None,
            bounds: Default::default(),
            hit: None,
        }
    }
    fn prepare(&mut self, scale: f32) -> Result<()> {
        let key = (self.sample, self.width.to_bits(), self.size.to_bits(), scale.to_bits());
        if self.snapshot.as_ref().is_some_and(|snapshot| snapshot.key == key) {
            return Ok(());
        }
        let source = paragraph::TEXTS[self.sample].1;
        let run = FontRun { len: source.len(), font_id: self.engine.system.resolve_font(&paragraph::font()) };
        let layout = self.engine.cache.layout_paragraph(source.into(), px(self.size), &[run], px(self.width))?;
        let image = raster::render(&*self.engine.backend, layout.runs(), px(self.size), layout.size(), scale, 0.)?;
        self.snapshot = Some(Snapshot { key, layout, image });
        Ok(())
    }
}
impl Render for LinePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Err(error) = self.prepare(window.scale_factor()) {
            return div().child(format!("Line layout unavailable: {error:#}")).into_any_element();
        }
        let snapshot = self.snapshot.as_ref().unwrap();
        let layout = snapshot.layout.clone();
        let image = snapshot.image.clone();
        let owner = cx.entity();
        let hit = self.hit;
        let mut rows = std::collections::BTreeMap::<u32, Vec<usize>>::new();
        for byte in layout.source().char_indices().map(|(byte, _)| byte).chain(std::iter::once(layout.source().len())) {
            if let Ok(caret) =
                layout.caret_bounds(ParagraphPosition { byte_index: byte, affinity: TextAffinity::Downstream })
            {
                rows.entry(f32::from(caret.top()).to_bits()).or_default().push(byte);
            }
        }
        let height = layout.size().height;
        let mut root=div().id("line-layout-probe").size_full().overflow_y_scroll().p_4().flex().flex_col().gap_4()
            .child("Line layout · wrapping, hard breaks, BiDi geometry and hit testing")
            .child(div().flex().gap_3().children(self.buttons.clone()).child(self.sample_choice.clone()))
            .child(div().flex().gap_4().children(self.sliders.iter().enumerate().map(|(id,slider)|div().w(px(240.)).flex().flex_col().gap_2().child(if id==0 {format!("Wrap width {:.0}px",self.width)}else{format!("Font size {:.0}px",self.size)}).child(slider.clone()))))
            .child(paragraph::TEXTS[self.sample].0)
            .child(format!("{} visual baseline rows · {:.1}px height · {} resolved runs",rows.len(),f32::from(height),layout.runs().len()))
            .child("Move the pointer over the read-only surface to inspect a cluster boundary. Resizing reflows the retained source; editing and IME live in Paragraphs.")
            .child(div().id("line-hit-surface").w(px(self.width)).h(height).flex_shrink_0().bg(rgb(0xffffff))
                .on_mouse_move(cx.listener(|this,event:&MouseMoveEvent,_,cx|{this.hit=this.snapshot.as_ref().and_then(|snapshot|snapshot.layout.hit_test(event.position-this.bounds.origin).ok());cx.notify();}))
                .child(canvas(move |bounds,_,cx|{owner.update(cx,|this,_|this.bounds=bounds);},move |bounds,_,window,_| {
                    let _=window.paint_image(bounds,bounds,Corners::default(),image.clone(),0,false);
                    if let Some(hit)=hit && let Ok(caret)=layout.caret_bounds(hit) {window.paint_quad(fill(Bounds::new(bounds.origin+caret.origin,caret.size),rgb(0xdc2626)));}
                }).size_full()))
            .child(format!("Hit: {:?}",self.hit));
        for (y, bytes) in rows {
            root = root.child(format!("y={:.1}px · source byte boundaries {:?}", f32::from_bits(y), bytes));
        }
        root.into_any_element()
    }
}
