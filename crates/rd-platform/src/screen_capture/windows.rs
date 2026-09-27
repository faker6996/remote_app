use async_trait::async_trait;
use rd_core::domain::{
    models::*,
    ports::ScreenCapture,
    error::CaptureError,
};
use tracing::debug;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
    GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
};

/// Windows screen capture implementation using Win32 GDI
pub struct WindowsScreenCapture {
    display_id: u32,
    sequence: u64,
}

impl WindowsScreenCapture {
    pub fn new() -> Result<Self, CaptureError> {
        debug!("Initializing Windows screen capture");
        Ok(Self {
            display_id: 0,
            sequence: 0,
        })
    }
}

#[async_trait]
impl ScreenCapture for WindowsScreenCapture {
    async fn capture(&mut self) -> Result<ScreenFrame, CaptureError> {
        let (width, height, raw_bgra) = tokio::task::spawn_blocking(move || -> Result<(u32, u32, Vec<u8>), CaptureError> {
            unsafe {
                let hdc_screen = GetDC(HWND::default());
                if hdc_screen.is_invalid() {
                    return Err(CaptureError::InitializationFailed("Failed to get desktop DC".into()));
                }
                
                let width = GetSystemMetrics(SM_CXSCREEN);
                let height = GetSystemMetrics(SM_CYSCREEN);
                if width <= 0 || height <= 0 {
                    let _ = ReleaseDC(HWND::default(), hdc_screen);
                    return Err(CaptureError::InitializationFailed("Invalid screen dimensions".into()));
                }
                
                let hdc_mem = CreateCompatibleDC(hdc_screen);
                if hdc_mem.is_invalid() {
                    let _ = ReleaseDC(HWND::default(), hdc_screen);
                    return Err(CaptureError::InitializationFailed("Failed to create compatible DC".into()));
                }
                
                let hbm_screen = CreateCompatibleBitmap(hdc_screen, width, height);
                if hbm_screen.is_invalid() {
                    let _ = DeleteDC(hdc_mem);
                    let _ = ReleaseDC(HWND::default(), hdc_screen);
                    return Err(CaptureError::InitializationFailed("Failed to create compatible bitmap".into()));
                }
                
                let old_obj = SelectObject(hdc_mem, hbm_screen);
                
                let blt_res = BitBlt(
                    hdc_mem,
                    0, 0,
                    width, height,
                    hdc_screen,
                    0, 0,
                    SRCCOPY,
                );
                
                if let Err(e) = blt_res {
                    SelectObject(hdc_mem, old_obj);
                    let _ = DeleteObject(hbm_screen);
                    let _ = DeleteDC(hdc_mem);
                    let _ = ReleaseDC(HWND::default(), hdc_screen);
                    return Err(CaptureError::CaptureFailed(format!("BitBlt failed: {}", e)));
                }
                
                let mut bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width,
                        biHeight: -height, // Negative height specifies top-down DIB
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        biSizeImage: 0,
                        biXPelsPerMeter: 0,
                        biYPelsPerMeter: 0,
                        biClrUsed: 0,
                        biClrImportant: 0,
                    },
                    bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD::default(); 1],
                };
                
                let buffer_size = (width * height * 4) as usize;
                let mut buffer: Vec<u8> = vec![0; buffer_size];
                
                let dib_res = GetDIBits(
                    hdc_mem,
                    hbm_screen,
                    0,
                    height as u32,
                    Some(buffer.as_mut_ptr() as *mut _),
                    &mut bmi,
                    DIB_RGB_COLORS,
                );
                
                // Cleanup GDI objects
                SelectObject(hdc_mem, old_obj);
                let _ = DeleteObject(hbm_screen);
                let _ = DeleteDC(hdc_mem);
                let _ = ReleaseDC(HWND::default(), hdc_screen);
                
                if dib_res == 0 {
                    return Err(CaptureError::CaptureFailed("GetDIBits failed to copy pixels".into()));
                }
                
                Ok((width as u32, height as u32, buffer))
            }
        }).await.map_err(|e| CaptureError::CaptureFailed(format!("Capture task join error: {}", e)))??;
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        self.sequence += 1;
        
        Ok(ScreenFrame {
            sequence: self.sequence,
            timestamp,
            data: raw_bgra,
            width,
            height,
            format: FrameFormat::Bgra,
        })
    }
    
    async fn get_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        let (w, h) = unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            (if width <= 0 { 1920 } else { width as u32 }, if height <= 0 { 1080 } else { height as u32 })
        };
        
        Ok(vec![DisplayInfo {
            id: 0,
            name: "Primary Display".to_string(),
            width: w,
            height: h,
            x: 0,
            y: 0,
            is_primary: true,
        }])
    }
    
    async fn set_target_display(&mut self, display_id: u32) -> Result<(), CaptureError> {
        self.display_id = display_id;
        debug!("Set target display to {}", display_id);
        Ok(())
    }
}
