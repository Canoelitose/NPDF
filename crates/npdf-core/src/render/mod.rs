//! Turning pages into pixels.
//!
//! The renderer sits behind a trait for two reasons. PDFium is a C++ library and
//! has to be supplied per target, so it may be missing on a platform we have not
//! finished packaging yet. And the pure Rust fallback for mobile, should PDFium
//! turn out to be impractical there, has to slot in without touching anything
//! else.

mod budget;
mod cache;
pub mod diff;

#[cfg(feature = "pdfium")]
mod pdfium;

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub use budget::MemoryBudget;
pub use cache::{CacheKey, RenderCache};

/// What to render and how large.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRequest {
    pub page_index: usize,
    /// Device pixels per PDF point. 1.0 renders at 72 dpi.
    pub scale: f64,
    /// Extra rotation on top of the page's own `/Rotate`, in quarter turns.
    #[serde(default)]
    pub extra_rotation: i32,
}

impl RenderRequest {
    pub fn new(page_index: usize, scale: f64) -> Self {
        Self {
            page_index,
            scale,
            extra_rotation: 0,
        }
    }
}

/// A rendered page. The pixels are RGBA, eight bits per channel, top row first,
/// which is what a canvas and an `ImageData` expect.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedPage {
    pub page_index: usize,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub rgba: Vec<u8>,
}

impl RenderedPage {
    pub fn byte_size(&self) -> usize {
        self.rgba.len()
    }

    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

/// Implemented by every rendering backend.
pub trait PageRenderer: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn page_count(&self) -> usize;
    fn render(&self, request: &RenderRequest) -> Result<RenderedPage>;
}

/// What the frontend is told about the renderer, so it can show an honest
/// message instead of an empty page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererInfo {
    pub backend: String,
    pub available: bool,
    /// German text for the user if the backend is missing.
    pub detail: String,
}

/// Check whether a rendering backend can be used right now.
pub fn probe() -> RendererInfo {
    #[cfg(feature = "pdfium")]
    {
        pdfium::probe()
    }
    #[cfg(not(feature = "pdfium"))]
    {
        RendererInfo {
            backend: "none".to_string(),
            available: false,
            detail: "Dieser Bau enthaelt keinen Renderer. Die Funktion pdfium fehlt.".to_string(),
        }
    }
}

/// Open a document for rendering. The bytes must be a complete PDF, which for an
/// edited document means the result of a save.
pub fn open(bytes: Vec<u8>) -> Result<Box<dyn PageRenderer>> {
    #[cfg(feature = "pdfium")]
    {
        Ok(Box::new(pdfium::PdfiumRenderer::open(bytes)?))
    }
    #[cfg(not(feature = "pdfium"))]
    {
        let _ = bytes;
        Err(crate::error::Error::RendererUnavailable(
            "this build has no rendering backend".to_string(),
        ))
    }
}

/// Clamp a scale so one page bitmap stays inside the budget. Returns the scale
/// that was actually used, which may be smaller than the one that was asked for.
pub fn clamp_scale(width_pt: f64, height_pt: f64, scale: f64, budget: &MemoryBudget) -> f64 {
    let scale = scale.clamp(0.05, 16.0);
    if width_pt <= 0.0 || height_pt <= 0.0 {
        return scale;
    }
    let pixels = width_pt * scale * height_pt * scale;
    if pixels <= budget.max_page_pixels as f64 {
        return scale;
    }
    // Shrink until the bitmap fits. Keeps a huge poster page from allocating a
    // gigabyte on a phone.
    (budget.max_page_pixels as f64 / (width_pt * height_pt)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scale_that_fits_is_left_alone() {
        let budget = MemoryBudget::desktop();
        let scale = clamp_scale(595.0, 842.0, 2.0, &budget);
        assert!((scale - 2.0).abs() < 1e-9);
    }

    #[test]
    fn an_oversized_page_is_scaled_down_to_the_budget() {
        let budget = MemoryBudget::mobile();
        // A very large page at a high zoom level.
        let scale = clamp_scale(2000.0, 3000.0, 8.0, &budget);
        let pixels = 2000.0 * scale * 3000.0 * scale;
        assert!(scale < 8.0);
        assert!(
            pixels <= budget.max_page_pixels as f64 + 1.0,
            "pixels {pixels}"
        );
    }

    #[test]
    fn probe_never_panics_and_reports_something() {
        let info = probe();
        assert!(!info.backend.is_empty());
        assert!(!info.detail.is_empty());
    }
}
