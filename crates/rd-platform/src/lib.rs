pub mod screen_capture;
pub mod input_injection;

pub use screen_capture::create_screen_capture;
pub use input_injection::create_input_injector;

// Re-export core traits
pub use rd_core::domain::ports::{ScreenCapture, InputInjector};
