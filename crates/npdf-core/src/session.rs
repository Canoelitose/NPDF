//! The open documents of one running app.
//!
//! The shell owns exactly one [`Session`] behind a lock and forwards every
//! command to it. Keeping this here rather than in the Tauri layer means the
//! whole application logic can be tested without a window.

use std::collections::HashMap;
use std::sync::Arc;

use crate::doc::{
    Document, DocumentId, DocumentSource, DocumentSummary, HistoryEntry, OpenOptions,
};
use crate::edit::{self, EditCommand};
use crate::error::{Error, Result};
use crate::platform::{DocumentHandle, NullPlatform, PlatformServices};
use crate::render::{
    self, CacheKey, MemoryBudget, PageRenderer, RenderCache, RenderRequest, RenderedPage,
};
use crate::save::{self, SaveMode, SaveReport};

struct RendererSlot {
    revision: u64,
    renderer: Box<dyn PageRenderer>,
}

pub struct Session {
    documents: HashMap<DocumentId, Document>,
    order: Vec<DocumentId>,
    renderers: HashMap<DocumentId, RendererSlot>,
    cache: RenderCache,
    platform: Arc<dyn PlatformServices>,
    next_id: u64,
}

impl Session {
    pub fn new(platform: Arc<dyn PlatformServices>) -> Self {
        let budget = MemoryBudget::for_platform(platform.kind());
        Self {
            documents: HashMap::new(),
            order: Vec::new(),
            renderers: HashMap::new(),
            cache: RenderCache::new(budget),
            platform,
            next_id: 1,
        }
    }

    pub fn with_null_platform() -> Self {
        Self::new(Arc::new(NullPlatform))
    }

    pub fn platform(&self) -> &Arc<dyn PlatformServices> {
        &self.platform
    }

    pub fn cache(&self) -> &RenderCache {
        &self.cache
    }

    /// Called when the operating system asks for memory back, which on iOS and
    /// Android happens whenever the app goes into the background.
    pub fn release_memory(&mut self) {
        self.cache
            .set_budget(MemoryBudget::for_platform(self.platform.kind()).under_pressure());
        self.renderers.clear();
    }

    /// Called when the app comes back to the foreground.
    pub fn restore_memory_budget(&mut self) {
        self.cache
            .set_budget(MemoryBudget::for_platform(self.platform.kind()));
    }

    // ----------------------------------------------------------------- opening

    pub fn open_bytes(
        &mut self,
        bytes: Vec<u8>,
        source: DocumentSource,
        options: &OpenOptions,
    ) -> Result<DocumentSummary> {
        let id = DocumentId(self.next_id);
        let document = Document::open(id, bytes, source, options)?;
        // Only count the id up once the document really opened, so a wrong
        // password does not burn a number on every attempt.
        self.next_id += 1;
        let summary = document.summary()?;
        self.documents.insert(id, document);
        self.order.push(id);
        Ok(summary)
    }

    /// Open through the platform, which is the only path that works on mobile.
    pub fn open_handle(
        &mut self,
        handle: DocumentHandle,
        options: &OpenOptions,
    ) -> Result<DocumentSummary> {
        let bytes = self.platform.read_document(&handle)?;
        let source = DocumentSource {
            display_name: handle.display_name(),
            handle: Some(handle),
        };
        self.open_bytes(bytes, source, options)
    }

    pub fn close(&mut self, id: DocumentId) -> Result<()> {
        if self.documents.remove(&id).is_none() {
            return Err(Error::UnknownDocument(id.0));
        }
        self.order.retain(|other| *other != id);
        self.renderers.remove(&id);
        self.cache.invalidate_document(id);
        Ok(())
    }

    pub fn get(&self, id: DocumentId) -> Result<&Document> {
        self.documents.get(&id).ok_or(Error::UnknownDocument(id.0))
    }

    pub fn get_mut(&mut self, id: DocumentId) -> Result<&mut Document> {
        self.documents
            .get_mut(&id)
            .ok_or(Error::UnknownDocument(id.0))
    }

