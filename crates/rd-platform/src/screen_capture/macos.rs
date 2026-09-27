use async_trait::async_trait;
use rd_core::domain::{
    error::CaptureError,
    models::{DisplayInfo, FrameFormat, ScreenFrame},
    ports::ScreenCapture,
};
use screencapturekit::{
    sc_content_filter::{InitParams, SCContentFilter},
    sc_output_handler::{SCStreamOutputType, StreamOutput},
    sc_error_handler::StreamErrorHandler,
    sc_shareable_content::SCShareableContent,
    sc_stream::SCStream,
    sc_stream_configuration::SCStreamConfiguration,
    cm_sample_buffer::CMSampleBuffer,
};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

static FRAME_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub static LAST_CAPTURE_WIDTH: AtomicI32 = AtomicI32::new(0);
pub static LAST_CAPTURE_HEIGHT: AtomicI32 = AtomicI32::new(0);

/// Shared dimensions from display (since CVPixelBuffer in this crate version doesn't expose width/height)
struct DisplayDimensions {
    width: u32,
    height: u32,
}

struct StreamHandler {
    tx: watch::Sender<Option<ScreenFrame>>,
    dimensions: Arc<DisplayDimensions>,
}

impl StreamOutput for StreamHandler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        match of_type {
            SCStreamOutputType::Screen => {
                // Get pixel buffer from sample
                let pixel_buffer = match &sample.pixel_buffer {
                    Some(pb) => pb,
                    None => {
                        warn!("macOS: No pixel buffer in sample");
                        return;
                    }
                };

                // Lock the buffer for reading (using crate's API which has typo "adress")
                if !pixel_buffer.lock() {
                    warn!("macOS: Failed to lock pixel buffer");
                    return;
                }

                // Use dimensions from display (since this crate version doesn't expose CVPixelBufferGetWidth etc)
                let width = self.dimensions.width;
                let height = self.dimensions.height;
                LAST_CAPTURE_WIDTH.store(width as i32, Ordering::Relaxed);
                LAST_CAPTURE_HEIGHT.store(height as i32, Ordering::Relaxed);
                let _bytes_per_row = width * 4;
                
                // Get raw pixel data pointer (note: crate has typo "adress")
                let base_ptr = pixel_buffer.get_base_adress();
                if base_ptr.is_null() {
                    warn!("macOS: Null base address");
                    pixel_buffer.unlock();
                    return;
                }

                // Copy pixel data
                let data_size = (width * height * 4) as usize;
                let data = unsafe {
                    let base = base_ptr as *const u8;
                    std::slice::from_raw_parts(base, data_size).to_vec()
                };

                // Unlock the buffer
                pixel_buffer.unlock();

                // Get timestamp
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);

                let sequence = FRAME_SEQUENCE.fetch_add(1, Ordering::SeqCst);

                debug!(
                    "macOS: Captured frame {}x{} (data_len={})",
                    width, height, data.len()
                );

                let frame = ScreenFrame {
                    sequence,
                    timestamp,
                    data,
                    width,
                    height,
                    format: FrameFormat::Bgra,
                };
                
                let _ = self.tx.send(Some(frame));
            }
            _ => {}
        }
    }
}

impl StreamErrorHandler for StreamHandler {
    fn on_error(&self) {
        error!("Stream error occurred");
    }
}

pub struct MacOSScreenCapture {
    display_id: u32,
    stream: Option<SCStream>,
    rx: Option<watch::Receiver<Option<ScreenFrame>>>,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

impl MacOSScreenCapture {
    pub fn new() -> Result<Self, CaptureError> {
        debug!("Initializing macOS screen capture (ScreenCaptureKit with CoreGraphics fallback)");
        let has_permission = unsafe { CGPreflightScreenCaptureAccess() };
        if !has_permission {
            warn!("macOS: Screen Recording permission NOT granted! Requesting access dialog...");
            unsafe { CGRequestScreenCaptureAccess(); }
        } else {
            info!("macOS: Screen Recording permission is GRANTED");
        }
        Ok(Self {
            display_id: 0, // Main display
            stream: None,
            rx: None,
        })
    }

    fn capture_core_graphics(&self) -> Result<ScreenFrame, CaptureError> {
        let display_id = if self.display_id == 0 {
            unsafe { core_graphics::display::CGMainDisplayID() }
        } else {
            self.display_id
        };
        
        let display = core_graphics::display::CGDisplay::new(display_id);
        let image = display.image().ok_or_else(|| {
            CaptureError::CaptureFailed(format!("CoreGraphics display {} image returned None", display_id))
        })?;
        
        let raw_width = image.width() as u32;
        let raw_height = image.height() as u32;
        let bpr = image.bytes_per_row();
        let data_ref = image.data();
        let bytes = data_ref.bytes();
        
        // If retina display (>1600 width), downsample 2x for high performance and matching logical display points
        let step = if raw_width > 1600 { 2 } else { 1 };
        let out_width = raw_width / step;
        let out_height = raw_height / step;
        LAST_CAPTURE_WIDTH.store(out_width as i32, Ordering::Relaxed);
        LAST_CAPTURE_HEIGHT.store(out_height as i32, Ordering::Relaxed);
        let mut data = Vec::with_capacity((out_width * out_height * 4) as usize);
        for y in (0..raw_height).step_by(step as usize) {
            let row_offset = y as usize * bpr;
            if row_offset >= bytes.len() { break; }
            let row_bytes = &bytes[row_offset..];
            for x in (0..raw_width).step_by(step as usize) {
                let px_offset = x as usize * 4;
                if px_offset + 4 <= row_bytes.len() {
                    data.extend_from_slice(&row_bytes[px_offset..px_offset + 4]);
                } else {
                    data.extend_from_slice(&[0, 0, 0, 255]);
                }
            }
        }
        
        let sequence = FRAME_SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
            
        Ok(ScreenFrame {
            sequence,
            timestamp,
            data,
            width: out_width,
            height: out_height,
            format: FrameFormat::Bgra,
        })
    }

