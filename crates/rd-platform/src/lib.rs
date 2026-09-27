pub mod screen_capture;
pub mod input_injection;
pub mod clipboard;
pub mod audio;

pub use screen_capture::create_screen_capture;
pub use input_injection::create_input_injector;
pub use clipboard::ClipboardManager;
pub use audio::AudioCaptureService;

// Re-export core traits
pub use rd_core::domain::ports::{ScreenCapture, InputInjector};

pub fn check_system_permissions() -> (bool, bool) {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGPreflightScreenCaptureAccess() -> bool;
        }
        let screen = unsafe { CGPreflightScreenCaptureAccess() };
        let a11y = unsafe { AXIsProcessTrusted() };
        (screen, a11y)
    }
    #[cfg(not(target_os = "macos"))]
    {
        (true, true)
    }
}

pub fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}
