use async_trait::async_trait;
use rd_core::domain::{
    models::*,
    ports::InputInjector,
    error::InjectionError,
};
use tracing::{debug, info, warn};
use crate::screen_capture::macos::{LAST_CAPTURE_WIDTH, LAST_CAPTURE_HEIGHT};

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> core_graphics::geometry::CGRect;
}

/// macOS input injection using CGEvent
/// Note: CGEventSource is not Send, so we use spawn_blocking
pub struct MacOSInputInjector;

impl MacOSInputInjector {
    pub fn new() -> Result<Self, InjectionError> {
        debug!("Initializing macOS input injector");
        let is_trusted = unsafe { AXIsProcessTrusted() };
        if !is_trusted {
            warn!("⚠️ macOS Accessibility permission NOT granted! Mouse/keyboard injection will not work until enabled in System Settings -> Privacy & Security -> Accessibility.");
        } else {
            info!("macOS: Accessibility permission is GRANTED");
        }
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

use std::sync::atomic::{AtomicI32, Ordering};

static LAST_MOUSE_X: AtomicI32 = AtomicI32::new(0);
static LAST_MOUSE_Y: AtomicI32 = AtomicI32::new(0);

fn map_screen_coordinates(x: i32, y: i32) -> core_graphics::geometry::CGPoint {
    use core_graphics::geometry::CGPoint;
    let cap_w = LAST_CAPTURE_WIDTH.load(Ordering::Relaxed);
    let cap_h = LAST_CAPTURE_HEIGHT.load(Ordering::Relaxed);
    let display_id = unsafe { CGMainDisplayID() };
    let bounds = unsafe { CGDisplayBounds(display_id) };
    if cap_w > 0 && cap_h > 0 {
        let sx = bounds.size.width / cap_w as f64;
        let sy = bounds.size.height / cap_h as f64;
        // Clamp with safe margins (15px X, 25px Y) to prevent hitting macOS Hot Corners at (0, 0) or triggering Desktop 0
        let target_x = (x as f64 * sx).clamp(15.0, bounds.size.width - 15.0);
        let target_y = (y as f64 * sy).clamp(25.0, bounds.size.height - 15.0);
        CGPoint::new(target_x, target_y)
    } else {
        CGPoint::new((x as f64).clamp(15.0, bounds.size.width - 15.0), (y as f64).clamp(25.0, bounds.size.height - 15.0))
    }
}

/// Synchronous event injection using CGEvent (runs on blocking thread)
fn inject_event_sync(event: InputEvent) -> Result<(), InjectionError> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGMouseButton, CGEventType, ScrollEventUnit};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    
    let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| InjectionError::InitializationFailed("Failed to create event source".into()))?;
    
    match event {
        InputEvent::MouseMove { x, y } => {
            LAST_MOUSE_X.store(x, Ordering::Relaxed);
            LAST_MOUSE_Y.store(y, Ordering::Relaxed);
            let point = map_screen_coordinates(x, y);
            debug!("Inject mouse move: ({}, {}) -> ({}, {})", x, y, point.x, point.y);
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
            
            // Use last tracked mouse position for accurate click injection
            let x = LAST_MOUSE_X.load(Ordering::Relaxed);
            let y = LAST_MOUSE_Y.load(Ordering::Relaxed);
            let point = map_screen_coordinates(x, y);
            
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
            
            debug!("Inject mouse button at ({}, {}) -> ({}, {}): {:?} pressed={}", x, y, point.x, point.y, button, pressed);
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
            let cg_event = CGEvent::new_scroll_event(
                event_source,
                ScrollEventUnit::LINE,
                2,
                delta_y,
                delta_x,
                0,
            ).map_err(|_| InjectionError::InjectionFailed("Failed to create scroll event".into()))?;
            cg_event.post(CGEventTapLocation::HID);
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
