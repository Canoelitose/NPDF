//! How much memory the renderer may use.
//!
//! Desktop machines can afford a generous cache. A phone cannot, and it also has
//! to hand memory back when the app goes into the background. Both numbers live
//! here so there is one place to argue about them.

use serde::{Deserialize, Serialize};

use crate::platform::PlatformKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryBudget {
    /// Upper bound for all cached page bitmaps together.
    pub max_cache_bytes: usize,
    /// Upper bound for a single page bitmap, in pixels.
    pub max_page_pixels: usize,
    /// How many pages ahead and behind the visible one we render in advance.
    pub prerender_radius: usize,
}

impl MemoryBudget {
    pub const fn desktop() -> Self {
        Self {
            // 384 MB of bitmaps, about forty A4 pages at 200 percent zoom.
            max_cache_bytes: 384 * 1024 * 1024,
            // 64 megapixels, enough for an A0 poster at 150 dpi.
            max_page_pixels: 64_000_000,
            prerender_radius: 2,
        }
    }

    pub const fn mobile() -> Self {
        Self {
            max_cache_bytes: 72 * 1024 * 1024,
            // 12 megapixels, roughly an A4 page at 300 dpi.
            max_page_pixels: 12_000_000,
            prerender_radius: 1,
        }
    }

    pub fn for_platform(kind: PlatformKind) -> Self {
        if kind.is_mobile() {
            Self::mobile()
        } else {
            Self::desktop()
        }
    }

    pub fn current() -> Self {
        Self::for_platform(PlatformKind::current())
    }

    /// What the app keeps when the system asks for memory back. The visible page
    /// stays, everything else goes.
    pub fn under_pressure(self) -> Self {
        Self {
            max_cache_bytes: self.max_cache_bytes / 8,
            max_page_pixels: self.max_page_pixels,
            prerender_radius: 0,
        }
    }
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self::current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_is_stricter_than_desktop() {
        let mobile = MemoryBudget::mobile();
        let desktop = MemoryBudget::desktop();
        assert!(mobile.max_cache_bytes < desktop.max_cache_bytes);
        assert!(mobile.max_page_pixels < desktop.max_page_pixels);
        assert!(mobile.prerender_radius < desktop.prerender_radius);
    }

    #[test]
    fn pressure_shrinks_the_cache_and_stops_prerendering() {
        let budget = MemoryBudget::desktop().under_pressure();
        assert_eq!(budget.prerender_radius, 0);
        assert!(budget.max_cache_bytes < MemoryBudget::desktop().max_cache_bytes);
    }
}
