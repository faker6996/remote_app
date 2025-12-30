use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

use crate::domain::{
    models::*,
    ports::*,
    error::*,
};

/// Stream Controller - Orchestrates screen capture and streaming
pub struct StreamController {
    capture: Arc<tokio::sync::Mutex<dyn ScreenCapture>>,
    encoder: Arc<tokio::sync::Mutex<dyn Encoder>>,
    transport: Arc<tokio::sync::Mutex<dyn Transport>>,
    config: StreamConfig,
}

#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub max_fps: u8,
    pub drop_frames_on_slow_client: bool,
    pub frame_buffer_size: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            max_fps: 30,
            drop_frames_on_slow_client: true,
            frame_buffer_size: 2,
        }
    }
}

impl StreamController {
    pub fn new(
        capture: Arc<tokio::sync::Mutex<dyn ScreenCapture>>,
        encoder: Arc<tokio::sync::Mutex<dyn Encoder>>,
        transport: Arc<tokio::sync::Mutex<dyn Transport>>,
        config: StreamConfig,
    ) -> Self {
        Self {
            capture,
            encoder,
            transport,
            config,
        }
    }
    
    /// Start the streaming loop
    pub async fn start_streaming(&self) -> Result<()> {
        info!("Starting screen streaming at {} FPS", self.config.max_fps);
        
        let frame_interval = std::time::Duration::from_millis(1000 / self.config.max_fps as u64);
        let mut interval = tokio::time::interval(frame_interval);
        let mut sequence: u64 = 0;
        
        loop {
            interval.tick().await;
            
            // Capture frame
            let frame = match self.capture.lock().await.capture().await {
                Ok(f) => f,
                Err(e) => {
                    warn!("Screen capture failed: {}", e);
                    continue;
                }
            };
            
            // Encode frame
            let encoded_data = match self.encoder.lock().await.encode(&frame).await {
                Ok(data) => data,
                Err(e) => {
                    error!("Frame encoding failed: {}", e);
                    continue;
                }
            };
            
            // Send via transport
            let message = ProtocolMessage::ScreenFrame {
                sequence,
                timestamp: frame.timestamp,
                data: encoded_data,
                width: frame.width,
                height: frame.height,
                format: frame.format,
            };
            
            match self.transport.lock().await.send(message).await {
                Ok(_) => {
                    sequence += 1;
                }
                Err(e) => {
                    error!("Failed to send frame: {}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    /// Handle incoming input events
    pub async fn handle_input_stream(
        injector: Arc<tokio::sync::Mutex<dyn InputInjector>>,
        transport: Arc<tokio::sync::Mutex<dyn Transport>>,
    ) -> Result<()> {
        info!("Starting input event handler");
        
        loop {
            let message = match transport.lock().await.receive().await {
                Ok(msg) => msg,
                Err(e) => {
                    error!("Failed to receive message: {}", e);
                    break;
                }
            };
            
            if let ProtocolMessage::InputEvent { event, .. } = message {
                if let Err(e) = injector.lock().await.inject(event).await {
                    warn!("Failed to inject input event: {}", e);
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // TODO: Add tests with mock capture, encoder, transport
}
