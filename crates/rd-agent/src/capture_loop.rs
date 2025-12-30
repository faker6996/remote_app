use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{info, warn, error};

use rd_core::domain::ports::{ScreenCapture, Encoder, Transport, ProtocolMessage};

pub async fn run_capture_loop(
    screen_capture: Arc<tokio::sync::Mutex<dyn ScreenCapture>>,
    encoder: Arc<tokio::sync::Mutex<dyn Encoder>>,
    transport: Arc<tokio::sync::Mutex<dyn Transport>>,
    max_fps: u8,
) -> anyhow::Result<()> {
    info!("Starting capture loop at {} FPS", max_fps);
    
    let frame_interval = Duration::from_millis(1000 / max_fps as u64);
    let mut ticker = interval(frame_interval);
    let mut sequence = 0u64;
    
    loop {
        ticker.tick().await;
        
        // Capture frame
        let frame = match screen_capture.lock().await.capture().await {
            Ok(f) => f,
            Err(e) => {
                warn!("Screen capture failed: {}", e);
                continue;
            }
        };
        
        // Encode frame
        let encoded_data = match encoder.lock().await.encode(&frame).await {
            Ok(data) => data,
            Err(e) => {
                error!("Frame encoding failed: {}", e);
                continue;
            }
        };
        
        // Send frame
        let message = ProtocolMessage::ScreenFrame {
            sequence,
            timestamp: frame.timestamp,
            data: encoded_data,
            width: frame.width,
            height: frame.height,
            format: frame.format,
        };
        
        if let Err(e) = transport.lock().await.send(message).await {
            error!("Failed to send frame: {}", e);
            break;
        }
        
        sequence += 1;
    }
    
    Ok(())
}
