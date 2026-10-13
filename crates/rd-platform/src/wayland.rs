/// Wayland detection and utilities for Linux
#[cfg(target_os = "linux")]
pub fn is_wayland() -> bool {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return true;
    }
    if let Ok(sess) = std::env::var("XDG_SESSION_TYPE") {
        if sess.eq_ignore_ascii_case("wayland") {
            return true;
        }
    }
    false
}

#[cfg(not(target_os = "linux"))]
pub fn is_wayland() -> bool {
    false
}
