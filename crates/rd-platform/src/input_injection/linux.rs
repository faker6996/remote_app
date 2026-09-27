use async_trait::async_trait;
use rd_core::domain::{
    models::*,
    ports::InputInjector,
    error::InjectionError,
};
use tracing::debug;
use x11rb::connection::Connection;
use x11rb::protocol::xproto;
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

/// Linux input injection using XTest extension
pub struct LinuxInputInjector {
    conn: RustConnection,
    root: xproto::Window,
}

impl LinuxInputInjector {
    pub fn new() -> Result<Self, InjectionError> {
        debug!("Initializing Linux input injector (XTest)");
        
        let (conn, screen_num) = x11rb::connect(None)
            .map_err(|e| InjectionError::InitializationFailed(format!("Failed to connect to X11: {}", e)))?;
            
        let root = conn.setup().roots.get(screen_num)
            .ok_or_else(|| InjectionError::InitializationFailed("Screen index not found".to_string()))?
            .root;
            
        Ok(Self {
            conn,
            root,
        })
    }
}

#[async_trait]
impl InputInjector for LinuxInputInjector {
    async fn inject(&mut self, event: InputEvent) -> Result<(), InjectionError> {
        match event {
            InputEvent::MouseMove { x, y } => {
                // MOTION_NOTIFY_EVENT = 6
                self.conn.xtest_fake_input(
                    xproto::MOTION_NOTIFY_EVENT,
                    0,
                    0,
                    self.root,
                    x as i16,
                    y as i16,
                    0,
                ).map_err(|e| InjectionError::InjectionFailed(e.to_string()))?;
            }
            
            InputEvent::MouseButton { button, pressed } => {
                let type_ = if pressed {
                    xproto::BUTTON_PRESS_EVENT
                } else {
                    xproto::BUTTON_RELEASE_EVENT
                };
                
                let detail: u8 = match button {
                    MouseButton::Left => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Right => 3,
                    MouseButton::X1 => 8,
                    MouseButton::X2 => 9,
                };
                
                self.conn.xtest_fake_input(
                    type_,
                    detail,
                    0,
                    self.root,
                    0,
                    0,
                    0,
                ).map_err(|e| InjectionError::InjectionFailed(e.to_string()))?;
            }
            
            InputEvent::MouseScroll { delta_x, delta_y } => {
                if delta_y != 0 {
                    let button: u8 = if delta_y > 0 { 4 } else { 5 }; // 4 = Up, 5 = Down
                    // Send Press then Release
                    self.conn.xtest_fake_input(xproto::BUTTON_PRESS_EVENT, button, 0, self.root, 0, 0, 0)
                        .map_err(|e| InjectionError::InjectionFailed(e.to_string()))?;
                    self.conn.xtest_fake_input(xproto::BUTTON_RELEASE_EVENT, button, 0, self.root, 0, 0, 0)
                        .map_err(|e| InjectionError::InjectionFailed(e.to_string()))?;
                }
                
                if delta_x != 0 {
                    let button: u8 = if delta_x > 0 { 7 } else { 6 }; // 7 = Right, 6 = Left
                    self.conn.xtest_fake_input(xproto::BUTTON_PRESS_EVENT, button, 0, self.root, 0, 0, 0)
                        .map_err(|e| InjectionError::InjectionFailed(e.to_string()))?;
                    self.conn.xtest_fake_input(xproto::BUTTON_RELEASE_EVENT, button, 0, self.root, 0, 0, 0)
                        .map_err(|e| InjectionError::InjectionFailed(e.to_string()))?;
                }
            }
            
            InputEvent::KeyPress { key, pressed } => {
                let type_ = if pressed {
                    xproto::KEY_PRESS_EVENT
                } else {
                    xproto::KEY_RELEASE_EVENT
                };
                
                self.conn.xtest_fake_input(
                    type_,
                    key.0 as u8,
                    0,
                    self.root,
                    0,
                    0,
                    0,
                ).map_err(|e| InjectionError::InjectionFailed(e.to_string()))?;
            }
        }
        
        // Flush commands to X server
        self.conn.flush().map_err(|e| InjectionError::InjectionFailed(e.to_string()))?;
        Ok(())
    }
}
