//! The document model.
//!
//! A [`Document`] keeps the bytes of the file exactly as they were read plus a
//! second, initially empty structure that holds only the objects an edit has
//! touched. Saving appends that second structure as an incremental update, so
//! every object nobody touched is written back byte for byte. Bookmarks,
//! metadata, form fields and anything else we do not understand survive
//! untouched by construction rather than by care.

mod history;
mod page;

use std::collections::HashSet;

use lopdf::{Dictionary, IncrementalDocument, Object, ObjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::platform::DocumentHandle;

pub use history::{AppliedEdit, History, HistoryEntry, HistoryView};
pub use page::{normalise_rotation, ObjectRef, PageInfo};

/// Identifies an open document for the whole session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocumentId(pub u64);

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// How a document was opened. Never a bare path, because on mobile there is none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSource {
    pub handle: Option<DocumentHandle>,
    pub display_name: String,
}

/// The snapshot the frontend gets after opening. Deliberately small, page
/// content is fetched per page and on demand.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    pub id: DocumentId,
    pub source: DocumentSource,
    pub pdf_version: String,
    pub page_count: usize,
    pub pages: Vec<PageInfo>,
    pub dirty: bool,
    pub revision: u64,
    /// True when the file was encrypted and we decrypted it with a password.
    pub was_encrypted: bool,
    pub byte_size: usize,
    /// Digest of the bytes as they were read, used to detect outside changes.
    pub original_sha256: String,
    pub history: HistoryView,
}

/// Options for [`Document::open`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOptions {
    pub password: Option<String>,
}

pub struct Document {
    id: DocumentId,
    source: DocumentSource,
    incremental: IncrementalDocument,
    page_ids: Vec<ObjectId>,
    history: History,
    was_encrypted: bool,
    byte_size: usize,
    original_sha256: String,
    /// Bumped by every command, undo and redo. The render cache and the
    /// renderer instance key off this, so a stale bitmap cannot survive an edit.
    revision: u64,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("id", &self.id)
            .field("name", &self.source.display_name)
            .field("pages", &self.page_ids.len())
            .field("dirty", &self.is_dirty())
            .finish()
    }
}

impl Document {
    /// Parse a PDF that is already in memory.
    pub fn open(
        id: DocumentId,
        bytes: Vec<u8>,
        source: DocumentSource,
        options: &OpenOptions,
    ) -> Result<Self> {
        if bytes.len() < 8 {
            return Err(Error::BrokenDocument(
                "the file is too small to be a PDF".into(),
            ));
        }
        // A PDF header may be preceded by junk, so search a small prefix rather
        // than insisting on byte zero.
        let header_window = &bytes[..bytes.len().min(1024)];
        if !header_window.windows(5).any(|w| w == b"%PDF-") {
            return Err(Error::BrokenDocument("no %PDF- header found".into()));
        }

        let parsed = match options.password.as_deref() {
            Some(password) => lopdf::Document::load_mem_with_options(
                &bytes,
                lopdf::LoadOptions::with_password(password),
            )?,
            None => lopdf::Document::load_mem(&bytes)?,
        };

        let was_encrypted = parsed.encryption_state.is_some();
        if parsed.is_encrypted() {
            // Still encrypted means no usable password was supplied.
            return Err(Error::PasswordRequired);
        }
        // Touch the catalog early so a structurally broken file fails here and
        // not somewhere deep inside a later command.
        parsed.catalog().map_err(|_| {
            Error::BrokenDocument("the document catalog is missing or unreadable".into())
        })?;

        let digest = format!("{:x}", Sha256::digest(&bytes));
        let byte_size = bytes.len();
        let incremental = IncrementalDocument::create_from(bytes, parsed);

        let mut doc = Self {
            id,
            source,
            incremental,
            page_ids: Vec::new(),
            history: History::new(),
            was_encrypted,
            byte_size,
            original_sha256: digest,
            revision: 0,
        };
        doc.page_ids = doc.collect_page_ids()?;
        Ok(doc)
    }

    pub fn id(&self) -> DocumentId {
        self.id
    }

    pub fn source(&self) -> &DocumentSource {
        &self.source
    }

    pub fn set_source(&mut self, source: DocumentSource) {
        self.source = source;
    }

    pub fn page_count(&self) -> usize {
        self.page_ids.len()
    }

    pub fn was_encrypted(&self) -> bool {
        self.was_encrypted
    }

    pub fn original_bytes(&self) -> &[u8] {
        self.incremental.get_prev_documents_bytes()
    }

    pub fn original_sha256(&self) -> &str {
        &self.original_sha256
    }

    pub fn pdf_version(&self) -> String {
        self.incremental.get_prev_documents().version.clone()
    }

