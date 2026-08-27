//! Fonts.
//!
//! Text editing lives or dies on this module. To put a character into an
//! existing line we need the advance width the original font would have used,
//! and we need to know whether the embedded subset even contains the glyph.
//!
//! M0 provides the font program side: parsing a TrueType or OpenType file,
//! glyph lookup and advance widths in PDF glyph space. The PDF side, encoding
//! tables, `/Differences`, CID maps and `/ToUnicode`, follows in M2 and M3, and
//! subset embedding in M4.

mod discovery;

use serde::{Deserialize, Serialize};
use ttf_parser::{Face, GlyphId};

use crate::error::{Error, Result};

pub use discovery::{discover_fonts, SystemFont};

/// PDF measures glyph widths in thousandths of the text space unit.
pub const PDF_GLYPH_SPACE: f64 = 1000.0;

/// A parsed font program.
///
/// The bytes are kept and the face is parsed on demand. `ttf_parser::Face`
/// borrows its data, and a struct that owns both would have to be self
/// referential. Parsing is cheap, it only reads the table directory.
#[derive(Clone)]
pub struct FontProgram {
    data: Vec<u8>,
    face_index: u32,
    units_per_em: u16,
}

impl std::fmt::Debug for FontProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontProgram")
            .field("bytes", &self.data.len())
            .field("units_per_em", &self.units_per_em)
            .field("family", &self.family_name())
            .finish()
    }
}

