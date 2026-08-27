//! NPDF core.
//!
//! Everything in this crate is plain Rust with no system dependency, so the same
//! code runs on Windows, macOS, Linux, iOS and Android and can be tested on any
//! of them. The only place platform differences are allowed is the
//! [`platform::PlatformServices`] trait, which the shell implements.
//!
//! The guiding rule for the whole crate: the bytes of the file the user opened
//! are never modified. Edits go into a separate update layer and saving appends
//! it, so anything we do not understand survives untouched.

pub mod doc;
pub mod edit;
pub mod error;
pub mod extract;
pub mod fonts;
pub mod geom;
pub mod platform;
pub mod render;
pub mod save;
pub mod session;
pub mod testutil;

pub use error::{Error, Result};
pub use session::Session;

use serde::{Deserialize, Serialize};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the shell reports on startup. The frontend uses it to decide which
/// features to show, so it is deliberately honest, it says when something is
/// missing instead of pretending.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreInfo {
    pub version: String,
    pub platform: platform::PlatformKind,
    pub capabilities: platform::PlatformCapabilities,
    pub renderer: render::RendererInfo,
    pub memory_budget: render::MemoryBudget,
    /// Cargo features this build was compiled with.
    pub features: Vec<String>,
}

impl CoreInfo {
    pub fn gather() -> Self {
        let platform = platform::PlatformKind::current();
        Self {
            version: VERSION.to_string(),
            platform,
            capabilities: platform::PlatformCapabilities::for_kind(platform),
            renderer: render::probe(),
            memory_budget: render::MemoryBudget::for_platform(platform),
            features: enabled_features(),
        }
    }
}

fn enabled_features() -> Vec<String> {
    let mut features = Vec::new();
    if cfg!(feature = "pdfium") {
        features.push("pdfium".to_string());
    }
    if cfg!(target_os = "ios") {
        // On iOS the library is part of the binary rather than a file beside it.
        features.push("pdfium-static".to_string());
    }
    features
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_info_can_always_be_gathered() {
        let info = CoreInfo::gather();
        assert_eq!(info.version, VERSION);
        assert!(info.memory_budget.max_cache_bytes > 0);
        // Serialising must work, the whole struct crosses the bridge to the UI.
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"renderer\""));
        assert!(json.contains("\"memoryBudget\""));
    }
}
