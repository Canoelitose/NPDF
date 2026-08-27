//! A least recently used cache for page bitmaps.
//!
//! Keyed by document, page and zoom step, because the same page at two zoom
//! levels is two different bitmaps. Bounded by bytes rather than by entry count,
//! since one poster page can be worth fifty text pages.

use std::collections::HashMap;
use std::sync::Arc;

use super::{MemoryBudget, RenderedPage};
use crate::doc::DocumentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub document: DocumentId,
    pub page_index: usize,
    /// Zoom quantised to a thousandth, so floating point noise does not produce
    /// a cache miss on every mouse wheel tick.
    pub scale_milli: u32,
    pub extra_rotation: i32,
}

impl CacheKey {
    pub fn new(document: DocumentId, page_index: usize, scale: f64, extra_rotation: i32) -> Self {
        Self {
            document,
            page_index,
            scale_milli: (scale.max(0.0) * 1000.0).round() as u32,
            extra_rotation: extra_rotation.rem_euclid(4),
        }
    }
}

struct Entry {
    page: Arc<RenderedPage>,
    /// Logical time of the last access, used to pick the victim.
    last_used: u64,
    bytes: usize,
}

pub struct RenderCache {
    entries: HashMap<CacheKey, Entry>,
    budget: MemoryBudget,
    used_bytes: usize,
    clock: u64,
    hits: u64,
    misses: u64,
}

impl RenderCache {
    pub fn new(budget: MemoryBudget) -> Self {
        Self {
            entries: HashMap::new(),
            budget,
            used_bytes: 0,
            clock: 0,
            hits: 0,
            misses: 0,
        }
    }

    pub fn budget(&self) -> MemoryBudget {
        self.budget
    }

    /// Replace the budget, for example when the app moves into the background.
    /// Evicts immediately if the new budget is smaller.
    pub fn set_budget(&mut self, budget: MemoryBudget) {
        self.budget = budget;
        self.evict_until_within_budget();
    }

    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn hits(&self) -> u64 {
        self.hits
    }

    pub fn misses(&self) -> u64 {
        self.misses
    }