impl FontProgram {
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        Self::parse_indexed(data, 0)
    }

    /// Parse one face out of a collection file such as a `.ttc`.
    pub fn parse_indexed(data: Vec<u8>, face_index: u32) -> Result<Self> {
        let units_per_em = {
            let face = Face::parse(&data, face_index)
                .map_err(|e| Error::Font(format!("the font could not be parsed: {e}")))?;
            face.units_per_em()
        };
        if units_per_em == 0 {
            return Err(Error::Font("the font reports zero units per em".into()));
        }
        Ok(Self {
            data,
            face_index,
            units_per_em,
        })
    }

    fn face(&self) -> Result<Face<'_>> {
        Face::parse(&self.data, self.face_index)
            .map_err(|e| Error::Font(format!("the font could not be parsed: {e}")))
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    pub fn family_name(&self) -> Option<String> {
        let face = self.face().ok()?;
        face.names()
            .into_iter()
            .find(|name| name.name_id == ttf_parser::name_id::FAMILY && name.is_unicode())
            .and_then(|name| name.to_string())
    }

    pub fn post_script_name(&self) -> Option<String> {
        let face = self.face().ok()?;
        face.names()
            .into_iter()
            .find(|name| name.name_id == ttf_parser::name_id::POST_SCRIPT_NAME && name.is_unicode())
            .and_then(|name| name.to_string())
    }

    pub fn is_bold(&self) -> bool {
        self.face().map(|f| f.is_bold()).unwrap_or(false)
    }

    pub fn is_italic(&self) -> bool {
        self.face().map(|f| f.is_italic()).unwrap_or(false)
    }

    pub fn glyph_count(&self) -> u16 {
        self.face().map(|f| f.number_of_glyphs()).unwrap_or(0)
    }

    /// Glyph id for a character through the font's own character map.
    pub fn glyph_index(&self, character: char) -> Option<u16> {
        let face = self.face().ok()?;
        face.glyph_index(character).map(|g| g.0)
    }

    pub fn has_glyph(&self, character: char) -> bool {
        self.glyph_index(character).is_some()
    }

    /// Advance width of a glyph in font design units.
    pub fn advance_units(&self, glyph: u16) -> Option<u16> {
        let face = self.face().ok()?;
        face.glyph_hor_advance(GlyphId(glyph))
    }

    /// Advance width of a character in PDF glyph space, thousandths of the text
    /// space unit. This is the number that goes into a `/Widths` array.
    pub fn advance_pdf_units(&self, character: char) -> Option<f64> {
        let glyph = self.glyph_index(character)?;
        let advance = self.advance_units(glyph)?;
        Some(advance as f64 * PDF_GLYPH_SPACE / self.units_per_em as f64)
    }

    /// Width of a string at a given font size, in points, ignoring kerning and
    /// character spacing. Returns `None` as soon as one character is missing,
    /// because a partial width would silently produce a wrong line.
    pub fn measure(&self, text: &str, font_size: f64) -> Option<f64> {
        let mut total = 0.0;
        for character in text.chars() {
            total += self.advance_pdf_units(character)?;
        }
        Some(total / PDF_GLYPH_SPACE * font_size)
    }

    /// Every character of `text` the font cannot draw. This is the check that
    /// decides between reusing the embedded font and embedding a new subset.
    pub fn missing_glyphs(&self, text: &str) -> Vec<char> {
        let Ok(face) = self.face() else {
            return text.chars().collect();
        };
        let mut missing: Vec<char> = Vec::new();
        for character in text.chars() {
            if face.glyph_index(character).is_none() && !missing.contains(&character) {
                missing.push(character);
            }
        }
        missing
    }

    /// The values a `/FontDescriptor` needs, in PDF glyph space.
    pub fn descriptor_metrics(&self) -> Result<FontMetrics> {
        let face = self.face()?;
        let scale = PDF_GLYPH_SPACE / self.units_per_em as f64;
        Ok(FontMetrics {
            ascent: face.ascender() as f64 * scale,
            descent: face.descender() as f64 * scale,
            line_gap: face.line_gap() as f64 * scale,
            cap_height: face.capital_height().map(|v| v as f64 * scale),
            x_height: face.x_height().map(|v| v as f64 * scale),
            italic_angle: face.italic_angle() as f64,
            bbox: [
                face.global_bounding_box().x_min as f64 * scale,
                face.global_bounding_box().y_min as f64 * scale,
                face.global_bounding_box().x_max as f64 * scale,
                face.global_bounding_box().y_max as f64 * scale,
            ],
            is_monospaced: face.is_monospaced(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontMetrics {
    pub ascent: f64,
    pub descent: f64,
    pub line_gap: f64,
    pub cap_height: Option<f64>,
    pub x_height: Option<f64>,
    pub italic_angle: f64,
    /// `[x_min, y_min, x_max, y_max]`.
    pub bbox: [f64; 4],
    pub is_monospaced: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Find any font on the machine so the parser has real input. Returns `None`
    /// in a container without fonts, in which case the test reports a skip
    /// instead of failing.
    fn any_system_font() -> Option<FontProgram> {
        for font in discover_fonts(&crate::platform::default_font_dirs()) {
            if let Ok(data) = std::fs::read(&font.path) {
                if let Ok(program) = FontProgram::parse(data) {
                    return Some(program);
                }
            }
        }
        None
    }

    #[test]
    fn rejects_data_that_is_not_a_font() {
        let error = FontProgram::parse(b"definitely not a font".to_vec()).unwrap_err();
        assert_eq!(error.code(), "font");
    }

    #[test]
    fn reads_metrics_and_widths_from_a_real_font() {
        let Some(font) = any_system_font() else {
            eprintln!("skipped: no system font available in this environment");
            return;
        };
        assert!(font.units_per_em() > 0);
        assert!(font.glyph_count() > 0);

        let width = font
            .advance_pdf_units('A')
            .expect("a text font can draw the letter A");
        assert!(
            width > 100.0 && width < 2000.0,
            "an A that is {width} units wide is not plausible"
        );

        // A space is never wider than an M. In a proportional font it is
        // strictly narrower, in a monospaced one every glyph has the same width.
        // Which kind of font we found first depends on the machine, so decide
        // from the widths rather than from the flag in the post table, which not
        // every font sets correctly.
        let space = font.advance_pdf_units(' ').unwrap();
        let em = font.advance_pdf_units('M').unwrap();
        let narrow = font.advance_pdf_units('i').unwrap();
        assert!(space <= em, "space {space} is wider than M {em}");
        if (narrow - em).abs() > 1e-6 {
            assert!(space < em, "space {space} should be narrower than M {em}");
        }

        let measured = font.measure("AA", 10.0).unwrap();
        assert!((measured - 2.0 * width / PDF_GLYPH_SPACE * 10.0).abs() < 1e-9);

        let metrics = font.descriptor_metrics().unwrap();
        assert!(metrics.ascent > 0.0);
        assert!(metrics.descent < 0.0);
    }

    #[test]
    fn missing_glyphs_are_reported_without_duplicates() {
        let Some(font) = any_system_font() else {
            eprintln!("skipped: no system font available in this environment");
            return;
        };
        // A private use character is not in any normal text font.
        let missing = font.missing_glyphs("AB\u{E000}C\u{E000}");
        assert_eq!(missing, vec!['\u{E000}']);
        assert!(font.measure("A\u{E000}", 12.0).is_none());
    }
}
