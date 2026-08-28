//! Undo and redo.
//!
//! Every command records the state of each object it touched inside the update
//! layer before it ran. Undo puts those objects back, redo runs the command
//! again. That mechanism does not need to know what a command does, so new
//! commands get undo for free as long as they stage every object they change.

use lopdf::{Object, ObjectId};
use serde::{Deserialize, Serialize};

use crate::edit::EditCommand;

/// How many steps we keep. Deep enough for a long session, bounded so a mobile
/// device does not run out of memory.
pub const DEFAULT_HISTORY_LIMIT: usize = 200;

/// One finished edit, with everything needed to take it back.
#[derive(Debug, Clone)]
pub struct AppliedEdit {
    pub label: String,
    pub page_index: Option<usize>,
    pub command: EditCommand,
    /// Object id and its state in the update layer before the command ran.
    /// `None` means the object was not in the update layer at all.
    pub(crate) snapshots: Vec<(ObjectId, Option<Object>)>,
    /// Whether the page tree has to be walked again after undo or redo.
    pub(crate) pages_changed: bool,
}

/// What the history list in the sidebar shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub label: String,
    pub page_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryView {
    pub can_undo: bool,
    pub can_redo: bool,
    /// Newest first, so the frontend can show the last steps without reversing.
    pub undo: Vec<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
}

#[derive(Debug, Clone)]
pub struct History {
    undo: Vec<AppliedEdit>,
    redo: Vec<AppliedEdit>,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_HISTORY_LIMIT)
    }

    pub fn with_limit(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn len(&self) -> usize {
        self.undo.len()
    }

    pub fn is_empty(&self) -> bool {
        self.undo.is_empty()
    }

    /// Record a finished edit. Doing something new drops the redo branch, which
    /// is what every editor does and what users expect.
    pub fn push(&mut self, edit: AppliedEdit) {
        self.redo.clear();
        self.undo.push(edit);
        while self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    pub(crate) fn pop_undo(&mut self) -> Option<AppliedEdit> {
        self.undo.pop()
    }

    pub(crate) fn push_redo(&mut self, edit: AppliedEdit) {
        self.redo.push(edit);
    }

    pub(crate) fn pop_redo(&mut self) -> Option<AppliedEdit> {
        self.redo.pop()
    }

    /// Put an edit back on the undo stack without clearing redo. Used while
    /// redoing, where the redo branch has to survive.
    pub(crate) fn push_undo_keeping_redo(&mut self, edit: AppliedEdit) {
        self.undo.push(edit);
    }

    pub fn view(&self) -> HistoryView {
        HistoryView {
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
            undo: self.undo.iter().rev().map(entry).collect(),
            redo: self.redo.iter().rev().map(entry).collect(),
        }
    }
}

fn entry(edit: &AppliedEdit) -> HistoryEntry {
    HistoryEntry {
        label: edit.label.clone(),
        page_index: edit.page_index,
    }
}
