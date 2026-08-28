//! Writing documents back out.
//!
//! [`SaveMode::Incremental`] is the default and the reason the whole document
//! model is built the way it is: the original bytes are copied unchanged and
//! only the objects an edit touched are appended. Everything we never looked at
//! survives exactly as the producing application wrote it.
//!
//! [`SaveMode::Full`] rewrites the file from the parsed structure. It produces
//! smaller files but can only preserve what lopdf understands, so it is offered
//! as an explicit export, never as the silent default.

use serde::{Deserialize, Serialize};

use crate::doc::Document;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SaveMode {
    /// Append the changed objects, keep the original bytes.
    #[default]
    Incremental,
    /// Rewrite the whole file from the parsed structure.
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReport {
    pub mode: SaveMode,
    pub byte_size: usize,
    /// How many bytes were appended. Zero for a full rewrite.
    pub appended_bytes: usize,
    /// How many objects the update layer carried.
    pub changed_objects: usize,
    pub validation: ValidationReport,
}

/// Serialise the document.
pub fn save_to_bytes(doc: &mut Document, mode: SaveMode) -> Result<(Vec<u8>, SaveReport)> {
    let original_len = doc.original_bytes().len();
    let changed_objects = doc.incremental().new_document.objects.len();

    let mut buffer: Vec<u8> = Vec::with_capacity(original_len + 4096);
    match mode {
        SaveMode::Incremental => {
            doc.incremental_mut()
                .save_to(&mut buffer)
                .map_err(|e| Error::Save(e.to_string()))?;
            // The incremental writer must never touch the prefix. Checking it
            // here turns a future regression into a failed save instead of a
            // silently damaged file.
            if buffer.len() < original_len || &buffer[..original_len] != doc.original_bytes() {
                return Err(Error::Save(
                    "the incremental writer changed the original bytes, refusing to save".into(),
                ));
            }
        }
        SaveMode::Full => {
            doc.flattened()
                .save_to(&mut buffer)
                .map_err(|e| Error::Save(e.to_string()))?;
        }
    }

    let validation = validate(&buffer);
    if !validation.ok {
        return Err(Error::Save(format!(
            "the written file did not pass validation: {}",
            validation.errors.join("; ")
        )));
    }

    let report = SaveReport {
        mode,
        byte_size: buffer.len(),
        appended_bytes: match mode {
            SaveMode::Incremental => buffer.len().saturating_sub(original_len),
            SaveMode::Full => 0,
        },
        changed_objects,
        validation: validation.clone(),
    };
    Ok((buffer, report))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub ok: bool,
    pub pdf_version: String,
    pub page_count: usize,
    pub has_trailing_eof: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Re-parse written bytes and check the things a viewer will check.
///
/// This is deliberately paranoid. Every save runs through it, because a file
/// that Acrobat refuses to open is the one failure mode this project cannot
/// afford.
pub fn validate(bytes: &[u8]) -> ValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if !bytes.starts_with(b"%PDF-") {
        errors.push("the file does not start with %PDF-".to_string());
    }

    // %%EOF may be followed by white space, so look at the tail rather than the
    // very last bytes.
    let tail_start = bytes.len().saturating_sub(1024);
    let has_trailing_eof = bytes[tail_start..]
        .windows(5)
        .any(|window| window == b"%%EOF");
    if !has_trailing_eof {
        errors.push("no %%EOF marker near the end of the file".to_string());
    }

    let (pdf_version, page_count) = match lopdf::Document::load_mem(bytes) {
        Ok(doc) => {
            let pages = doc.get_pages().len();
            if pages == 0 {
                errors.push("the written document has no pages".to_string());
            }
            if doc.catalog().is_err() {
                errors.push("the written document has no readable catalog".to_string());
            }
            if doc.trailer.get(b"Root").is_err() {
                errors.push("the trailer of the written document has no /Root".to_string());
            }
            (doc.version.clone(), pages)
        }
        Err(error) => {
            errors.push(format!("the written file cannot be parsed again: {error}"));
            (String::new(), 0)
        }
    };

    if bytes.len() > 2 * 1024 * 1024 * 1024 {
        warnings.push("the file is larger than two gigabytes".to_string());
    }

    ValidationReport {
        ok: errors.is_empty(),
        pdf_version,
        page_count,
        has_trailing_eof,
        errors,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{DocumentId, DocumentSource, OpenOptions};
    use crate::edit::{self, EditCommand};
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
    fn an_untouched_document_saves_to_a_readable_file() {
        let original = testutil::simple_text_pdf(2);
        let mut doc = open(original.clone());
        let (bytes, report) = save_to_bytes(&mut doc, SaveMode::Incremental).unwrap();
        assert!(report.validation.ok, "{:?}", report.validation.errors);
        assert_eq!(report.validation.page_count, 2);
        assert_eq!(report.changed_objects, 0);
        assert_eq!(&bytes[..original.len()], original.as_slice());
    }

    #[test]
    fn an_edit_is_appended_and_survives_a_reopen() {
        let original = testutil::simple_text_pdf(3);
        let mut doc = open(original.clone());
        edit::apply(
            &mut doc,
            EditCommand::RotatePage {
                page_index: 2,
                degrees: 90,
            },
        )
        .unwrap();

        let (bytes, report) = save_to_bytes(&mut doc, SaveMode::Incremental).unwrap();
        assert_eq!(report.changed_objects, 1);
        assert!(report.appended_bytes > 0);
        // The whole original file is still in there, byte for byte.
        assert_eq!(&bytes[..original.len()], original.as_slice());

        let reopened = open(bytes);
        assert_eq!(reopened.page_count(), 3);
        assert_eq!(reopened.page_info(2).unwrap().rotation, 90);
        assert_eq!(reopened.page_info(0).unwrap().rotation, 0);
    }

    #[test]
    fn a_full_rewrite_keeps_the_pages() {
        let mut doc = open(testutil::simple_text_pdf(2));
        edit::apply(
            &mut doc,
            EditCommand::RotatePage {
                page_index: 0,
                degrees: 180,
            },
        )
        .unwrap();
        let (bytes, report) = save_to_bytes(&mut doc, SaveMode::Full).unwrap();
        assert!(report.validation.ok, "{:?}", report.validation.errors);
        assert_eq!(report.appended_bytes, 0);
        let reopened = open(bytes);
        assert_eq!(reopened.page_count(), 2);
        assert_eq!(reopened.page_info(0).unwrap().rotation, 180);
    }

    #[test]
    fn validation_rejects_rubbish() {
        let report = validate(b"not a pdf");
        assert!(!report.ok);
        assert!(!report.errors.is_empty());
    }
}
