//! Finding the PDFium library at startup.
//!
//! The core looks for the library in a few well known places and honours the
//! `NPDF_PDFIUM_PATH` environment variable. The shell is the only part that
//! knows where a bundled resource ends up on each platform, so it sets that
//! variable before the first render can happen.

use std::path::{Path, PathBuf};

use tauri::{App, Manager, Runtime};

/// File name of the shared library on the current platform.
fn library_file_names() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["pdfium.dll"]
    }
    #[cfg(target_os = "macos")]
    {
        &["libpdfium.dylib"]
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        &["libpdfium.so"]
    }
    #[cfg(target_os = "ios")]
    {
        // Statically linked, nothing to look for.
        &[]
    }
}

fn contains_library(dir: &Path) -> bool {
    library_file_names()
        .iter()
        .any(|name| dir.join(name).exists())
}

pub fn configure<R: Runtime>(app: &App<R>) {
    if std::env::var_os("NPDF_PDFIUM_PATH").is_some() {
        // An explicit setting always wins, that is what it is for.
        return;
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(resources) = app.path().resource_dir() {
        candidates.push(resources.join("pdfium"));
        candidates.push(resources);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.to_path_buf());
            // Inside a macOS bundle the libraries sit in Contents/Frameworks.
            candidates.push(dir.join("../Frameworks"));
        }
    }
    // Where the development fetch script puts it.
    candidates.push(PathBuf::from("vendor/pdfium/lib"));

    if let Some(found) = candidates.into_iter().find(|dir| contains_library(dir)) {
        std::env::set_var("NPDF_PDFIUM_PATH", found);
    }
}
