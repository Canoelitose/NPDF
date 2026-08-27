//! Editing commands.
//!
//! A command never writes into the original document. It stages the objects it
//! needs into the update layer, records what they looked like before, and then
//! changes them. Undo restores the recorded state, redo runs the command again.
//!
//! M0 ships two commands that exercise the whole path end to end. The text
//! commands follow in M3, the image and annotation commands in M5.

use std::collections::BTreeMap;

use lopdf::{Dictionary, Object, ObjectId, StringFormat};
use serde::{Deserialize, Serialize};

use crate::doc::{AppliedEdit, Document, HistoryEntry};
use crate::error::{Error, Result};

/// Everything the core knows how to change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum EditCommand {
    /// Set the absolute rotation of one page. `degrees` is normalised to a
    /// quarter turn.
    RotatePage { page_index: usize, degrees: i32 },
    /// Change entries of the document information dictionary. A `None` value
    /// removes the entry.
    SetDocumentInfo {
        #[serde(default)]
        fields: BTreeMap<String, Option<String>>,
    },
}

impl EditCommand {
    /// German label for the history list in the sidebar.
    pub fn label(&self) -> String {
        match self {
            EditCommand::RotatePage {
                page_index,
                degrees,
            } => {
                format!("Seite {} auf {} Grad gedreht", page_index + 1, degrees)
            }
            EditCommand::SetDocumentInfo { .. } => "Metadaten geändert".to_string(),
        }
    }

    pub fn page_index(&self) -> Option<usize> {
        match self {
            EditCommand::RotatePage { page_index, .. } => Some(*page_index),
            EditCommand::SetDocumentInfo { .. } => None,
        }
    }

    /// Whether the page tree has to be walked again afterwards.
    fn changes_page_tree(&self) -> bool {
        false
    }

    /// Objects this command is going to write to. Recorded before it runs so
    /// undo has something to restore.
    fn touched_objects(&self, doc: &mut Document) -> Result<Vec<ObjectId>> {
        match self {
            EditCommand::RotatePage { page_index, .. } => Ok(vec![doc.page_id(*page_index)?]),
            EditCommand::SetDocumentInfo { .. } => Ok(vec![info_dictionary_id(doc)?]),
        }
    }

