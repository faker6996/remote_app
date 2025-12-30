#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
pub use self::windows::WindowsScreenCapture;

#[cfg(target_os = "linux")]
pub use self::linux::LinuxScreenCapture;

#[cfg(target_os = "macos")]
pub use self::macos::MacOSScreenCapture;

use rd_core::domain::{ports::ScreenCapture, error::CaptureError};
use std::sync::Arc;

/// Create a platform-specific screen capture implementation
pub fn create_screen_capture() -> Result<Arc<tokio::sync::Mutex<dyn ScreenCapture>>, CaptureError> {
    #[cfg(target_os = "windows")]
    {
        Ok(Arc::new(tokio::sync::Mutex::new(WindowsScreenCapture::new()?)))
    }
    
    #[cfg(target_os = "linux")]
    {
        Ok(Arc::new(tokio::sync::Mutex::new(LinuxScreenCapture::new()?)))
    }
    
    #[cfg(target_os = "macos")]
    {
        Ok(Arc::new(tokio::sync::Mutex::new(MacOSScreenCapture::new()?)))
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(CaptureError::UnsupportedPlatform)
    }
}