    /// Open documents in the order they were opened, which is the order of the
    /// cards in the sidebar.
    pub fn summaries(&self) -> Result<Vec<DocumentSummary>> {
        self.order
            .iter()
            .filter_map(|id| self.documents.get(id))
            .map(|doc| doc.summary())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    pub fn len(&self) -> usize {
        self.documents.len()
    }

    // ----------------------------------------------------------------- editing

    pub fn apply(&mut self, id: DocumentId, command: EditCommand) -> Result<HistoryEntry> {
        let page_index = command.page_index();
        let document = self.get_mut(id)?;
        let entry = edit::apply(document, command)?;
        self.invalidate(id, page_index);
        Ok(entry)
    }

    pub fn undo(&mut self, id: DocumentId) -> Result<HistoryEntry> {
        let document = self.get_mut(id)?;
        let entry = edit::undo(document)?;
        self.invalidate(id, entry.page_index);
        Ok(entry)
    }

    pub fn redo(&mut self, id: DocumentId) -> Result<HistoryEntry> {
        let document = self.get_mut(id)?;
        let entry = edit::redo(document)?;
        self.invalidate(id, entry.page_index);
        Ok(entry)
    }

    fn invalidate(&mut self, id: DocumentId, page_index: Option<usize>) {
        match page_index {
            Some(page_index) => self.cache.invalidate_page(id, page_index),
            None => self.cache.invalidate_document(id),
        }
        // The renderer holds the bytes from before the edit.
        self.renderers.remove(&id);
    }

    // ------------------------------------------------------------------ saving

    /// Serialise a document without writing it anywhere.
    pub fn save_to_bytes(
        &mut self,
        id: DocumentId,
        mode: SaveMode,
    ) -> Result<(Vec<u8>, SaveReport)> {
        let document = self.get_mut(id)?;
        save::save_to_bytes(document, mode)
    }

    /// Write a document back to where it came from, or to a new place.
    pub fn save(
        &mut self,
        id: DocumentId,
        target: Option<DocumentHandle>,
        mode: SaveMode,
    ) -> Result<SaveReport> {
        let handle = match target {
            Some(handle) => handle,
            None => self.get(id)?.source().handle.clone().ok_or_else(|| {
                Error::InvalidArgument(
                    "this document has no place to be saved to, ask the user first".into(),
                )
            })?,
        };

        let (bytes, report) = self.save_to_bytes(id, mode)?;
        self.platform.write_document(&handle, &bytes)?;

        let display_name = handle.display_name();
        let document = self.get_mut(id)?;
        document.set_source(DocumentSource {
            handle: Some(handle),
            display_name,
        });
        Ok(report)
    }

    // --------------------------------------------------------------- rendering

    /// Render a page, using the cache where possible.
    pub fn render_page(
        &mut self,
        id: DocumentId,
        request: &RenderRequest,
    ) -> Result<Arc<RenderedPage>> {
        let document = self.get(id)?;
        let page = document.page_info(request.page_index)?;
        let budget = self.cache.budget();
        let scale = render::clamp_scale(page.width_pt, page.height_pt, request.scale, &budget);
        let request = RenderRequest { scale, ..*request };

        let key = CacheKey::new(id, request.page_index, scale, request.extra_rotation);
        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached);
        }

