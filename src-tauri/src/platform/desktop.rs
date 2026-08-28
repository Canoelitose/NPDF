//! Windows, macOS and Linux.
//!
//! Desktop platforms hand out real file paths, so the core can read and write
//! files itself.

use std::path::PathBuf;

use npdf_core::platform::{
    default_font_dirs, DocumentHandle, PlatformCapabilities, PlatformKind, PlatformServices,
};
use npdf_core::{Error, Result};
use tauri::{AppHandle, Manager, Runtime};

pub struct DesktopPlatform {
    autosave_dir: PathBuf,
}

impl DesktopPlatform {
    pub fn new<R: Runtime>(app: &AppHandle<R>) -> Result<Self> {
        let autosave_dir = app
            .path()
            .app_local_data_dir()
            .map(|dir| dir.join("autosave"))
            .map_err(|e| Error::InvalidArgument(format!("no data directory: {e}")))?;
        Ok(Self { autosave_dir })
    }
}

impl PlatformServices for DesktopPlatform {
    fn kind(&self) -> PlatformKind {
        PlatformKind::current()
    }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities::for_kind(self.kind())
    }

    fn autosave_dir(&self) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.autosave_dir)
            .map_err(|e| Error::io(self.autosave_dir.clone(), e))?;
        Ok(self.autosave_dir.clone())
    }

    fn system_font_dirs(&self) -> Vec<PathBuf> {
        default_font_dirs()
    }

    fn read_document(&self, handle: &DocumentHandle) -> Result<Vec<u8>> {
        match handle {
            DocumentHandle::Path(path) => {
                std::fs::read(path).map_err(|e| Error::io(path.clone(), e))
            }
            _ => Err(Error::InvalidArgument(
                "on the desktop a document is addressed by its path".into(),
            )),
        }
    }

    fn write_document(&self, handle: &DocumentHandle, bytes: &[u8]) -> Result<()> {
        match handle {
            DocumentHandle::Path(path) => {
                // Write beside the target and rename, so a crash half way through
                // cannot destroy the file the user already had.
                let temporary = path.with_extension("npdf-part");
                std::fs::write(&temporary, bytes).map_err(|e| Error::io(temporary.clone(), e))?;
                std::fs::rename(&temporary, path).map_err(|e| {
                    let _ = std::fs::remove_file(&temporary);
                    Error::io(path.clone(), e)
                })
            }
            _ => Err(Error::InvalidArgument(
                "on the desktop a document is addressed by its path".into(),
            )),
        }
    }
}
