use async_trait::async_trait;
use rd_core::domain::{
    models::*,
    ports::ScreenCapture,
    error::CaptureError,
};
use tracing::debug;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt as _};
use x11rb::rust_connection::RustConnection;

/// Linux screen capture using X11
pub struct LinuxScreenCapture {
    conn: RustConnection,
    screen_num: usize,
    display_id: u32,
    sequence: u64,
}

impl LinuxScreenCapture {
    pub fn new() -> Result<Self, CaptureError> {
        debug!("Initializing Linux screen capture (X11)");
        
        let (conn, screen_num) = x11rb::connect(None)
            .map_err(|e| CaptureError::InitializationFailed(format!("Failed to connect to X11: {}", e)))?;
            
        Ok(Self {
            conn,
            screen_num,
            display_id: 0,
            sequence: 0,
        })
    }
}

#[async_trait]
impl ScreenCapture for LinuxScreenCapture {
    async fn capture(&mut self) -> Result<ScreenFrame, CaptureError> {
        let screen = self.conn.setup().roots.get(self.screen_num)
            .ok_or_else(|| CaptureError::CaptureFailed("Screen index not found".to_string()))?;
            
        let root = screen.root;
        let width = screen.width_in_pixels;
        let height = screen.height_in_pixels;
        
        let reply = self.conn.get_image(
            xproto::ImageFormat::Z_PIXMAP,
            root,
            0,
            0,
            width,
            height,
            !0,
        ).map_err(|e| CaptureError::CaptureFailed(format!("get_image request error: {}", e)))?
        .reply()
        .map_err(|e| CaptureError::CaptureFailed(format!("get_image reply error: {}", e)))?;
        
        self.sequence += 1;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        Ok(ScreenFrame {
            sequence: self.sequence,
            timestamp,
            data: reply.data,
            width: width as u32,
            height: height as u32,
            format: FrameFormat::Bgra,
        })
    }
    
    async fn get_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        let roots = &self.conn.setup().roots;
        let mut displays = Vec::new();
        
        for (i, root) in roots.iter().enumerate() {
            displays.push(DisplayInfo {
                id: i as u32,
                name: format!("X11 Display {}", i),
                width: root.width_in_pixels as u32,
                height: root.height_in_pixels as u32,
                x: 0,
                y: 0,
                is_primary: i == self.screen_num,
            });
        }
        
        if displays.is_empty() {
            displays.push(DisplayInfo {
                id: 0,
                name: "Primary Display".to_string(),
                width: 1920,
                height: 1080,
                x: 0,
                y: 0,
                is_primary: true,
            });
        }
        
        Ok(displays)
    }
    
    async fn set_target_display(&mut self, display_id: u32) -> Result<(), CaptureError> {
        if (display_id as usize) < self.conn.setup().roots.len() {
            self.screen_num = display_id as usize;
            self.display_id = display_id;
            debug!("Set target display to {}", display_id);
            Ok(())
        } else {
            Err(CaptureError::DisplayNotFound(display_id))
        }
    }
}
