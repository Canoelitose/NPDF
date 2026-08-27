//! The single place where platform differences are allowed to exist.
//!
//! Everything that touches the document itself is plain Rust and behaves the
//! same on all five targets. Everything that needs the operating system goes
//! through [`PlatformServices`], which the shell implements once per platform.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlatformKind {
    Windows,
    MacOs,
    Linux,
    Ios,
    Android,
    /// Anything we do not ship for. Keeps the core compiling in test runners.
    Other,
}

impl PlatformKind {
    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        {
            PlatformKind::Windows
        }
        #[cfg(target_os = "macos")]
        {
            PlatformKind::MacOs
        }
        #[cfg(target_os = "linux")]
        {
            PlatformKind::Linux
        }
        #[cfg(target_os = "ios")]
        {
            PlatformKind::Ios
        }
        #[cfg(target_os = "android")]
        {
            PlatformKind::Android
        }
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux",
            target_os = "ios",
            target_os = "android"
        )))]
        {
            PlatformKind::Other
        }
    }

    pub fn is_mobile(self) -> bool {
        matches!(self, PlatformKind::Ios | PlatformKind::Android)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PlatformKind::Windows => "windows",
            PlatformKind::MacOs => "macos",
            PlatformKind::Linux => "linux",
            PlatformKind::Ios => "ios",
            PlatformKind::Android => "android",
            PlatformKind::Other => "other",
        }
    }
}

/// What the current platform lets us do. The frontend hides features instead of
/// offering something that cannot work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    /// Free file system paths. False on iOS and Android, where we only ever get
    /// a handle from the system document picker.
    pub free_file_paths: bool,
    pub share_sheet: bool,
    pub printing: bool,
    /// Whether we can enumerate installed fonts for the font fallback in M4.
    pub system_fonts: bool,
    /// OCR is optional and switched off where the dependency does not build.
    pub ocr: bool,
    /// Whether the app can be suspended by the system and has to save state.
    pub can_be_suspended: bool,
}

impl PlatformCapabilities {
    pub fn for_kind(kind: PlatformKind) -> Self {
        match kind {
            PlatformKind::Ios => Self {
                free_file_paths: false,
                share_sheet: true,
                printing: true,
                system_fonts: true,
                ocr: false,
                can_be_suspended: true,
            },
            PlatformKind::Android => Self {
                free_file_paths: false,
                share_sheet: true,
                printing: true,
                system_fonts: true,
                ocr: false,
                can_be_suspended: true,
            },
            _ => Self {
                free_file_paths: true,
                share_sheet: false,
                printing: true,
                system_fonts: true,
                ocr: true,
                can_be_suspended: false,
            },
        }
    }
}

/// How a document was handed to us. On desktop this is a path, on mobile an
/// opaque handle that only the shell knows how to open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind",
    content = "value"
)]
pub enum DocumentHandle {
    /// A plain file system path. Desktop only.
    Path(PathBuf),
    /// An Android `content://` URI obtained from the Storage Access Framework.
    ContentUri(String),
    /// A base64 encoded iOS security scoped bookmark.
    SecurityScopedBookmark(String),
    /// Bytes that were handed to us directly, for example from a share sheet.
    InMemory { name: String },
}

impl DocumentHandle {
    /// A short label for the document card in the sidebar.
    pub fn display_name(&self) -> String {
        match self {
            DocumentHandle::Path(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.to_string_lossy().to_string()),
            DocumentHandle::ContentUri(uri) => {
                uri.rsplit(['/', '%']).next().unwrap_or(uri).to_string()
            }
            DocumentHandle::SecurityScopedBookmark(_) => "Dokument".to_string(),
            DocumentHandle::InMemory { name } => name.clone(),
        }
    }
}

/// Implemented once per platform by the shell.
pub trait PlatformServices: Send + Sync {
    fn kind(&self) -> PlatformKind;

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities::for_kind(self.kind())
    }

    /// Where autosave snapshots go. Must survive an app restart.
    fn autosave_dir(&self) -> Result<PathBuf>;

    /// Directories that hold system and user installed fonts, used by the font
    /// fallback in M4.
    fn system_font_dirs(&self) -> Vec<PathBuf>;

    /// Read the bytes behind a handle. On desktop this is a file read, on mobile
    /// the shell resolves the handle through the platform document API first.
    fn read_document(&self, handle: &DocumentHandle) -> Result<Vec<u8>>;

    /// Write bytes back to the place the handle points at.
    fn write_document(&self, handle: &DocumentHandle, bytes: &[u8]) -> Result<()>;
}

