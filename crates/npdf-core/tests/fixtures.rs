//! Tests against real world PDFs.
//!
//! The files are fetched with `npm run fetch:fixtures` and are not part of the
//! repository. When they are missing every test here reports a skip instead of
//! failing, so a run without network access still gives a meaningful result.

use std::collections::BTreeMap;
use std::path::PathBuf;

use npdf_core::doc::{DocumentSource, OpenOptions};
use npdf_core::edit::EditCommand;
use npdf_core::save::SaveMode;
use npdf_core::Session;

fn fixture_dir() -> PathBuf {
    // The test binary runs with the crate directory as its working directory.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/downloads")
}

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let dir = fixture_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, Vec<u8>)> = entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|e| e == "pdf"))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            std::fs::read(entry.path()).ok().map(|bytes| (name, bytes))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

fn skip_notice() {
    eprintln!(
        "skipped: no sample files in {}. Run: npm run fetch:fixtures",
        fixture_dir().display()
    );
}

fn source(name: &str) -> DocumentSource {
    DocumentSource {
        handle: None,
        display_name: name.to_string(),
    }
}

#[test]
fn every_sample_file_opens_and_reports_pages() {
    let files = fixtures();
    if files.is_empty() {
        skip_notice();
        return;
    }

    for (name, bytes) in files {
        let mut session = Session::with_null_platform();
        let summary = session
            .open_bytes(bytes.clone(), source(&name), &OpenOptions::default())
            .unwrap_or_else(|error| panic!("{name} did not open: {error}"));

        assert!(summary.page_count > 0, "{name} reports no pages");
        assert_eq!(summary.pages.len(), summary.page_count);
        assert_eq!(summary.byte_size, bytes.len());
        assert!(!summary.dirty);

        for page in &summary.pages {
            assert!(
                page.width_pt > 0.0,
                "{name} page {} has no width",
                page.index + 1
            );
            assert!(
                page.height_pt > 0.0,
                "{name} page {} has no height",
                page.index + 1
            );
            assert!(matches!(page.rotation, 0 | 90 | 180 | 270));
        }
    }
}

#[test]
fn text_can_be_extracted_from_every_sample_file() {
    let files = fixtures();
    if files.is_empty() {
        skip_notice();
        return;
    }

    for (name, bytes) in files {
        let mut session = Session::with_null_platform();
        let summary = session
            .open_bytes(bytes, source(&name), &OpenOptions::default())
            .unwrap();
        let document = session.get(summary.id).unwrap();

        let mut runs_found = 0;
        // Look at the first few pages only, the large sample has hundreds.
        for index in 0..summary.page_count.min(5) {
            let page = npdf_core::extract::extract_page_text(document, index)
                .unwrap_or_else(|error| panic!("{name} page {}: {error}", index + 1));
            assert_eq!(page.page_index, index);
            runs_found += page.runs.len();
            for run in &page.runs {
                assert!(run.font_size >= 0.0);
                assert!(run.effective_font_size.is_finite());
                assert!(run.origin.x.is_finite() && run.origin.y.is_finite());
            }
        }

        // Every sample except the pure image one carries text.
        if !name.contains("cmyk-image") {
            assert!(runs_found > 0, "{name} produced no text runs at all");
        }
    }
}

#[test]
fn an_incremental_save_keeps_the_original_bytes_and_reopens() {
    let files = fixtures();
    if files.is_empty() {
        skip_notice();
        return;
    }

    for (name, bytes) in files {
        let mut session = Session::with_null_platform();
        let summary = session
            .open_bytes(bytes.clone(), source(&name), &OpenOptions::default())
            .unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("Producer".to_string(), Some("NPDF".to_string()));
        session
            .apply(summary.id, EditCommand::SetDocumentInfo { fields })
            .unwrap_or_else(|error| panic!("{name}: metadata edit failed: {error}"));

        let (written, report) = session
            .save_to_bytes(summary.id, SaveMode::Incremental)
            .unwrap_or_else(|error| panic!("{name}: save failed: {error}"));

        assert!(
            report.validation.ok,
            "{name}: {:?}",
            report.validation.errors
        );
        assert_eq!(
            &written[..bytes.len()],
            bytes.as_slice(),
            "{name}: the original bytes were not preserved"
        );

        let mut second = Session::with_null_platform();
        let reopened = second
            .open_bytes(written, source(&name), &OpenOptions::default())
            .unwrap_or_else(|error| panic!("{name}: the saved file did not open again: {error}"));
        assert_eq!(
            reopened.page_count, summary.page_count,
            "{name}: the page count changed while saving"
        );
    }
}

#[test]
fn rotating_a_page_survives_a_save_in_every_sample_file() {
    let files = fixtures();
    if files.is_empty() {
        skip_notice();
        return;
    }

    for (name, bytes) in files {
        let mut session = Session::with_null_platform();
        let summary = session
            .open_bytes(bytes, source(&name), &OpenOptions::default())
            .unwrap();
        let before = summary.pages[0].rotation;
        let wanted = (before + 90) % 360;

        session
            .apply(
                summary.id,
                EditCommand::RotatePage {
                    page_index: 0,
                    degrees: wanted,
                },
            )
            .unwrap();
        let (written, _) = session
            .save_to_bytes(summary.id, SaveMode::Incremental)
            .unwrap();

        let mut second = Session::with_null_platform();
        let reopened = second
            .open_bytes(written, source(&name), &OpenOptions::default())
            .unwrap();
        assert_eq!(
            reopened.pages[0].rotation, wanted,
            "{name}: the rotation did not survive the save"
        );
        if reopened.page_count > 1 {
            assert_eq!(
                reopened.pages[1].rotation, summary.pages[1].rotation,
                "{name}: an untouched page changed"
            );
        }
    }
}