    async fn start_stream(&mut self) -> Result<(), CaptureError> {
        // Fetch shareable content (synchronous in this crate version)
        let content = SCShareableContent::current();
        
        let displays = content.displays;
        if displays.is_empty() {
            return Err(CaptureError::DisplayNotFound(0));
        }
        
        // Find display matching ID or use first
        let display = displays.iter()
            .find(|d| d.display_id == self.display_id)
            .unwrap_or(&displays[0])
            .clone();
        
        let raw_width = display.width as u32;
        let raw_height = display.height as u32;
        
        // Maximum capture resolution: 1920 Full HD (preserves aspect ratio)
        let max_dim = 1920.0f32;
        let scale = if raw_width as f32 > max_dim {
            max_dim / raw_width as f32
        } else {
            1.0f32
        };
        let width = ((raw_width as f32 * scale).round() as u32) & !1;
        let height = ((raw_height as f32 * scale).round() as u32) & !1;
        
        let display_id = display.display_id;
        
        info!("macOS: Capturing display {} (native: {}x{}, scaled: {}x{})", 
            display_id, raw_width, raw_height, width, height);
        
        let filter = SCContentFilter::new(InitParams::Display(display));
        let config = SCStreamConfiguration::from_size(width, height, false);
        
        let (tx, rx) = watch::channel(None);
        let dimensions = Arc::new(DisplayDimensions { width, height });
        let handler = StreamHandler { tx, dimensions };
        
        // Create and start stream with ErrorHandler and add StreamOutput for screen
        struct ErrorHandler;
        impl StreamErrorHandler for ErrorHandler {
            fn on_error(&self) {
                error!("macOS SCStream error occurred");
            }
        }
        
        let mut stream = SCStream::new(filter, config, ErrorHandler);
        stream.add_output(handler, SCStreamOutputType::Screen);
        stream.start_capture().map_err(|e| CaptureError::CaptureFailed(format!("Start failed: {:?}", e)))?;
        
        self.stream = Some(stream);
        self.rx = Some(rx);
        
        info!("macOS: Stream started successfully");
        Ok(())
    }
}

#[async_trait]
impl ScreenCapture for MacOSScreenCapture {
    async fn capture(&mut self) -> Result<ScreenFrame, CaptureError> {
        if self.stream.is_none() {
            if let Err(e) = self.start_stream().await {
                warn!("ScreenCaptureKit start_stream failed: {}, falling back to CoreGraphics", e);
                return self.capture_core_graphics();
            }
        }
        
        if let Some(ref mut rx) = self.rx {
            match tokio::time::timeout(std::time::Duration::from_millis(35), rx.changed()).await {
                Ok(Ok(())) => {
                    if let Some(frame) = rx.borrow().clone() {
                        return Ok(frame);
                    }
                }
                Ok(Err(_)) => {
                    warn!("macOS SCStream ended, falling back to CoreGraphics");
                    self.stream = None;
                    self.rx = None;
                }
                Err(_) => {
                    // Screen didn't change this tick (normal for ScreenCaptureKit when screen is idle).
                    // If we already have a frame, return it to avoid expensive CoreGraphics capture!
                    if let Some(frame) = rx.borrow().clone() {
                        return Ok(frame);
                    }
                }
            }
        }
        
        self.capture_core_graphics()
    }

    async fn get_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        let displays: Vec<DisplayInfo> = {
            let content = SCShareableContent::current();
            content.displays.iter().map(|d| DisplayInfo {
                id: d.display_id,
                name: format!("Display {}", d.display_id),
                width: d.width as u32,
                height: d.height as u32,
                x: 0, // SCDisplay doesn't expose position
                y: 0,
                is_primary: d.display_id == 0,
            }).collect()
        };
        
        if !displays.is_empty() {
            return Ok(displays);
        }
        
        // Fallback to CoreGraphics main display
        let main_id = unsafe { core_graphics::display::CGMainDisplayID() };
        let disp = core_graphics::display::CGDisplay::new(main_id);
        let bounds = disp.bounds();
        Ok(vec![DisplayInfo {
            id: main_id,
            name: format!("Display {}", main_id),
            width: bounds.size.width as u32,
            height: bounds.size.height as u32,
            x: bounds.origin.x as i32,
            y: bounds.origin.y as i32,
            is_primary: true,
        }])
    }
    
    async fn set_target_display(&mut self, display_id: u32) -> Result<(), CaptureError> {
        self.display_id = display_id;
        // Stop current stream if running, will restart on next capture
        self.stream = None;
        self.rx = None;
        Ok(())
    }
}
