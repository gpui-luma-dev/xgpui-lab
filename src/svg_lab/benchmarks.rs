//! Embedded CPU SVG benchmarks; corpus cases use fresh copies of this executable, not Python/source workers.
use anyhow::{Context as _, Result, ensure};
use gpui::{SvgRenderer, RenderImage};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use xgpui_svg::{SvgDocument, RasterPolicy, MIB};

pub const MANIFEST: &str = include_str!("corpus.json");
fn stats(samples: &[f64]) -> Value {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    json!({"samples_ms": samples, "median_ms": sorted[sorted.len()/2], "max_ms": sorted.last()})
}
fn rss() -> Option<u64> {
    let mut system = sysinfo::System::new();
    let pid = sysinfo::get_current_pid().ok()?;
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        sysinfo::ProcessRefreshKind::new().with_memory(),
    );
    system.process(pid).map(|process| process.memory())
}
fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take((32 * MIB + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 32 * MIB, "SVG exceeds 32 MiB input limit");
    Ok(bytes)
}
fn save(image: &RenderImage, output: &Path) -> Result<()> {
    let size = image.size(0);
    let mut pixels = image.as_bytes(0).context("missing SVG pixels")?.to_vec();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    image::save_buffer(output, &pixels, size.width.0 as u32, size.height.0 as u32, image::ColorType::Rgba8)?;
    Ok(())
}
pub fn measure(bytes: &[u8], zoom: f32, budget: usize, output: &Path, cancel: &AtomicBool) -> Result<Value> {
    ensure!(zoom.is_finite() && (0.25..=16.).contains(&zoom), "zoom must be 0.25..=16");
    crate::benchmarks::check_cancel(cancel)?;
    let rss_before = rss();
    let renderer = SvgRenderer::new(Arc::new(()));
    let start = Instant::now();
    let document = SvgDocument::parse(&renderer, bytes)?;
    let first_parse = start.elapsed().as_secs_f64() * 1000.;
    let logical = document.fit_size(320.)?;
    let policy = RasterPolicy { max_output_bytes: budget, ..Default::default() };
    let resolution = policy.resolution(logical, zoom, 2.)?;
    let base_size = policy.resolution(logical, 1., 1.)?.output;
    let base = document.rasterize(&renderer, base_size, budget)?;
    let mut previous = document.rasterize(&renderer, resolution.output, budget)?;
    let first_raster = previous.ms;
    let mut parse = Vec::new();
    let mut raster = Vec::new();
    let mut sampled_rss_peak = rss_before;
    for _ in 0..5 {
        crate::benchmarks::check_cancel(cancel)?;
        let start = Instant::now();
        let fresh = SvgDocument::parse(&renderer, bytes)?;
        parse.push(start.elapsed().as_secs_f64() * 1000.);
        let next = fresh.rasterize(&renderer, resolution.output, budget)?;
        raster.push(next.ms);
        sampled_rss_peak = sampled_rss_peak.into_iter().chain(rss()).max();
        previous = next;
    }
    let pixels = previous.image.as_bytes(0).context("missing SVG pixels")?;
    let visible = pixels.as_chunks::<4>().0.iter().filter(|pixel| pixel[3] != 0).count();
    save(&previous.image, &output.join("capture.png"))?;
    Ok(json!({"input_bytes": bytes.len(), "zoom": zoom, "density": 2, "budget_bytes": budget,
        "parse_first_ms": first_parse, "parse": stats(&parse), "raster_first_ms": first_raster, "raster": stats(&raster),
        "pixel_width": resolution.output.width.0, "pixel_height": resolution.output.height.0, "resolution_limited": resolution.limited(),
        "visible_pixels": visible, "output_bytes": pixels.len(), "base_bytes": base.image.as_bytes(0).context("base pixels")?.len(),
        "rss_start_bytes": rss_before, "sampled_process_rss_peak_bytes": sampled_rss_peak,
        "measurement": "CPU parse/raster samples; base and previous zoom retained during replacement. PNG encoding excluded. RSS covers the whole process; not GPU memory or per-document allocation.", "errors": []}))
}
pub fn builtin(output: &Path, cancel: &AtomicBool) -> Result<Value> {
    measure(super::scene::svg(false, true).as_bytes(), 4., 16 * MIB, output, cancel)
}
pub fn selected(path: &Path, output: &Path, cancel: &AtomicBool) -> Result<Value> {
    measure(&read(path)?, 4., 16 * MIB, output, cancel)
}
const SAMPLES: &[(&str, &[u8])] = &[
    ("AJ_Digital_Camera", include_bytes!("../../assets/samples/svg/AJ_Digital_Camera.svg").as_slice()),
    ("rg1024_green_grapes", include_bytes!("../../assets/samples/svg/rg1024_green_grapes.svg").as_slice()),
    ("rg1024_metal_effect", include_bytes!("../../assets/samples/svg/rg1024_metal_effect.svg").as_slice()),
    ("dragon", include_bytes!("../../assets/samples/svg/dragon.svg").as_slice()),
];
/// Materialize embedded repository fixtures for isolated case processes.
pub fn bundled_corpus(output: &Path, cancel: &AtomicBool) -> Result<Value> {
    let inputs = output.join("inputs");
    std::fs::create_dir_all(&inputs)?;
    for (id, bytes) in SAMPLES {
        crate::benchmarks::check_cancel(cancel)?;
        std::fs::write(inputs.join(format!("{id}.svg")), bytes)?;
    }
    corpus(&inputs, output, cancel)
}
fn verify_hash(bytes: &[u8], expected: &str) -> Result<()> {
    ensure!(format!("{:x}", Sha256::digest(bytes)) == expected, "corpus SHA-256 mismatch");
    Ok(())
}
pub fn corpus(folder: &Path, output: &Path, cancel: &AtomicBool) -> Result<Value> {
    let manifest: Vec<Value> = serde_json::from_str(MANIFEST)?;
    // Validate all supplied documents before starting any timed case.
    for item in &manifest {
        crate::benchmarks::check_cancel(cancel)?;
        let path = folder.join(format!("{}.svg", item["id"].as_str().context("manifest id")?));
        verify_hash(
            &read(&path).with_context(|| format!("Corpus file {}", path.display()))?,
            item["sha256"].as_str().context("manifest hash")?,
        )
        .with_context(|| format!("Corpus file {}", path.display()))?;
    }
    let mut cases = Vec::new();
    for item in manifest {
        let id = item["id"].as_str().context("manifest id")?;
        let source = folder.join(format!("{id}.svg"));
        for zoom in [1, 4, 8, 16] {
            crate::benchmarks::check_cancel(cancel)?;
            let directory = output.join(format!("{id}-{zoom}x"));
            std::fs::create_dir_all(&directory)?;
            let args = vec![
                "--svg-case".to_owned(),
                source.display().to_string(),
                "--zoom".into(),
                zoom.to_string(),
                "--output".into(),
                directory.display().to_string(),
            ];
            let result = crate::benchmarks::run_self(&args, &directory, cancel);
            cases.push(match result {
                Ok(mut report) => {
                    report["id"] = json!(id);
                    report["sha256"] = item["sha256"].clone();
                    report["source"] = item["url"].clone();
                    report
                }
                Err(error) => json!({"id": id, "zoom": zoom, "error": format!("{error:#}")}),
            });
            std::fs::write(output.join("raw.json"), serde_json::to_vec_pretty(&json!({"cases": cases}))?)?;
        }
    }
    let failures = cases.iter().filter(|case| case.get("error").is_some()).count();
    Ok(
        json!({"cases": cases, "errors": if failures == 0 { vec![] } else { vec![format!("{failures} corpus cases failed")] },
        "measurement": "Hash-verified source manifest, one fresh copy of this executable per document/zoom; five parse/raster repeats, zooms 1/4/8/16 at density 2, 256 MiB output budget. No runtime compilation or Python."}),
    )
}
pub fn run_case_cli(args: &[String]) -> Result<()> {
    let source = crate::benchmarks::argument(args, "--svg-case").context("SVG path required")?;
    let output = crate::benchmarks::argument(args, "--output").context("output directory required")?;
    let zoom = crate::benchmarks::argument(args, "--zoom").unwrap_or("4").parse()?;
    let output = Path::new(output);
    std::fs::create_dir_all(output)?;
    let result = measure(&read(Path::new(source))?, zoom, 256 * MIB, output, &AtomicBool::new(false))?;
    std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&result)?)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn bundled_samples_match_the_manifest() {
        let manifest: Vec<Value> = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(manifest.len(), SAMPLES.len());
        for (id, bytes) in SAMPLES {
            let entry = manifest.iter().find(|item| item["id"] == *id).unwrap();
            verify_hash(bytes, entry["sha256"].as_str().unwrap()).unwrap();
            assert!(entry["local"].as_str().unwrap().starts_with("assets/samples/"));
        }
    }
    #[::core::prelude::v1::test]
    fn corpus_hash_rejects_changed_bytes() {
        let hash = format!("{:x}", Sha256::digest(b"fixture"));
        assert!(verify_hash(b"fixture", &hash).is_ok());
        assert!(verify_hash(b"changed", &hash).is_err());
    }
    #[::core::prelude::v1::test]
    fn cancelled_svg_does_not_parse_or_write_a_preview() {
        let output = tempfile::tempdir().unwrap();
        assert!(builtin(output.path(), &AtomicBool::new(true)).is_err());
        assert!(!output.path().join("capture.png").exists());
    }
}
