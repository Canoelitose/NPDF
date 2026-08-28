//! iOS and Android.
//!
//! Neither platform gives an app a free file system path. A document arrives as
//! a handle from the system picker, from a share sheet or from another app, and
//! only the platform layer can turn that handle into bytes.
//!
//! Reading and writing therefore runs through the Tauri file system plugin on
//! the frontend side, which knows how to resolve a `content://` URI and how to
//! hold on to an iOS security scoped bookmark. The core receives the bytes and
//! hands finished bytes back, it never touches the file system on mobile.

use std::path::PathBuf;

use npdf_core::platform::{
    default_font_dirs, DocumentHandle, PlatformCapabilities, PlatformKind, PlatformServices,
};
use npdf_core::{Error, Result};
use tauri::{AppHandle, Manager, Runtime};

pub struct MobilePlatform {
    autosave_dir: PathBuf,
}

impl MobilePlatform {
    pub fn new<R: Runtime>(app: &AppHandle<R>) -> Result<Self> {
        let autosave_dir = app
            .path()
            .app_local_data_dir()
            .map(|dir| dir.join("autosave"))
            .map_err(|e| Error::InvalidArgument(format!("no data directory: {e}")))?;
        Ok(Self { autosave_dir })
    }
}

impl PlatformServices for MobilePlatform {
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
            // The sandbox does allow plain paths inside the app container, which
            // is where autosave snapshots live.
            DocumentHandle::Path(path) => {
                std::fs::read(path).map_err(|e| Error::io(path.clone(), e))
            }
            _ => Err(Error::NotImplemented(
                "on iOS and Android the shell reads the document and passes the bytes in",
            )),
        }
    }

    fn write_document(&self, handle: &DocumentHandle, bytes: &[u8]) -> Result<()> {
        match handle {
            DocumentHandle::Path(path) => {
                std::fs::write(path, bytes).map_err(|e| Error::io(path.clone(), e))
            }
            _ => Err(Error::NotImplemented(
                "on iOS and Android the shell writes the bytes the core hands back",
            )),
        }
    }
}
