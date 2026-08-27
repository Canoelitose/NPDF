//! Reading what is actually on a page.
//!
//! The content stream is a little stack machine. To know where a piece of text
//! sits on the page we have to replay it: the current transformation matrix, the
//! text matrix and the text state all change as operators go by.
//!
//! M0 delivers the replay and the raw runs. Grouping runs into lines and
//! paragraphs and decoding character codes through the font encoding is M2, and
//! marked as such below.

mod state;

use lopdf::content::Content;
use lopdf::{Object, ObjectId};
use serde::{Deserialize, Serialize};

use crate::doc::Document;
use crate::error::{Error, Result};
use crate::geom::{Matrix, Point};

pub use state::{GraphicsState, TextState};

/// One item of a text showing operator. `TJ` mixes strings and numbers, and the
/// numbers matter, they are the kerning we have to preserve when editing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum ShowItem {
    /// Raw character codes exactly as they appear in the file. Decoding them
    /// needs the font encoding, which is M2.
    Text(Vec<u8>),
    /// A displacement in thousandths of a text space unit, subtracted from the
    /// current position.
    Adjust(f64),
}

/// One text showing operator with the state that was active when it ran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRun {
    pub page_index: usize,
    /// Which content stream of the page this came from.
    pub stream_index: usize,
    /// Position of the operator inside the decoded stream. This is the anchor an
    /// edit uses to find the operator again without re-parsing by text.
    pub operation_index: usize,
    /// The operator itself: `Tj`, `TJ`, `'` or `"`.
    pub operator: String,
    pub items: Vec<ShowItem>,
    /// Resource name of the font, for example `F1`.
    pub font_resource: String,
    pub font_size: f64,
    pub char_spacing: f64,
    pub word_spacing: f64,
    /// Horizontal scaling as a factor, so 100 percent is 1.0.
    pub horizontal_scale: f64,
    pub rise: f64,
    pub render_mode: i64,
    /// Text matrix at the start of the run.
    pub text_matrix: Matrix,
    /// Current transformation matrix at the start of the run.
    pub ctm: Matrix,
    /// Where the first glyph sits in page coordinates.
    pub origin: Point,
    /// Effective font size on the page, after all matrices.
    pub effective_font_size: f64,
}

impl TextRun {
    /// All character codes of the run, concatenated.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for item in &self.items {
            if let ShowItem::Text(bytes) = item {
                out.extend_from_slice(bytes);
            }
        }
        out
    }

    /// A rough, single byte reading of the codes, good enough for the debug
    /// overlay in M2. Real decoding goes through the font encoding and the
    /// `/ToUnicode` map and lands with the font work.
    pub fn text_lossy(&self) -> String {
        self.bytes().iter().map(|&b| b as char).collect()
    }

    /// Whether the run is invisible, which is how OCR text layers are stored.
    pub fn is_invisible(&self) -> bool {
        self.render_mode == 3
    }
}

/// Everything the frontend needs to draw the debug overlay for one page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageText {
    pub page_index: usize,
    pub runs: Vec<TextRun>,
    /// Resource names of every font the page references.
    pub fonts: Vec<String>,
}

/// The decoded content streams of a page, one entry per stream object.
pub fn page_content_streams(doc: &Document, page_index: usize) -> Result<Vec<(ObjectId, Vec<u8>)>> {
    let page_id = doc.page_id(page_index)?;
    let dict = doc.get_dictionary(page_id)?;
    let contents = match dict.get(b"Contents") {
        Ok(contents) => contents,
        // A page without content is legal, it is simply empty.
        Err(_) => return Ok(Vec::new()),
    };

    let mut ids: Vec<ObjectId> = Vec::new();
    match contents {
        Object::Reference(id) => ids.push(*id),
        Object::Array(items) => {
            for item in items {
                if let Ok(id) = item.as_reference() {
                    ids.push(id);
                }
            }
        }
        _ => {}
    }

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let stream = match doc.get_object(id).and_then(|o| Ok(o.as_stream()?)) {
            Ok(stream) => stream,
            // Skip an unreadable stream instead of losing the whole page.
            Err(_) => continue,
        };
        let data = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        out.push((id, data));
    }
    Ok(out)
}

/// Replay the content streams of a page and collect every text showing operator.
pub fn extract_page_text(doc: &Document, page_index: usize) -> Result<PageText> {
    let streams = page_content_streams(doc, page_index)?;
    let mut runs = Vec::new();
    // The streams of a page behave as one single stream, so graphics state
    // carries over from one to the next.
    let mut state = GraphicsState::default();
    let mut stack: Vec<GraphicsState> = Vec::new();

    for (stream_index, (_, data)) in streams.iter().enumerate() {
        let content = Content::decode(data)
            .map_err(|e| Error::ContentStream(format!("page {}: {e}", page_index + 1)))?;
        state::replay(
            &content,
            &mut state,
            &mut stack,
            &mut |operation_index, operator, items, state| {
                runs.push(build_run(
                    page_index,
                    stream_index,
                    operation_index,
                    operator,
                    items,
                    state,
                ));
            },
        );
    }

    Ok(PageText {
        page_index,
        fonts: page_font_names(doc, page_index)?,
        runs,
    })
}

