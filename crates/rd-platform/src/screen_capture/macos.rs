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
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use tracing::{debug, error, info};

struct StreamHandler {
    tx: watch::Sender<Option<ScreenFrame>>,
}

impl StreamOutput for StreamHandler {
    fn did_output_sample_buffer(&self, _sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        match of_type {
            SCStreamOutputType::Screen => {
                debug!("macOS: Frame callback received");
                // Dummy frame for POC
                let frame = ScreenFrame {
                    sequence: 0,
                    timestamp: 0, // Should use system time
                    data: vec![255, 0, 0, 255] .repeat(100 * 100), // 100x100 Red
                    width: 100,
                    height: 100,
                    format: FrameFormat::Raw,
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
             // Assuming DisplayNotFound takes u32 based on error
            return Err(CaptureError::DisplayNotFound(0));
        }
        
        // Find display matching ID or use first
        let display = displays.iter()
            .find(|d| d.display_id == self.display_id)
            .unwrap_or(&displays[0])
            .clone();
        
        let width = display.width as u32;
        let height = display.height as u32;
        
        let filter = SCContentFilter::new(InitParams::Display(display));
        
        let config = SCStreamConfiguration::from_size(width, height, false);
        
        // Set frame rate manually if methods exist, otherwise rely on default
        // CMTime construction:
        // config.set_minimum_frame_interval(CMTime { value: 1, timescale: 60, flags: 0, epoch: 0 }); 
        // Need to check if setters exist or if it's builder pattern.
        // Assuming builder pattern based on previous error `minimum_frame_interval` existed but arg was wrong type?
        // Actually error `method not found` was for `new`.
        
        let (tx, rx) = watch::channel(None);
        let handler = StreamHandler { tx };
        
        // Create stream
        let stream = SCStream::new(filter, config, handler);
        // stream.add_stream_output(handler, type);
        // Actually new() usually returns the stream.
        // I need to check how to start it.
        // stream.start_capture().await?;
        
        self.stream = Some(stream);
        self.rx = Some(rx);
        
        if let Some(stream) = &self.stream {
            stream.start_capture().map_err(|e| CaptureError::CaptureFailed(format!("Start failed: {:?}", e)))?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl ScreenCapture for MacOSScreenCapture {
    async fn capture(&mut self) -> Result<ScreenFrame, CaptureError> {
        if self.stream.is_none() {
            self.start_stream().await?;
            info!("macOS: Stream started");
        }
        
        let rx = self.rx.as_mut().ok_or(CaptureError::InitializationFailed("No receiver".into()))?;
        
        // Wait for new frame
        rx.changed().await.map_err(|_| CaptureError::CaptureFailed("Stream ended".into()))?;
        
        let frame = rx.borrow().clone();
        frame.ok_or(CaptureError::CaptureFailed("No frame received".into()))
    }

    async fn get_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        Ok(vec![])
    }
    
    async fn set_target_display(&mut self, display_id: u32) -> Result<(), CaptureError> {
        self.display_id = display_id;
        Ok(())
    }
}