    pub fn get(&mut self, key: &CacheKey) -> Option<Arc<RenderedPage>> {
        self.clock += 1;
        let clock = self.clock;
        match self.entries.get_mut(key) {
            Some(entry) => {
                entry.last_used = clock;
                self.hits += 1;
                Some(Arc::clone(&entry.page))
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    pub fn insert(&mut self, key: CacheKey, page: RenderedPage) -> Arc<RenderedPage> {
        let bytes = page.byte_size();
        let page = Arc::new(page);
        self.clock += 1;

        if let Some(old) = self.entries.remove(&key) {
            self.used_bytes -= old.bytes;
        }

        // A single bitmap larger than the whole budget is handed back but not
        // kept, otherwise it would evict everything else and then itself.
        if bytes > self.budget.max_cache_bytes {
            return page;
        }

        self.entries.insert(
            key,
            Entry {
                page: Arc::clone(&page),
                last_used: self.clock,
                bytes,
            },
        );
        self.used_bytes += bytes;
        self.evict_until_within_budget();
        page
    }

    /// Drop everything belonging to one document, for example when it is closed
    /// or after an edit changed its appearance.
    pub fn invalidate_document(&mut self, document: DocumentId) {
        self.entries.retain(|key, _| key.document != document);
        // Recomputed rather than subtracted, so the counter cannot drift.
        self.used_bytes = self.entries.values().map(|e| e.bytes).sum();
    }

    /// Drop every cached bitmap of one page at every zoom level.
    pub fn invalidate_page(&mut self, document: DocumentId, page_index: usize) {
        self.entries
            .retain(|key, _| !(key.document == document && key.page_index == page_index));
        self.used_bytes = self.entries.values().map(|e| e.bytes).sum();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.used_bytes = 0;
    }

    fn evict_until_within_budget(&mut self) {
        while self.used_bytes > self.budget.max_cache_bytes && !self.entries.is_empty() {
            let victim = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| *key);
            let Some(victim) = victim else { break };
            if let Some(entry) = self.entries.remove(&victim) {
                self.used_bytes -= entry.bytes;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(page_index: usize, bytes: usize) -> RenderedPage {
        RenderedPage {
            page_index,
            width: 1,
            height: (bytes / 4).max(1) as u32,
            scale: 1.0,
            rgba: vec![0u8; bytes],
        }
    }

    fn small_budget(bytes: usize) -> MemoryBudget {
        MemoryBudget {
            max_cache_bytes: bytes,
            max_page_pixels: 1_000_000,
            prerender_radius: 0,
        }
    }

    #[test]
    fn a_stored_page_comes_back() {
        let mut cache = RenderCache::new(small_budget(1000));
        let key = CacheKey::new(DocumentId(1), 0, 1.0, 0);
        cache.insert(key, page(0, 100));
        let got = cache.get(&key).expect("the page is cached");
        assert_eq!(got.page_index, 0);
        assert_eq!(cache.hits(), 1);
        assert_eq!(cache.used_bytes(), 100);
    }

    #[test]
    fn the_same_page_at_two_zoom_levels_is_two_entries() {
        let mut cache = RenderCache::new(small_budget(1000));
        cache.insert(CacheKey::new(DocumentId(1), 0, 1.0, 0), page(0, 100));
        cache.insert(CacheKey::new(DocumentId(1), 0, 2.0, 0), page(0, 400));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn a_zoom_that_differs_only_by_noise_hits_the_same_entry() {
        let mut cache = RenderCache::new(small_budget(1000));
        cache.insert(CacheKey::new(DocumentId(1), 0, 1.5, 0), page(0, 100));
        assert!(cache
            .get(&CacheKey::new(DocumentId(1), 0, 1.50004, 0))
            .is_some());
    }

    #[test]
    fn the_least_recently_used_entry_is_dropped_first() {
        let mut cache = RenderCache::new(small_budget(250));
        let a = CacheKey::new(DocumentId(1), 0, 1.0, 0);
        let b = CacheKey::new(DocumentId(1), 1, 1.0, 0);
        let c = CacheKey::new(DocumentId(1), 2, 1.0, 0);
        cache.insert(a, page(0, 100));
        cache.insert(b, page(1, 100));
        // Touch a so that b becomes the oldest.
        assert!(cache.get(&a).is_some());
        cache.insert(c, page(2, 100));

        assert!(
            cache.get(&a).is_some(),
            "a was used recently and must survive"
        );
        assert!(cache.get(&c).is_some(), "c was just inserted");
        assert!(cache.get(&b).is_none(), "b was the least recently used");
        assert!(cache.used_bytes() <= 250);
    }

    #[test]
    fn an_oversized_bitmap_is_returned_but_not_kept() {
        let mut cache = RenderCache::new(small_budget(100));
        let key = CacheKey::new(DocumentId(1), 0, 1.0, 0);
        let stored = cache.insert(key, page(0, 4000));
        assert_eq!(stored.byte_size(), 4000);
        assert!(cache.is_empty());
        assert_eq!(cache.used_bytes(), 0);
    }

    #[test]
    fn invalidation_removes_the_right_entries_and_fixes_the_byte_count() {
        let mut cache = RenderCache::new(small_budget(10_000));
        cache.insert(CacheKey::new(DocumentId(1), 0, 1.0, 0), page(0, 100));
        cache.insert(CacheKey::new(DocumentId(1), 1, 1.0, 0), page(1, 100));
        cache.insert(CacheKey::new(DocumentId(2), 0, 1.0, 0), page(0, 100));

        cache.invalidate_page(DocumentId(1), 0);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.used_bytes(), 200);

        cache.invalidate_document(DocumentId(1));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.used_bytes(), 100);
    }

    #[test]
    fn a_smaller_budget_evicts_right_away() {
        let mut cache = RenderCache::new(small_budget(10_000));
        for index in 0..10 {
            cache.insert(
                CacheKey::new(DocumentId(1), index, 1.0, 0),
                page(index, 500),
            );
        }
        assert_eq!(cache.used_bytes(), 5000);
        cache.set_budget(small_budget(1200));
        assert!(cache.used_bytes() <= 1200, "used {}", cache.used_bytes());
    }
}
