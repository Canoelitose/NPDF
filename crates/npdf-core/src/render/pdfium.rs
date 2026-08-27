//! The PDFium backend.
//!
//! PDFium is BSD-3-Clause licensed and is the renderer Chrome uses, so its
//! output is what most people consider to be the correct look of a PDF.
//!
//! Binding strategy per target:
//!
//! * Windows, macOS, Linux: the shared library ships next to the executable and
//!   is loaded at run time. `NPDF_PDFIUM_PATH` overrides the search.
//! * Android: `libpdfium.so` ships inside the APK under `jniLibs` and is found
//!   through the normal library search path.
//! * iOS: dynamic libraries are not an option, so the static library is linked
//!   into the binary. That happens automatically for the iOS target, the build
//!   only needs PDFIUM_STATIC_LIB_PATH to point at the directory with
//!   libpdfium.a.
//!
//! `Pdfium::new` may only be called once per process, so the instance lives in a
//! `OnceLock` and everything borrows from it.

use std::sync::OnceLock;

use pdfium_render::prelude::{
    PdfDocument, PdfPageRenderRotation, PdfRenderConfig, Pdfium, PdfiumError,
};

use super::{PageRenderer, RenderRequest, RenderedPage, RendererInfo};
use crate::error::{Error, Result};

static PDFIUM: OnceLock<std::result::Result<Pdfium, String>> = OnceLock::new();

/// Bind to PDFium once and keep the instance for the lifetime of the process.
fn instance() -> std::result::Result<&'static Pdfium, String> {
    PDFIUM.get_or_init(bind).as_ref().map_err(|e| e.clone())
}

#[cfg(target_os = "ios")]
fn bind() -> std::result::Result<Pdfium, String> {
    Pdfium::bind_to_statically_linked_library()
        .map(Pdfium::new)
        .map_err(|e| describe(&e))
}

#[cfg(not(target_os = "ios"))]
fn bind() -> std::result::Result<Pdfium, String> {
    let mut attempts: Vec<String> = Vec::new();

    for directory in search_directories() {
        let candidate = Pdfium::pdfium_platform_library_name_at_path(&directory);
        match Pdfium::bind_to_library(&candidate) {
            Ok(bindings) => return Ok(Pdfium::new(bindings)),
            Err(error) => attempts.push(format!("{}: {}", candidate.display(), describe(&error))),
        }
    }

    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(error) => {
            attempts.push(format!("Systempfad: {}", describe(&error)));
            Err(attempts.join(" | "))
        }
    }
}

/// Where to look for the shared library, most specific first.
#[cfg(not(target_os = "ios"))]
fn search_directories() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(explicit) = std::env::var("NPDF_PDFIUM_PATH") {
        if !explicit.is_empty() {
            dirs.push(PathBuf::from(explicit));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            // macOS puts frameworks beside the executable inside the bundle.
            dirs.push(dir.join("../Frameworks"));
        }
    }
    // The location the fetch script uses during development.
    dirs.push(PathBuf::from("vendor/pdfium/lib"));
    dirs.retain(|dir| dir.exists());
    dirs
}

fn describe(error: &PdfiumError) -> String {
    error.to_string()
}

/// Whether the renderer can be used, with a German explanation if it cannot.
pub fn probe() -> RendererInfo {
    match instance() {
        Ok(_) => RendererInfo {
            backend: "pdfium".to_string(),
            available: true,
            detail: "PDFium ist geladen.".to_string(),
        },
        Err(reason) => RendererInfo {
            backend: "pdfium".to_string(),
            available: false,
            detail: format!(
                "PDFium konnte nicht geladen werden. Bitte die Bibliothek bereitstellen, \
                 zum Beispiel mit npm run fetch:pdfium. Versuche: {reason}"
            ),
        },
    }
}

pub struct PdfiumRenderer {
    // The document borrows the process wide PDFium instance, which lives as long
    // as the program does, so the borrow is 'static.
    document: PdfDocument<'static>,
    page_count: usize,
}

// PDFium is not reentrant. The `thread_safe` feature of pdfium-render puts a
// mutex around every call, which makes sharing the renderer between Tauri
// command threads sound.
unsafe impl Send for PdfiumRenderer {}
unsafe impl Sync for PdfiumRenderer {}

impl PdfiumRenderer {
    /// Take ownership of the bytes and open them. The renderer always works on a
    /// complete saved document, never on the edit model.
    pub fn open(bytes: Vec<u8>) -> Result<Self> {
        let pdfium = instance().map_err(Error::RendererUnavailable)?;
        let document = pdfium
            .load_pdf_from_byte_vec(bytes, None)
            .map_err(|e| Error::Render(describe(&e)))?;
        let page_count = document.pages().len() as usize;
        Ok(Self {
            document,
            page_count,
        })
    }
}

impl PageRenderer for PdfiumRenderer {
    fn backend_name(&self) -> &'static str {
        "pdfium"
    }

    fn page_count(&self) -> usize {
        self.page_count
    }

    fn render(&self, request: &RenderRequest) -> Result<RenderedPage> {
        if request.page_index >= self.page_count {
            return Err(Error::UnknownPage {
                index: request.page_index,
                count: self.page_count,
            });
        }
        let page = self
            .document
            .pages()
            .get(request.page_index as i32)
            .map_err(|e| Error::Render(describe(&e)))?;

        let mut config = PdfRenderConfig::new().scale_page_by_factor(request.scale as f32);
        if let Some(rotation) = quarter_turns(request.extra_rotation) {
            config = config.rotate(rotation, true);
        }

        let bitmap = page
            .render_with_config(&config)
            .map_err(|e| Error::Render(describe(&e)))?;

        Ok(RenderedPage {
            page_index: request.page_index,
            width: bitmap.width() as u32,
            height: bitmap.height() as u32,
            scale: request.scale,
            rgba: bitmap.as_rgba_bytes(),
        })
    }
}

fn quarter_turns(value: i32) -> Option<PdfPageRenderRotation> {
    match value.rem_euclid(4) {
        1 => Some(PdfPageRenderRotation::Degrees90),
        2 => Some(PdfPageRenderRotation::Degrees180),
        3 => Some(PdfPageRenderRotation::Degrees270),
        _ => None,
    }
}
