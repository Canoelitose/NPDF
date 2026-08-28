//! End to end tests for the M0 walking skeleton.
//!
//! Open a document, change it, undo it, save it incrementally, open the result
//! again. Where PDFium is available the same run also renders before and after
//! and proves with the pixel comparison that nothing outside the edit moved.

use npdf_core::doc::{DocumentSource, OpenOptions};
use npdf_core::edit::EditCommand;
use npdf_core::render::{diff, RenderRequest};
use npdf_core::save::SaveMode;
use npdf_core::{testutil, Session};

fn source(name: &str) -> DocumentSource {
    DocumentSource {
        handle: None,
        display_name: name.to_string(),
    }
}

#[test]
fn the_whole_loop_from_open_to_reopen() {
    let original = testutil::simple_text_pdf(4);
    let mut session = Session::with_null_platform();

    let opened = session
        .open_bytes(
            original.clone(),
            source("loop.pdf"),
            &OpenOptions::default(),
        )
        .unwrap();
    assert_eq!(opened.page_count, 4);
    assert!(!opened.dirty);

    // Two edits, then take one of them back.
    session
        .apply(
            opened.id,
            EditCommand::RotatePage {
                page_index: 1,
                degrees: 90,
            },
        )
        .unwrap();
    session
        .apply(
            opened.id,
            EditCommand::RotatePage {
                page_index: 2,
                degrees: 180,
            },
        )
        .unwrap();
    session.undo(opened.id).unwrap();

    let (bytes, report) = session
        .save_to_bytes(opened.id, SaveMode::Incremental)
        .unwrap();
    assert!(report.validation.ok, "{:?}", report.validation.errors);
    assert_eq!(report.validation.page_count, 4);
    // The original file survives byte for byte at the front of the result.
    assert_eq!(&bytes[..original.len()], original.as_slice());

    let mut second = Session::with_null_platform();
    let reopened = second
        .open_bytes(bytes, source("loop.pdf"), &OpenOptions::default())
        .unwrap();
    assert_eq!(reopened.pages[0].rotation, 0);
    assert_eq!(reopened.pages[1].rotation, 90);
    // The undone edit must not be in the file.
    assert_eq!(reopened.pages[2].rotation, 0);
    assert_eq!(reopened.pages[3].rotation, 0);
}

#[test]
fn text_extraction_survives_a_save() {
    let mut session = Session::with_null_platform();
    let opened = session
        .open_bytes(
            testutil::split_stream_pdf(2),
            source("streams.pdf"),
            &OpenOptions::default(),
        )
        .unwrap();

    let before = npdf_core::extract::extract_page_text(session.get(opened.id).unwrap(), 0).unwrap();
    assert_eq!(before.runs.len(), 2);

    session
        .apply(
            opened.id,
            EditCommand::RotatePage {
                page_index: 0,
                degrees: 90,
            },
        )
        .unwrap();
    let (bytes, _) = session
        .save_to_bytes(opened.id, SaveMode::Incremental)
        .unwrap();

    let mut second = Session::with_null_platform();
    let reopened = second
        .open_bytes(bytes, source("streams.pdf"), &OpenOptions::default())
        .unwrap();
    let after = npdf_core::extract::extract_page_text(second.get(reopened.id).unwrap(), 0).unwrap();

    assert_eq!(after.runs.len(), before.runs.len());
    assert_eq!(after.runs[0].text_lossy(), before.runs[0].text_lossy());
    assert_eq!(after.runs[1].origin, before.runs[1].origin);
}

/// The proof asked for in the specification: outside the edited area the two
/// renderings have to be identical. Rotating a page changes everything, so this
/// test uses a metadata change, which must change no pixel at all.
#[test]
fn an_edit_that_touches_no_content_changes_no_pixel() {
    let mut session = Session::with_null_platform();
    let opened = session
        .open_bytes(
            testutil::simple_text_pdf(1),
            source("pixels.pdf"),
            &OpenOptions::default(),
        )
        .unwrap();

    let before = match session.render_page(opened.id, &RenderRequest::new(0, 1.0)) {
        Ok(page) => page,
        Err(error) if error.code() == "renderer_unavailable" => {
            eprintln!("skipped: PDFium is not available here, {error}");
            return;
        }
        Err(error) => panic!("rendering failed: {error}"),
    };

    let mut fields = std::collections::BTreeMap::new();
    fields.insert("Title".to_string(), Some("Geaendert".to_string()));
    session
        .apply(opened.id, EditCommand::SetDocumentInfo { fields })
        .unwrap();

    let after = session
        .render_page(opened.id, &RenderRequest::new(0, 1.0))
        .expect("the page still renders after the edit");

    let report = diff::compare(&before, &after, None, 0).unwrap();
    assert!(
        report.ok && report.changed_pixels == 0,
        "a metadata change moved {} pixels, first at {:?}",
        report.changed_pixels,
        report.outside_bounds
    );
    assert!(before.width > 0 && before.height > 0);
}