    /// True as soon as one command has written into the update layer.
    pub fn is_dirty(&self) -> bool {
        !self.incremental.new_document.objects.is_empty()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn history_mut(&mut self) -> &mut History {
        &mut self.history
    }

    pub fn summary(&self) -> Result<DocumentSummary> {
        Ok(DocumentSummary {
            id: self.id,
            source: self.source.clone(),
            pdf_version: self.pdf_version(),
            page_count: self.page_count(),
            pages: self.pages()?,
            dirty: self.is_dirty(),
            revision: self.revision,
            was_encrypted: self.was_encrypted,
            byte_size: self.byte_size,
            original_sha256: self.original_sha256.clone(),
            history: self.history.view(),
        })
    }

    // ---------------------------------------------------------------- objects

    /// Look an object up in the update layer first, then in the original file.
    pub fn get_object(&self, id: ObjectId) -> Result<&Object> {
        if self.incremental.new_document.has_object(id) {
            return Ok(self.incremental.new_document.get_object(id)?);
        }
        Ok(self.incremental.get_prev_documents().get_object(id)?)
    }

    pub fn get_dictionary(&self, id: ObjectId) -> Result<&Dictionary> {
        Ok(self.get_object(id)?.as_dict()?)
    }

    /// Follow references until a direct object is reached.
    pub fn resolve<'a>(&'a self, object: &'a Object) -> Result<&'a Object> {
        let mut current = object;
        for _ in 0..64 {
            match current {
                Object::Reference(id) => current = self.get_object(*id)?,
                other => return Ok(other),
            }
        }
        Err(Error::BrokenDocument(
            "reference chain is longer than 64 steps, the file is probably looping".into(),
        ))
    }

    pub fn catalog_id(&self) -> Result<ObjectId> {
        self.incremental
            .get_prev_documents()
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(|_| Error::BrokenDocument("the trailer has no /Root reference".into()))
    }

    /// Walk the page tree. Works even after a page reorder, because the rewritten
    /// `/Kids` array lives in the update layer and [`Document::get_object`] sees it.
    fn collect_page_ids(&self) -> Result<Vec<ObjectId>> {
        let catalog_id = self.catalog_id()?;
        let pages_id = self
            .get_dictionary(catalog_id)?
            .get(b"Pages")
            .and_then(Object::as_reference)
            .map_err(|_| Error::BrokenDocument("the catalog has no /Pages reference".into()))?;

        let mut out = Vec::new();
        let mut seen = HashSet::new();
        self.walk_page_tree(pages_id, &mut out, &mut seen, 0)?;
        if out.is_empty() {
            return Err(Error::BrokenDocument("the document has no pages".into()));
        }
        Ok(out)
    }

    fn walk_page_tree(
        &self,
        node_id: ObjectId,
        out: &mut Vec<ObjectId>,
        seen: &mut HashSet<ObjectId>,
        depth: usize,
    ) -> Result<()> {
        if depth > 64 || !seen.insert(node_id) {
            // A malformed file may point back at a node it already used. Stop
            // instead of recursing forever.
            return Ok(());
        }
        let dict = match self.get_dictionary(node_id) {
            Ok(dict) => dict,
            // A single unreadable node must not lose the rest of the document.
            Err(_) => return Ok(()),
        };

        let kids = dict.get(b"Kids").ok().and_then(|k| self.resolve(k).ok());
        match kids {
            Some(Object::Array(kids)) => {
                let kids = kids.clone();
                for kid in kids {
                    if let Ok(kid_id) = kid.as_reference() {
                        self.walk_page_tree(kid_id, out, seen, depth + 1)?;
                    }
                }
            }
            // No /Kids means this is a leaf. Some producers omit /Type on pages,
            // so the absence of kids is the more reliable signal.
            _ => out.push(node_id),
        }
        Ok(())
    }

    pub fn page_id(&self, index: usize) -> Result<ObjectId> {
        self.page_ids.get(index).copied().ok_or(Error::UnknownPage {
            index,
            count: self.page_ids.len(),
        })
    }

    pub fn page_ids(&self) -> &[ObjectId] {
        &self.page_ids
    }

    /// Re-read the page tree. Call after any command that changes page order or
    /// page count.
    pub fn refresh_pages(&mut self) -> Result<()> {
        self.page_ids = self.collect_page_ids()?;
        Ok(())
    }

    /// Look a key up on the page and, if it is missing, on its ancestors.
    /// `/Resources`, `/MediaBox`, `/CropBox` and `/Rotate` are inheritable.
    pub fn inherited(&self, page_id: ObjectId, key: &[u8]) -> Option<&Object> {
        let mut current = page_id;
        for _ in 0..64 {
            let dict = self.get_dictionary(current).ok()?;
            if let Ok(value) = dict.get(key) {
                if !matches!(value, Object::Null) {
                    return self.resolve(value).ok();
                }
            }
            current = dict.get(b"Parent").ok()?.as_reference().ok()?;
        }
        None
    }

    pub fn page_info(&self, index: usize) -> Result<PageInfo> {
        let id = self.page_id(index)?;
        Ok(PageInfo::read(self, index, id))
    }

    pub fn pages(&self) -> Result<Vec<PageInfo>> {
        (0..self.page_ids.len())
            .map(|i| self.page_info(i))
            .collect()
    }

    // ---------------------------------------------------------------- editing

    /// Copy an object into the update layer so it can be changed. The original
    /// stays where it is.
    pub fn stage(&mut self, id: ObjectId) -> Result<()> {
        self.incremental.opt_clone_object_to_new_document(id)?;
        Ok(())
    }

    /// The current state of an object inside the update layer, if it has been
    /// staged. Used to record undo snapshots.
    pub fn staged_snapshot(&self, id: ObjectId) -> Option<Object> {
        self.incremental.new_document.objects.get(&id).cloned()
    }

    /// Put an object back the way a snapshot found it. `None` removes it from the
    /// update layer, which restores the original object.
    pub fn restore_snapshot(&mut self, id: ObjectId, object: Option<Object>) {
        match object {
            Some(object) => {
                self.incremental.new_document.objects.insert(id, object);
            }
            None => {
                self.incremental.new_document.objects.remove(&id);
            }
        }
    }

    pub fn staged_dictionary_mut(&mut self, id: ObjectId) -> Result<&mut Dictionary> {
        self.stage(id)?;
        Ok(self
            .incremental
            .new_document
            .get_object_mut(id)?
            .as_dict_mut()?)
    }

    /// Reserve a fresh object id in the update layer.
    pub fn new_object_id(&mut self) -> ObjectId {
        self.incremental.new_document.new_object_id()
    }

    pub fn set_object(&mut self, id: ObjectId, object: Object) {
        self.incremental.new_document.objects.insert(id, object);
    }

    pub(crate) fn incremental_mut(&mut self) -> &mut IncrementalDocument {
        &mut self.incremental
    }

    pub(crate) fn incremental(&self) -> &IncrementalDocument {
        &self.incremental
    }

    /// A flattened copy of the document, original objects plus the update layer.
    /// Needed for a full rewrite and for handing bytes to the renderer.
    pub fn flattened(&self) -> lopdf::Document {
        let mut flat = self.incremental.get_prev_documents().clone();
        for (id, object) in &self.incremental.new_document.objects {
            flat.objects.insert(*id, object.clone());
            if id.0 > flat.max_id {
                flat.max_id = id.0;
            }
        }
        flat
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        .expect("document opens")
    }

    #[test]
    fn opens_a_synthetic_document_and_finds_its_pages() {
        let doc = open(testutil::simple_text_pdf(3));
        assert_eq!(doc.page_count(), 3);
        assert!(!doc.is_dirty());
        assert_eq!(doc.pdf_version(), "1.5");
        let page = doc.page_info(0).unwrap();
        assert_eq!(page.rotation, 0);
        assert!(
            (page.width_pt - 595.0).abs() < 0.5,
            "width was {}",
            page.width_pt
        );
        assert!(
            (page.height_pt - 842.0).abs() < 0.5,
            "height was {}",
            page.height_pt
        );
    }

    #[test]
    fn rejects_data_that_is_not_a_pdf() {
        let err = Document::open(
            DocumentId(1),
            b"this is a text file, not a PDF at all".to_vec(),
            DocumentSource {
                handle: None,
                display_name: "x.txt".into(),
            },
            &OpenOptions::default(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "broken_document");
    }

    #[test]
    fn staging_an_object_leaves_the_original_bytes_alone() {
        let bytes = testutil::simple_text_pdf(1);
        let mut doc = open(bytes.clone());
        let page_id = doc.page_id(0).unwrap();
        doc.staged_dictionary_mut(page_id)
            .unwrap()
            .set("Rotate", 90);
        assert!(doc.is_dirty());
        assert_eq!(doc.original_bytes(), bytes.as_slice());
        assert_eq!(doc.page_info(0).unwrap().rotation, 90);
    }

    #[test]
    fn inherited_attributes_come_from_the_pages_node() {
        // The synthetic builder puts /MediaBox on the /Pages node only.
        let doc = open(testutil::simple_text_pdf(2));
        let page_id = doc.page_id(1).unwrap();
        let media = doc.inherited(page_id, b"MediaBox");
        assert!(media.is_some(), "MediaBox must be inherited from /Pages");
    }
}
