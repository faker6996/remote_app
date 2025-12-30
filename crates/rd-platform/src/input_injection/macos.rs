use async_trait::async_trait;
use rd_core::domain::{
    models::*,
    ports::InputInjector,
    error::InjectionError,
};
use tracing::{debug, warn};

/// macOS input injection using CGEvent
/// Note: CGEventSource is not Send, so we use spawn_blocking
pub struct MacOSInputInjector;

impl MacOSInputInjector {
    pub fn new() -> Result<Self, InjectionError> {
        debug!("Initializing macOS input injector");
        Ok(Self)
    }
}

#[async_trait]
impl InputInjector for MacOSInputInjector {
    async fn inject(&mut self, event: InputEvent) -> Result<(), InjectionError> {
        // Run CGEvent code in blocking context since CGEventSource is not Send
        tokio::task::spawn_blocking(move || {
            inject_event_sync(event)
        })
        .await
        .map_err(|e| InjectionError::InjectionFailed(format!("Task join error: {}", e)))?
    }
}

/// Synchronous event injection using CGEvent (runs on blocking thread)
fn inject_event_sync(event: InputEvent) -> Result<(), InjectionError> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGMouseButton, CGEventType};
    use core_graphics::geometry::CGPoint;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    
    let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| InjectionError::InitializationFailed("Failed to create event source".into()))?;
    
    match event {
        InputEvent::MouseMove { x, y } => {
            debug!("Inject mouse move: ({}, {})", x, y);
            let point = CGPoint::new(x as f64, y as f64);
            let cg_event = CGEvent::new_mouse_event(
                event_source,
                CGEventType::MouseMoved,
                point,
                CGMouseButton::Left,
            ).map_err(|_| InjectionError::InjectionFailed("Failed to create mouse move event".into()))?;
            cg_event.post(CGEventTapLocation::HID);
            Ok(())
        }
        InputEvent::MouseButton { button, pressed } => {
            let cg_button = match button {
                MouseButton::Left => CGMouseButton::Left,
                MouseButton::Right => CGMouseButton::Right,
                MouseButton::Middle => CGMouseButton::Center,
                _ => return Err(InjectionError::UnsupportedEvent),
            };
            
            // For button events, we need to get current mouse location
            // For now, use (0,0) - in production, query CGEvent::location
            let point = CGPoint::new(0.0, 0.0);
            
            let event_type = if pressed {
                match button {
                    MouseButton::Left => CGEventType::LeftMouseDown,
                    MouseButton::Right => CGEventType::RightMouseDown,
                    _ => CGEventType::OtherMouseDown,
                }
            } else {
                match button {
                    MouseButton::Left => CGEventType::LeftMouseUp,
                    MouseButton::Right => CGEventType::RightMouseUp,
                    _ => CGEventType::OtherMouseUp,
                }
            };
            
            debug!("Inject mouse button: {:?} pressed={}", button, pressed);
            let cg_event = CGEvent::new_mouse_event(
                event_source,
                event_type,
                point,
                cg_button,
            ).map_err(|_| InjectionError::InjectionFailed("Failed to create mouse button event".into()))?;
            cg_event.post(CGEventTapLocation::HID);
            Ok(())
        }
        InputEvent::MouseScroll { delta_x, delta_y } => {
            debug!("Inject mouse scroll: ({}, {})", delta_x, delta_y);
            warn!("Mouse scroll not yet implemented");
            Ok(())
        }
        InputEvent::KeyPress { key, pressed } => {
            debug!("Inject key: {:?} pressed={}", key, pressed);
            let cg_event = CGEvent::new_keyboard_event(
                event_source,
                key.0 as u16,
                pressed,
            ).map_err(|_| InjectionError::InjectionFailed("Failed to create keyboard event".into()))?;
            cg_event.post(CGEventTapLocation::HID);
            Ok(())
        }
    }
}
