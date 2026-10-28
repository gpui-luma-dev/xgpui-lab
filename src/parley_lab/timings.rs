use super::samples::Sample;
use anyhow::{Result, ensure};
use gpui::{PlatformTextSystem, RenderGlyphParams, TextSystem, WindowTextSystem, point, px};
use std::{collections::HashSet, hint::black_box, sync::Arc, time::Instant};

pub struct Timing {
    pub layout_first: f64,
    pub layout_warm: f64,
    pub raster_first: f64,
    pub raster_warm: f64,
    pub width: f32,
    pub faces: usize,
    pub glyphs: usize,
}

fn micros(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1e6
}

pub fn measure(
    text: Arc<TextSystem>,
    backend: &dyn PlatformTextSystem,
    samples: &[Sample],
    iterations: usize,
    scale: f32,
) -> Result<Vec<Timing>> {
    let window = WindowTextSystem::new(text);
    let mut results = Vec::new();
    for sample in samples {
        let runs = sample.runs();
        let start = Instant::now();
        let layout = window.layout_line(&sample.text, px(28.0), &runs, None);
        let layout_first = micros(start);
        let mut times = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            let warm = window.layout_line(&sample.text, px(28.0), &runs, None);
            times.push(micros(start));
            ensure!(Arc::ptr_eq(&layout, &warm), "expected GPUI layout cache hit");
            black_box(warm);
        }
        times.sort_by(f64::total_cmp);
        let layout_warm = times[times.len() / 2];
        let mut seen = HashSet::new();
        let mut glyphs = Vec::new();
        for run in &layout.runs {
            for glyph in &run.glyphs {
                let p = RenderGlyphParams {
                    font_id: run.font_id,
                    glyph_id: glyph.id,
                    font_size: px(28.0),
                    scale_factor: scale,
                    subpixel_variant: point(0, 0),
                    is_emoji: glyph.is_emoji,
                    subpixel_rendering: false,
                    dilation: 0,
                };
                if seen.insert(p.clone()) {
                    glyphs.push(p);
                }
            }
        }
        let raster = || -> Result<f64> {
            let start = Instant::now();
            for p in &glyphs {
                let bounds = backend.glyph_raster_bounds(p)?;
                if bounds.size.width.0 != 0 && bounds.size.height.0 != 0 {
                    black_box(backend.rasterize_glyph(p, bounds)?);
                }
            }
            Ok(micros(start))
        };
        let raster_first = raster()?;
        let mut times = (0..iterations).map(|_| raster()).collect::<Result<Vec<_>>>()?;
        times.sort_by(f64::total_cmp);
        results.push(Timing {
            layout_first,
            layout_warm,
            raster_first,
            raster_warm: times[times.len() / 2],
            width: f32::from(layout.width),
            faces: layout.runs.iter().map(|r| r.font_id).collect::<HashSet<_>>().len(),
            glyphs: glyphs.len(),
        });
    }
    Ok(results)
}