/// A services implementation that fails every request. Used in unit tests and as
/// a safe default before the shell has installed the real one.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullPlatform;

impl PlatformServices for NullPlatform {
    fn kind(&self) -> PlatformKind {
        PlatformKind::current()
    }

    fn autosave_dir(&self) -> Result<PathBuf> {
        Ok(std::env::temp_dir().join("npdf-autosave"))
    }

    fn system_font_dirs(&self) -> Vec<PathBuf> {
        default_font_dirs()
    }

    fn read_document(&self, handle: &DocumentHandle) -> Result<Vec<u8>> {
        match handle {
            DocumentHandle::Path(path) => {
                std::fs::read(path).map_err(|e| crate::Error::io(path.clone(), e))
            }
            _ => Err(crate::Error::NotImplemented(
                "this handle kind needs the platform shell",
            )),
        }
    }

    fn write_document(&self, handle: &DocumentHandle, bytes: &[u8]) -> Result<()> {
        match handle {
            DocumentHandle::Path(path) => {
                std::fs::write(path, bytes).map_err(|e| crate::Error::io(path.clone(), e))
            }
            _ => Err(crate::Error::NotImplemented(
                "this handle kind needs the platform shell",
            )),
        }
    }
}

/// Conventional font locations per platform. The shell may add more.
pub fn default_font_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    match PlatformKind::current() {
        PlatformKind::Windows => {
            if let Ok(win) = std::env::var("WINDIR") {
                dirs.push(PathBuf::from(win).join("Fonts"));
            }
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                dirs.push(PathBuf::from(local).join("Microsoft/Windows/Fonts"));
            }
        }
        PlatformKind::MacOs => {
            dirs.push(PathBuf::from("/System/Library/Fonts"));
            dirs.push(PathBuf::from("/Library/Fonts"));
            if let Ok(home) = std::env::var("HOME") {
                dirs.push(PathBuf::from(home).join("Library/Fonts"));
            }
        }
        PlatformKind::Linux => {
            dirs.push(PathBuf::from("/usr/share/fonts"));
            dirs.push(PathBuf::from("/usr/local/share/fonts"));
            if let Ok(home) = std::env::var("HOME") {
                dirs.push(PathBuf::from(&home).join(".local/share/fonts"));
                dirs.push(PathBuf::from(&home).join(".fonts"));
            }
        }
        PlatformKind::Android => {
            dirs.push(PathBuf::from("/system/fonts"));
            dirs.push(PathBuf::from("/product/fonts"));
        }
        PlatformKind::Ios => {
            dirs.push(PathBuf::from("/System/Library/Fonts"));
        }
        PlatformKind::Other => {}
    }
    dirs.retain(|d| d.exists());
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_platforms_have_no_free_paths() {
        for kind in [PlatformKind::Ios, PlatformKind::Android] {
            assert!(kind.is_mobile());
            assert!(!PlatformCapabilities::for_kind(kind).free_file_paths);
        }
        for kind in [
            PlatformKind::Windows,
            PlatformKind::MacOs,
            PlatformKind::Linux,
        ] {
            assert!(!kind.is_mobile());
            assert!(PlatformCapabilities::for_kind(kind).free_file_paths);
        }
    }

    #[test]
    fn the_handle_wire_format_is_tagged() {
        let handle = DocumentHandle::Path(PathBuf::from("/tmp/a.pdf"));
        let json = serde_json::to_string(&handle).unwrap();
        assert_eq!(json, r#"{"kind":"path","value":"/tmp/a.pdf"}"#);
        let parsed: DocumentHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, handle);
    }

    #[test]
    fn handle_display_name_uses_the_file_name() {
        let handle = DocumentHandle::Path(PathBuf::from("/tmp/some folder/Rechnung.pdf"));
        assert_eq!(handle.display_name(), "Rechnung.pdf");
    }
}
