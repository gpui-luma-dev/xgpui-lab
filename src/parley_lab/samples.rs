use anyhow::{Context, Result};
use gpui::{Font, FontFallbacks, FontWeight, TextRun, font, rgb};
use skrifa::{FontRef, MetadataProvider};

pub const PLEX: &[u8] = include_bytes!("../../assets/samples/fonts/IBMPlexSans-Regular.ttf");
pub const INTER: &[u8] = include_bytes!("../../assets/samples/fonts/InterVariable.ttf");

pub struct Sample {
    pub label: String,
    pub text: String,
    pub font: Font,
}
impl Sample {
    pub fn runs(&self) -> Vec<TextRun> {
        vec![TextRun {
            len: self.text.len(),
            font: self.font.clone(),
            color: rgb(0xeff4fc).into(),
            ..Default::default()
        }]
    }
}

pub fn samples() -> Result<Vec<Sample>> {
    let primary = FontRef::new(INTER)?;
    let fallback = FontRef::new(PLEX)?;
    let ch = (0x00c0..0x0250)
        .filter_map(char::from_u32)
        .find(|&ch| primary.charmap().map(ch).is_none() && fallback.charmap().map(ch).is_some())
        .context("fixtures need a fallback-only Latin character")?;
    let mut fallback_font = font("Inter Variable");
    fallback_font.fallbacks = Some(FontFallbacks::from_fonts(vec!["IBM Plex Sans".into()]));
    let mut samples = vec![
        Sample {
            label: "Plex / ligatures".into(),
            text: "office affine efficient".into(),
            font: font("IBM Plex Sans"),
        },
        Sample {
            label: "Plex / accents and combining marks".into(),
            text: "café naïve e\u{301} Ångström".into(),
            font: font("IBM Plex Sans"),
        },
        Sample {
            label: format!("Inter -> Plex / fallback U+{:04X}", ch as u32),
            text: format!("H{ch}H  office {ch} affine"),
            font: fallback_font,
        },
    ];
    for weight in [100.0, 400.0, 900.0] {
        let mut face = font("Inter Variable");
        face.weight = FontWeight(weight);
        samples.push(Sample {
            label: format!("Inter Variable / weight {weight}"),
            text: "Hamburgefonts 012345".into(),
            font: face,
        });
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corpus_has_real_fallback_and_three_variable_weights() -> Result<()> {
        let rows = samples()?;
        let ch = rows[2].text.chars().nth(1).unwrap();
        assert!(FontRef::new(INTER)?.charmap().map(ch).is_none());
        assert!(FontRef::new(PLEX)?.charmap().map(ch).is_some());
        assert_eq!(rows[3..].iter().map(|s| s.font.weight.0).collect::<Vec<_>>(), vec![100.0, 400.0, 900.0]);
        for sample in rows {
            assert_eq!(sample.runs()[0].len, sample.text.len());
        }
        Ok(())
    }
}
