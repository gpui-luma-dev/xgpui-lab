use anyhow::{Context, Result, ensure};
use std::path::PathBuf;

#[derive(Clone)]
pub struct Sample {
    pub name: String,
    pub description: &'static str,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}
pub struct Fixtures {
    pub samples: Vec<Sample>,
    _directory: tempfile::TempDir,
}
fn byte(value: f32) -> u8 {
    (value.clamp(0., 1.) * 255.).round() as u8
}
impl Fixtures {
    pub fn new(external: Option<PathBuf>) -> Result<Self> {
        let directory = tempfile::Builder::new().prefix("xgpui-media-lab-").tempdir()?;
        let mut samples = Vec::new();
        for (index, (name, description, width, height)) in [
            (
                "Frequency field",
                "Fine lines, a zone plate and woven detail reveal downscaling aliasing.",
                1536,
                1024,
            ),
            ("Alpha edges", "Feathered translucent shapes expose alpha handling and edge color.", 1024, 1024),
            (
                "Color terrain",
                "A synthetic color field with fine contour lines and smooth transitions.",
                1600,
                1000,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let pixels = image::RgbaImage::from_fn(width, height, |x, y| {
                let u = x as f32 / width as f32;
                let v = y as f32 / height as f32;
                let rgba = match index {
                    0 => {
                        let wave = if u < 0.5 {
                            if v < 0.5 {
                                ((x as f32 * 2.6 + y as f32 * 0.4).sin() * 0.5 + 0.5).powf(0.7)
                            } else {
                                if (x / 2 + y / 2) % 2 == 0 { 0. } else { 1. }
                            }
                        } else {
                            let dx = u - 0.75;
                            let dy = v - 0.5;
                            (12000. * (dx * dx + dy * dy)).sin() * 0.5 + 0.5
                        };
                        [byte(0.18 + wave * 0.68), byte(0.22 + wave * 0.68), byte(0.32 + wave * 0.62), 255]
                    }
                    1 => {
                        let mut rgb = [0., 0., 0.];
                        let mut alpha = 0.;
                        for (cx, cy, r, color) in [
                            (0.38, 0.40, 0.29, [0.20, 0.93, 0.77]),
                            (0.64, 0.48, 0.27, [0.98, 0.48, 0.28]),
                            (0.48, 0.67, 0.25, [0.49, 0.46, 0.98]),
                        ] {
                            let distance = ((u - cx).powi(2) + (v - cy).powi(2)).sqrt();
                            let a = ((r - distance) * 100.).clamp(0., 0.85);
                            for c in 0..3 {
                                rgb[c] = color[c] * a + rgb[c] * (1. - a);
                            }
                            alpha = a + alpha * (1. - a);
                        }
                        if alpha > 0. {
                            [byte(rgb[0] / alpha), byte(rgb[1] / alpha), byte(rgb[2] / alpha), byte(alpha)]
                        } else {
                            [0, 0, 255, 0]
                        }
                    }
                    _ => {
                        let ridge = ((u * 8. + (v * 6.).sin() * 0.4).sin() + (v * 10. + u * 3.).cos()) * 0.25 + 0.5;
                        let contour = (ridge * 170.).sin().abs();
                        let ink = if contour < 0.12 { 0.72 } else { 1. };
                        [
                            byte((0.12 + 0.85 * u) * ink),
                            byte((0.20 + 0.68 * ridge) * ink),
                            byte((0.52 + 0.32 * (1. - v)) * ink),
                            255,
                        ]
                    }
                };
                image::Rgba(rgba)
            });
            let path = directory.path().join(format!("sample-{index}.png"));
            pixels.save(&path)?;
            samples.push(Sample { name: name.into(), description, path, width, height });
        }
        if let Some(path) = external {
            let path = path.canonicalize().context("open custom image")?;
            let (width, height) = image::ImageReader::open(&path)?.with_guessed_format()?.into_dimensions()?;
            ensure!(
                width > 0 && height > 0 && width <= 16384 && height <= 16384,
                "image dimensions exceed media limits"
            );
            let mut bytes = 0u64;
            let (mut w, mut h) = (width, height);
            loop {
                bytes += w as u64 * h as u64 * 4;
                if w == 1 && h == 1 {
                    break;
                }
                w = (w / 2).max(1);
                h = (h / 2).max(1);
            }
            ensure!(
                bytes <= gpui::ImageTextureData::MAX_BYTES as u64,
                "custom image exceeds the 64 MiB full-mip payload limit"
            );
            ensure!(std::fs::metadata(&path)?.len() <= 64 * 1024 * 1024, "encoded custom image exceeds 64 MiB");
            samples.push(Sample { name:"Your image".into(),description:"Custom source; media assumes SDR sRGB and does not apply embedded ICC or EXIF orientation.",path,width,height });
        }
        Ok(Self { samples, _directory: directory })
    }
}
