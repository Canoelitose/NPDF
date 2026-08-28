//! The command surface.
//!
//! Every command is a thin forwarder. Argument checking, PDF work and error
//! wording all live in the core, so the behaviour is identical no matter which
//! shell calls it. Errors cross the bridge as `{ code, message }`, so the
//! frontend can react to a case such as a missing password without matching on
//! English text.

mod dto;

use npdf_core::doc::{DocumentId, DocumentSource, DocumentSummary, HistoryEntry, OpenOptions};
use npdf_core::edit::EditCommand;
use npdf_core::extract::PageText;
use npdf_core::platform::DocumentHandle;
use npdf_core::render::RenderRequest;
use npdf_core::save::{SaveMode, SaveReport};
use npdf_core::CoreInfo;
use tauri::ipc::Response;
use tauri::{AppHandle, Runtime};

use crate::state::SessionAccess;

pub use dto::CommandError;

type CommandResult<T> = std::result::Result<T, CommandError>;

/// Version, platform, capabilities and whether the renderer is there. The
/// frontend calls this first and shows an honest message if something is
/// missing.
#[tauri::command]
pub fn core_info() -> CoreInfo {
    CoreInfo::gather()
}

/// Desktop path: the core reads the file itself.
#[tauri::command]
pub fn open_document_path<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    password: Option<String>,
) -> CommandResult<DocumentSummary> {
    let handle = DocumentHandle::Path(path.into());
    Ok(app.npdf().open_handle(handle, &OpenOptions { password })?)
}

/// Mobile path, and anything that arrives from a share sheet: the shell has
/// already read the bytes.
#[tauri::command]
pub fn open_document_bytes<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    bytes: Vec<u8>,
    handle: Option<DocumentHandle>,
    password: Option<String>,
) -> CommandResult<DocumentSummary> {
    let source = DocumentSource {
        display_name: name,
        handle,
    };
    Ok(app
        .npdf()
        .open_bytes(bytes, source, &OpenOptions { password })?)
}

#[tauri::command]
pub fn close_document<R: Runtime>(app: AppHandle<R>, id: DocumentId) -> CommandResult<()> {
    Ok(app.npdf().close(id)?)
}

#[tauri::command]
pub fn list_documents<R: Runtime>(app: AppHandle<R>) -> CommandResult<Vec<DocumentSummary>> {
    Ok(app.npdf().summaries()?)
}

#[tauri::command]
pub fn document_summary<R: Runtime>(
    app: AppHandle<R>,
    id: DocumentId,
) -> CommandResult<DocumentSummary> {
    Ok(app.npdf().get(id)?.summary()?)
}

/// The text runs of one page, used for the debug overlay in M2 and for the
/// editable text layer from M3 on.
#[tauri::command]
pub fn page_text<R: Runtime>(
    app: AppHandle<R>,
    id: DocumentId,
    page_index: usize,
) -> CommandResult<PageText> {
    let session = app.npdf();
    let document = session.get(id)?;
    Ok(npdf_core::extract::extract_page_text(document, page_index)?)
}

#[tauri::command]
pub fn apply_edit<R: Runtime>(
    app: AppHandle<R>,
    id: DocumentId,
    command: EditCommand,
) -> CommandResult<HistoryEntry> {
    Ok(app.npdf().apply(id, command)?)
}

#[tauri::command]
pub fn undo<R: Runtime>(app: AppHandle<R>, id: DocumentId) -> CommandResult<HistoryEntry> {
    Ok(app.npdf().undo(id)?)
}

#[tauri::command]
pub fn redo<R: Runtime>(app: AppHandle<R>, id: DocumentId) -> CommandResult<HistoryEntry> {
    Ok(app.npdf().redo(id)?)
}

/// Render one page.
///
/// The pixels come back as raw bytes rather than as JSON, because a single A4
/// page at 200 percent is about eight megabytes and a JSON array of numbers
/// would be far too slow. The layout of the reply is documented in
/// [`dto::encode_rendered_page`].
#[tauri::command]
pub fn render_page<R: Runtime>(
    app: AppHandle<R>,
    id: DocumentId,
    request: RenderRequest,
) -> CommandResult<Response> {
    let page = app.npdf().render_page(id, &request)?;
    Ok(Response::new(dto::encode_rendered_page(&page)))
}

/// Write the document back through the platform. Desktop only, because on mobile
/// there is nothing the core could write to.
#[tauri::command]
pub fn save_document<R: Runtime>(
    app: AppHandle<R>,
    id: DocumentId,
    target: Option<DocumentHandle>,
    mode: Option<SaveMode>,
) -> CommandResult<SaveReport> {
    Ok(app.npdf().save(id, target, mode.unwrap_or_default())?)
}

/// Serialise the document and hand the bytes to the shell, which writes them
/// through the platform document API. This is the mobile path.
#[tauri::command]
pub fn save_document_bytes<R: Runtime>(
    app: AppHandle<R>,
    id: DocumentId,
    mode: Option<SaveMode>,
) -> CommandResult<Response> {
    let (bytes, report) = app.npdf().save_to_bytes(id, mode.unwrap_or_default())?;
    Ok(Response::new(dto::encode_saved_document(&bytes, &report)?))
}

/// Called when the app goes into the background. On a phone this is the
/// difference between being suspended and being killed.
#[tauri::command]
pub fn release_memory<R: Runtime>(app: AppHandle<R>) {
    app.npdf().release_memory();
}

#[tauri::command]
pub fn restore_memory_budget<R: Runtime>(app: AppHandle<R>) {
    app.npdf().restore_memory_budget();
}
