//! Small synthetic PDFs.
//!
//! These are not a replacement for the real world sample files under
//! `tests/fixtures`, which are fetched separately. They exist so that the unit
//! tests run offline, in every CI job and on every target, with input we control
//! byte for byte.

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};

/// A4 in points.
pub const A4: (f64, f64) = (595.0, 842.0);

/// A document with `pages` pages, one line of Helvetica text on each.
///
/// `/MediaBox` and `/Resources` sit on the `/Pages` node only, so the tests also
/// cover attribute inheritance.
pub fn simple_text_pdf(pages: usize) -> Vec<u8> {
    build(pages, false)
}

/// The same document, but every page splits its content over two streams.
/// Producers do this all the time and the extractor has to cope with it.
pub fn split_stream_pdf(pages: usize) -> Vec<u8> {
    build(pages, true)
}

/// Bytes that look like a PDF at the start but are not one.
pub fn truncated_pdf() -> Vec<u8> {
    let mut bytes = simple_text_pdf(1);
    bytes.truncate(bytes.len() / 3);
    bytes
}

fn build(pages: usize, split_streams: bool) -> Vec<u8> {
    let pages = pages.max(1);
    let mut doc = Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });

    let pages_id = doc.new_object_id();
    let mut kids: Vec<Object> = Vec::with_capacity(pages);

    for index in 0..pages {
        let head = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new(
                    "Tj",
                    vec![Object::string_literal(format!("Seite {}", index + 1))],
                ),
                Operation::new("ET", vec![]),
            ],
        };
        let tail = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 11.into()]),
                Operation::new("Td", vec![72.into(), 690.into()]),
                Operation::new("Tj", vec![Object::string_literal("Testdokument fuer NPDF")]),
                Operation::new("ET", vec![]),
            ],
        };

        let contents: Object = if split_streams {
            let a = doc.add_object(Stream::new(dictionary! {}, head.encode().unwrap()));
            let b = doc.add_object(Stream::new(dictionary! {}, tail.encode().unwrap()));
            Object::Array(vec![a.into(), b.into()])
        } else {
            let mut operations = head.operations;
            operations.extend(tail.operations);
            let merged = Content { operations };
            doc.add_object(Stream::new(dictionary! {}, merged.encode().unwrap()))
                .into()
        };

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => contents,
        });
        kids.push(page_id.into());
    }

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => pages as i64,
            "Kids" => Object::Array(kids),
            "Resources" => resources_id,
            "MediaBox" => Object::Array(vec![
                0.into(),
                0.into(),
                Object::Real(A4.0 as f32),
                Object::Real(A4.1 as f32),
            ]),
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buffer = Vec::new();
    doc.save_to(&mut buffer)
        .expect("synthetic document is writable");
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builder_produces_a_document_lopdf_can_read_back() {
        let bytes = simple_text_pdf(4);
        let doc = Document::load_mem(&bytes).unwrap();
        assert_eq!(doc.get_pages().len(), 4);
        assert!(bytes.starts_with(b"%PDF-1.5"));
    }

    #[test]
    fn split_streams_produce_two_content_objects_per_page() {
        let bytes = split_stream_pdf(1);
        let doc = Document::load_mem(&bytes).unwrap();
        let page_id = *doc.get_pages().values().next().unwrap();
        assert_eq!(doc.get_page_contents(page_id).len(), 2);
    }
}
