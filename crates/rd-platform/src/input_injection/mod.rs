#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub mod wayland;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
pub use self::windows::WindowsInputInjector;

#[cfg(target_os = "linux")]
pub use self::linux::LinuxInputInjector;
#[cfg(target_os = "linux")]
pub use self::wayland::WaylandInputInjector;

#[cfg(target_os = "macos")]
pub use self::macos::MacOSInputInjector;

use rd_core::domain::{ports::InputInjector, error::InjectionError};
use std::sync::Arc;

/// Create a platform-specific input injector implementation
pub fn create_input_injector() -> Result<Arc<tokio::sync::Mutex<dyn InputInjector>>, InjectionError> {
    #[cfg(target_os = "windows")]
    {
        Ok(Arc::new(tokio::sync::Mutex::new(WindowsInputInjector::new()?)))
    }
    
    #[cfg(target_os = "linux")]
    {
        if crate::wayland::is_wayland() {
            tracing::info!("Detected Wayland session: using WaylandInputInjector (XDG Desktop Portal)");
            match WaylandInputInjector::new() {
                Ok(inj) => return Ok(Arc::new(tokio::sync::Mutex::new(inj))),
                Err(e) => {
                    tracing::warn!("Failed to initialize WaylandInputInjector ({}), falling back to X11/XTest", e);
                }
            }
        }
        Ok(Arc::new(tokio::sync::Mutex::new(LinuxInputInjector::new()?)))
    }
    
    #[cfg(target_os = "macos")]
    {
        Ok(Arc::new(tokio::sync::Mutex::new(MacOSInputInjector::new()?)))
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(InjectionError::UnsupportedPlatform)
    }
}
