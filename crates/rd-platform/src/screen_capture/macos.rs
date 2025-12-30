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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

static FRAME_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
                let bytes_per_row = width * 4; // BGRA = 4 bytes per pixel, assuming no padding
                
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
                    format: FrameFormat::Raw, // BGRA format from macOS
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

impl MacOSScreenCapture {
    pub fn new() -> Result<Self, CaptureError> {
        debug!("Initializing macOS screen capture (ScreenCaptureKit)");
        Ok(Self {
            display_id: 0, // Main display
            stream: None,
            rx: None,
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
        
        let width = display.width as u32;
        let height = display.height as u32;
        
        let display_id = display.display_id;
        
        info!("macOS: Capturing display {} ({}x{})", display_id, width, height);
        
        let filter = SCContentFilter::new(InitParams::Display(display));
        let config = SCStreamConfiguration::from_size(width, height, false);
        
        let (tx, rx) = watch::channel(None);
        let dimensions = Arc::new(DisplayDimensions { width, height });
        let handler = StreamHandler { tx, dimensions };
        
        // Create and start stream
        let stream = SCStream::new(filter, config, handler);
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
            self.start_stream().await?;
        }
        
        let rx = self.rx.as_mut().ok_or(CaptureError::InitializationFailed("No receiver".into()))?;
        
        // Wait for new frame
        rx.changed().await.map_err(|_| CaptureError::CaptureFailed("Stream ended".into()))?;
        
        let frame = rx.borrow().clone();
        frame.ok_or(CaptureError::CaptureFailed("No frame received".into()))
    }

    async fn get_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        let content = SCShareableContent::current();
        let displays = content.displays.iter().map(|d| DisplayInfo {
            id: d.display_id,
            name: format!("Display {}", d.display_id),
            width: d.width as u32,
            height: d.height as u32,
            x: 0, // SCDisplay doesn't expose position
            y: 0,
            is_primary: d.display_id == 0,
        }).collect();
        Ok(displays)
    }
    
    async fn set_target_display(&mut self, display_id: u32) -> Result<(), CaptureError> {
        self.display_id = display_id;
        // Stop current stream if running, will restart on next capture
        self.stream = None;
        self.rx = None;
        Ok(())
    }
}
