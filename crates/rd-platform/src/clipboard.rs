use std::sync::Arc;
use tokio::sync::Mutex;

/// Cross-platform clipboard manager supporting macOS, Windows, and Linux
pub struct ClipboardManager {
    last_text: Arc<Mutex<String>>,
}

impl ClipboardManager {
    pub fn new() -> Self {
        Self {
            last_text: Arc::new(Mutex::new(String::new())),
        }
    }

    /// Read current text from OS clipboard
    pub fn read_text() -> Result<String, String> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| format!("Failed to access clipboard: {}", e))?;
        clipboard.get_text().map_err(|e| e.to_string())
    }

    /// Write text to OS clipboard
    pub fn write_text(text: &str) -> Result<(), String> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| format!("Failed to access clipboard: {}", e))?;
        clipboard.set_text(text.to_string()).map_err(|e| e.to_string())
    }

    /// Update the last known synchronized text so local listener doesn't bounce it back
    pub async fn set_synced(&self, text: &str) {
        let mut last = self.last_text.lock().await;
        *last = text.to_string();
    }

    /// Check if current OS clipboard text is new (different from last synced text)
    pub async fn get_new_text_if_changed(&self) -> Option<String> {
        if let Ok(current) = Self::read_text() {
            let mut last = self.last_text.lock().await;
            if !current.is_empty() && *last != current {
                *last = current.clone();
                return Some(current);
            }
        }
        None
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