    fn execute(&self, doc: &mut Document) -> Result<()> {
        match self {
            EditCommand::RotatePage {
                page_index,
                degrees,
            } => {
                let normalised = crate::doc::normalise_rotation(*degrees as i64);
                let page_id = doc.page_id(*page_index)?;
                doc.staged_dictionary_mut(page_id)?
                    .set("Rotate", Object::Integer(normalised as i64));
                Ok(())
            }
            EditCommand::SetDocumentInfo { fields } => {
                if fields.is_empty() {
                    return Err(Error::InvalidArgument("no metadata fields given".into()));
                }
                let info_id = info_dictionary_id(doc)?;
                let dict = doc.staged_dictionary_mut(info_id)?;
                for (key, value) in fields {
                    match value {
                        // Text strings are stored as literal strings. UTF-16BE
                        // with a byte order mark would also be legal, plain
                        // bytes keep simple ASCII values readable in a diff.
                        Some(value) => dict.set(
                            key.as_bytes().to_vec(),
                            Object::String(value.as_bytes().to_vec(), StringFormat::Literal),
                        ),
                        None => {
                            dict.remove(key.as_bytes());
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

/// The `/Info` dictionary, created if the document does not have one yet.
fn info_dictionary_id(doc: &mut Document) -> Result<ObjectId> {
    let existing = doc
        .incremental()
        .new_document
        .trailer
        .get(b"Info")
        .and_then(Object::as_reference)
        .ok();
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = doc.new_object_id();
    doc.set_object(id, Object::Dictionary(Dictionary::new()));
    doc.incremental_mut()
        .new_document
        .trailer
        .set("Info", Object::Reference(id));
    Ok(id)
}

/// Run a command and record it in the history.
pub fn apply(doc: &mut Document, command: EditCommand) -> Result<HistoryEntry> {
    let applied = run(doc, command)?;
    let entry = HistoryEntry {
        label: applied.label.clone(),
        page_index: applied.page_index,
    };
    let mut history = std::mem::take(doc.history_mut());
    history.push(applied);
    *doc.history_mut() = history;
    Ok(entry)
}

/// Run a command without touching the history. Used by [`apply`] and by redo.
fn run(doc: &mut Document, command: EditCommand) -> Result<AppliedEdit> {
    let touched = command.touched_objects(doc)?;
    let snapshots: Vec<(ObjectId, Option<Object>)> = touched
        .iter()
        .map(|id| (*id, doc.staged_snapshot(*id)))
        .collect();

    if let Err(error) = command.execute(doc) {
        // A command that fails half way must not leave the update layer in a
        // state nobody recorded.
        for (id, snapshot) in snapshots.into_iter().rev() {
            doc.restore_snapshot(id, snapshot);
        }
        return Err(error);
    }

    if command.changes_page_tree() {
        doc.refresh_pages()?;
    }
    doc.bump_revision();

    Ok(AppliedEdit {
        label: command.label(),
        page_index: command.page_index(),
        pages_changed: command.changes_page_tree(),
        command,
        snapshots,
    })
}

pub fn undo(doc: &mut Document) -> Result<HistoryEntry> {
    let mut history = std::mem::take(doc.history_mut());
    let Some(edit) = history.pop_undo() else {
        *doc.history_mut() = history;
        return Err(Error::NothingToUndo);
    };

    for (id, snapshot) in edit.snapshots.iter().rev() {
        doc.restore_snapshot(*id, snapshot.clone());
    }
    if edit.pages_changed {
        doc.refresh_pages()?;
    }
    doc.bump_revision();

    let entry = HistoryEntry {
        label: edit.label.clone(),
        page_index: edit.page_index,
    };
    history.push_redo(edit);
    *doc.history_mut() = history;
    Ok(entry)
}

pub fn redo(doc: &mut Document) -> Result<HistoryEntry> {
    let mut history = std::mem::take(doc.history_mut());
    let Some(edit) = history.pop_redo() else {
        *doc.history_mut() = history;
        return Err(Error::NothingToRedo);
    };
    // Put the history back before running, so a failing command leaves a sane
    // stack behind.
    *doc.history_mut() = history;

    let command = edit.command.clone();
    let applied = match run(doc, command) {
        Ok(applied) => applied,
        Err(error) => {
            let mut history = std::mem::take(doc.history_mut());
            history.push_redo(edit);
            *doc.history_mut() = history;
            return Err(error);
        }
    };

    let entry = HistoryEntry {
        label: applied.label.clone(),
        page_index: applied.page_index,
    };
    let mut history = std::mem::take(doc.history_mut());
    history.push_undo_keeping_redo(applied);
    *doc.history_mut() = history;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{DocumentId, DocumentSource, OpenOptions};
    use crate::testutil;

    fn open(pages: usize) -> Document {
        Document::open(
            DocumentId(7),
            testutil::simple_text_pdf(pages),
            DocumentSource {
                handle: None,
                display_name: "test.pdf".into(),
            },
            &OpenOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn the_wire_format_is_what_the_frontend_sends() {
        let command = EditCommand::RotatePage {
            page_index: 2,
            degrees: 90,
        };
        let json = serde_json::to_string(&command).unwrap();
        assert_eq!(json, r#"{"type":"rotatePage","pageIndex":2,"degrees":90}"#);
        let parsed: EditCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, command);
    }

    #[test]
    fn rotate_then_undo_then_redo() {
        let mut doc = open(2);
        assert_eq!(doc.page_info(1).unwrap().rotation, 0);
        assert!(!doc.is_dirty());

        let entry = apply(
            &mut doc,
            EditCommand::RotatePage {
                page_index: 1,
                degrees: 270,
            },
        )
        .unwrap();
        assert_eq!(entry.label, "Seite 2 auf 270 Grad gedreht");
        assert_eq!(doc.page_info(1).unwrap().rotation, 270);
        assert!(doc.is_dirty());
        assert!(doc.history().can_undo());

        undo(&mut doc).unwrap();
        assert_eq!(doc.page_info(1).unwrap().rotation, 0);
        // Undo removed the only staged object, so the document is clean again.
        assert!(!doc.is_dirty());
        assert!(doc.history().can_redo());

        redo(&mut doc).unwrap();
        assert_eq!(doc.page_info(1).unwrap().rotation, 270);
        assert!(doc.history().can_undo());
        assert!(!doc.history().can_redo());
    }

    #[test]
    fn rotation_is_normalised_before_it_is_written() {
        let mut doc = open(1);
        apply(
            &mut doc,
            EditCommand::RotatePage {
                page_index: 0,
                degrees: -90,
            },
        )
        .unwrap();
        assert_eq!(doc.page_info(0).unwrap().rotation, 270);
    }

    #[test]
    fn a_new_command_drops_the_redo_branch() {
        let mut doc = open(1);
        apply(
            &mut doc,
            EditCommand::RotatePage {
                page_index: 0,
                degrees: 90,
            },
        )
        .unwrap();
        undo(&mut doc).unwrap();
        assert!(doc.history().can_redo());
        apply(
            &mut doc,
            EditCommand::RotatePage {
                page_index: 0,
                degrees: 180,
            },
        )
        .unwrap();
        assert!(!doc.history().can_redo());
        assert_eq!(doc.page_info(0).unwrap().rotation, 180);
    }

    #[test]
    fn undo_on_an_empty_history_reports_it() {
        let mut doc = open(1);
        assert_eq!(undo(&mut doc).unwrap_err().code(), "nothing_to_undo");
        assert_eq!(redo(&mut doc).unwrap_err().code(), "nothing_to_redo");
    }

    #[test]
    fn metadata_can_be_set_and_removed() {
        let mut doc = open(1);
        let mut fields = BTreeMap::new();
        fields.insert("Title".to_string(), Some("Mein Dokument".to_string()));
        fields.insert("Author".to_string(), Some("NPDF".to_string()));
        apply(&mut doc, EditCommand::SetDocumentInfo { fields }).unwrap();

        let info_id = doc
            .incremental()
            .new_document
            .trailer
            .get(b"Info")
            .and_then(Object::as_reference)
            .unwrap();
        let title = doc.get_dictionary(info_id).unwrap().get(b"Title").unwrap();
        assert_eq!(title.as_str().unwrap(), b"Mein Dokument");

        let mut removal = BTreeMap::new();
        removal.insert("Author".to_string(), None);
        apply(&mut doc, EditCommand::SetDocumentInfo { fields: removal }).unwrap();
        assert!(doc.get_dictionary(info_id).unwrap().get(b"Author").is_err());

        undo(&mut doc).unwrap();
        assert_eq!(
            doc.get_dictionary(info_id)
                .unwrap()
                .get(b"Author")
                .unwrap()
                .as_str()
                .unwrap(),
            b"NPDF"
        );
    }
}