        self.ensure_renderer(id)?;
        let slot = self
            .renderers
            .get(&id)
            .ok_or_else(|| Error::RendererUnavailable("no renderer for this document".into()))?;
        let rendered = slot.renderer.render(&request)?;
        Ok(self.cache.insert(key, rendered))
    }

    /// Build a renderer for the current state of the document, if the one we
    /// have is missing or stale.
    fn ensure_renderer(&mut self, id: DocumentId) -> Result<()> {
        let revision = self.get(id)?.revision();
        if self
            .renderers
            .get(&id)
            .is_some_and(|slot| slot.revision == revision)
        {
            return Ok(());
        }

        // An untouched document can be handed to the renderer as it was read.
        // A changed one has to be serialised first, which is cheap because the
        // incremental writer copies the original bytes and appends.
        let bytes = if self.get(id)?.is_dirty() {
            self.save_to_bytes(id, SaveMode::Incremental)?.0
        } else {
            self.get(id)?.original_bytes().to_vec()
        };

        let renderer = render::open(bytes)?;
        self.renderers
            .insert(id, RendererSlot { revision, renderer });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil;

    fn source(name: &str) -> DocumentSource {
        DocumentSource {
            handle: None,
            display_name: name.to_string(),
        }
    }

    #[test]
    fn documents_open_close_and_keep_their_order() {
        let mut session = Session::with_null_platform();
        let a = session
            .open_bytes(
                testutil::simple_text_pdf(1),
                source("a.pdf"),
                &OpenOptions::default(),
            )
            .unwrap();
        let b = session
            .open_bytes(
                testutil::simple_text_pdf(2),
                source("b.pdf"),
                &OpenOptions::default(),
            )
            .unwrap();
        assert_eq!(session.len(), 2);
        assert_ne!(a.id, b.id);

        let names: Vec<String> = session
            .summaries()
            .unwrap()
            .into_iter()
            .map(|s| s.source.display_name)
            .collect();
        assert_eq!(names, vec!["a.pdf".to_string(), "b.pdf".to_string()]);

        session.close(a.id).unwrap();
        assert_eq!(session.len(), 1);
        assert_eq!(session.close(a.id).unwrap_err().code(), "unknown_document");
    }

    #[test]
    fn a_failed_open_does_not_consume_a_document_id() {
        let mut session = Session::with_null_platform();
        assert!(session
            .open_bytes(b"garbage".to_vec(), source("x"), &OpenOptions::default())
            .is_err());
        let good = session
            .open_bytes(
                testutil::simple_text_pdf(1),
                source("a.pdf"),
                &OpenOptions::default(),
            )
            .unwrap();
        assert_eq!(good.id, DocumentId(1));
    }

    #[test]
    fn editing_through_the_session_updates_the_summary() {
        let mut session = Session::with_null_platform();
        let opened = session
            .open_bytes(
                testutil::simple_text_pdf(2),
                source("a.pdf"),
                &OpenOptions::default(),
            )
            .unwrap();
        assert!(!opened.dirty);
        assert_eq!(opened.revision, 0);

        session
            .apply(
                opened.id,
                EditCommand::RotatePage {
                    page_index: 1,
                    degrees: 90,
                },
            )
            .unwrap();

        let after = session.get(opened.id).unwrap().summary().unwrap();
        assert!(after.dirty);
        assert_eq!(after.revision, 1);
        assert_eq!(after.pages[1].rotation, 90);
        assert!(after.history.can_undo);

        session.undo(opened.id).unwrap();
        let undone = session.get(opened.id).unwrap().summary().unwrap();
        assert_eq!(undone.pages[1].rotation, 0);
        assert_eq!(undone.revision, 2);
    }

    #[test]
    fn saving_writes_through_the_platform_and_round_trips() {
        let dir = std::env::temp_dir().join("npdf-session-save-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("round-trip.pdf");
        std::fs::write(&path, testutil::simple_text_pdf(2)).unwrap();

        let mut session = Session::with_null_platform();
        let opened = session
            .open_handle(DocumentHandle::Path(path.clone()), &OpenOptions::default())
            .unwrap();
        assert_eq!(opened.source.display_name, "round-trip.pdf");

        session
            .apply(
                opened.id,
                EditCommand::RotatePage {
                    page_index: 0,
                    degrees: 180,
                },
            )
            .unwrap();
        let report = session
            .save(opened.id, None, SaveMode::Incremental)
            .unwrap();
        assert!(report.validation.ok, "{:?}", report.validation.errors);

        let mut second = Session::with_null_platform();
        let reopened = second
            .open_handle(DocumentHandle::Path(path.clone()), &OpenOptions::default())
            .unwrap();
        assert_eq!(reopened.pages[0].rotation, 180);
        assert_eq!(reopened.page_count, 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn saving_without_a_target_is_a_clear_error() {
        let mut session = Session::with_null_platform();
        let opened = session
            .open_bytes(
                testutil::simple_text_pdf(1),
                source("a.pdf"),
                &OpenOptions::default(),
            )
            .unwrap();
        let error = session
            .save(opened.id, None, SaveMode::Incremental)
            .unwrap_err();
        assert_eq!(error.code(), "invalid_argument");
    }

    #[test]
    fn releasing_memory_shrinks_the_cache_budget() {
        let mut session = Session::with_null_platform();
        let full = session.cache().budget().max_cache_bytes;
        session.release_memory();
        assert!(session.cache().budget().max_cache_bytes < full);
        session.restore_memory_budget();
        assert_eq!(session.cache().budget().max_cache_bytes, full);
    }

    #[test]
    fn rendering_reports_a_missing_backend_instead_of_panicking() {
        let mut session = Session::with_null_platform();
        let opened = session
            .open_bytes(
                testutil::simple_text_pdf(1),
                source("a.pdf"),
                &OpenOptions::default(),
            )
            .unwrap();
        match session.render_page(opened.id, &RenderRequest::new(0, 1.0)) {
            Ok(page) => {
                // PDFium is present in this environment.
                assert_eq!(page.page_index, 0);
                assert!(page.width > 0 && page.height > 0);
            }
            Err(error) => assert_eq!(
                error.code(),
                "renderer_unavailable",
                "unexpected render error: {error}"
            ),
        }
    }
}
