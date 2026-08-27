//! The platform layer.
//!
//! Everything that needs the operating system lives behind
//! `npdf_core::platform::PlatformServices` and has exactly one implementation per
//! platform family. Nothing outside this directory may use `cfg(target_os)`.

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod desktop;
#[cfg(any(target_os = "android", target_os = "ios"))]
mod mobile;

use npdf_core::platform::PlatformServices;
use tauri::{AppHandle, Runtime};

/// Build the services for the platform this build targets.
pub fn services<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Box<dyn PlatformServices>, Box<dyn std::error::Error>> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Ok(Box::new(desktop::DesktopPlatform::new(app)?))
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        Ok(Box::new(mobile::MobilePlatform::new(app)?))
    }
}