fn build_run(
    page_index: usize,
    stream_index: usize,
    operation_index: usize,
    operator: &str,
    items: Vec<ShowItem>,
    state: &GraphicsState,
) -> TextRun {
    let text = &state.text;
    // Text rendering matrix, see the PDF specification, text space to user space:
    // [Tfs * Th, 0, 0, Tfs, 0, Trise] x Tm x CTM
    let scale = Matrix::new(
        text.font_size * text.horizontal_scale,
        0.0,
        0.0,
        text.font_size,
        0.0,
        text.rise,
    );
    let render = scale.then(&text.matrix).then(&state.ctm);

    TextRun {
        page_index,
        stream_index,
        operation_index,
        operator: operator.to_string(),
        items,
        font_resource: text.font_resource.clone(),
        font_size: text.font_size,
        char_spacing: text.char_spacing,
        word_spacing: text.word_spacing,
        horizontal_scale: text.horizontal_scale,
        rise: text.rise,
        render_mode: text.render_mode,
        text_matrix: text.matrix,
        ctm: state.ctm,
        origin: render.apply(Point::new(0.0, 0.0)),
        effective_font_size: text.font_size * text.matrix.then(&state.ctm).y_scale(),
    }
}

/// Resource names of the fonts a page can use.
pub fn page_font_names(doc: &Document, page_index: usize) -> Result<Vec<String>> {
    let page_id = doc.page_id(page_index)?;
    let Some(resources) = doc.inherited(page_id, b"Resources") else {
        return Ok(Vec::new());
    };
    let Ok(resources) = resources.as_dict() else {
        return Ok(Vec::new());
    };
    let Ok(fonts) = resources.get(b"Font") else {
        return Ok(Vec::new());
    };
    let Ok(fonts) = doc.resolve(fonts) else {
        return Ok(Vec::new());
    };
    let Ok(fonts) = fonts.as_dict() else {
        return Ok(Vec::new());
    };
    Ok(fonts
        .iter()
        .map(|(name, _)| String::from_utf8_lossy(name).to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{DocumentId, DocumentSource, OpenOptions};
    use crate::testutil;

    fn open(bytes: Vec<u8>) -> Document {
        Document::open(
            DocumentId(1),
            bytes,
            DocumentSource {
                handle: None,
                display_name: "test.pdf".into(),
            },
            &OpenOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn finds_both_runs_of_a_synthetic_page() {
        let doc = open(testutil::simple_text_pdf(1));
        let text = extract_page_text(&doc, 0).unwrap();
        assert_eq!(text.runs.len(), 2);
        assert_eq!(text.fonts, vec!["F1".to_string()]);

        let first = &text.runs[0];
        assert_eq!(first.operator, "Tj");
        assert_eq!(first.text_lossy(), "Seite 1");
        assert_eq!(first.font_resource, "F1");
        assert!((first.font_size - 24.0).abs() < 1e-9);
        assert!(
            (first.origin.x - 72.0).abs() < 1e-9,
            "x was {}",
            first.origin.x
        );
        assert!(
            (first.origin.y - 720.0).abs() < 1e-9,
            "y was {}",
            first.origin.y
        );
        assert!((first.effective_font_size - 24.0).abs() < 1e-9);

        let second = &text.runs[1];
        assert_eq!(second.text_lossy(), "Testdokument fuer NPDF");
        assert!((second.origin.y - 690.0).abs() < 1e-9);
        assert!((second.font_size - 11.0).abs() < 1e-9);
    }

    #[test]
    fn state_carries_over_between_the_streams_of_one_page() {
        let doc = open(testutil::split_stream_pdf(1));
        let text = extract_page_text(&doc, 0).unwrap();
        assert_eq!(text.runs.len(), 2);
        assert_eq!(text.runs[0].stream_index, 0);
        assert_eq!(text.runs[1].stream_index, 1);
        assert_eq!(text.runs[1].text_lossy(), "Testdokument fuer NPDF");
    }

    #[test]
    fn an_unknown_page_is_an_error_and_not_a_panic() {
        let doc = open(testutil::simple_text_pdf(1));
        assert_eq!(
            extract_page_text(&doc, 5).unwrap_err().code(),
            "unknown_page"
        );
    }
}
