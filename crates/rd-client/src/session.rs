use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

use rd_core::domain::{
    models::*,
    ports::*,
    error::*,
};

use rd_codec::jpeg::JpegDecoder;

/// Remote session client
pub struct RemoteSession {
    session_id: Option<SessionId>,
    transport: Arc<tokio::sync::Mutex<dyn Transport>>,
    decoder: Arc<tokio::sync::Mutex<dyn Decoder>>,
    frame_receiver: mpsc::UnboundedReceiver<ScreenFrame>,
}

impl RemoteSession {
    /// Create a new remote session
    pub async fn new(transport: Arc<tokio::sync::Mutex<dyn Transport>>) -> Result<Self, ApplicationError> {
        let decoder = Arc::new(tokio::sync::Mutex::new(JpegDecoder::new()));
        let (tx, rx) = mpsc::unbounded_channel();
        
        let transport_clone = transport.clone();
        let decoder_clone = decoder.clone();
        
        // Start frame receiver task
        tokio::spawn(async move {
            loop {
                let message = match transport_clone.lock().await.receive().await {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("Failed to receive message: {}", e);
                        break;
                    }
                };
                
                if let ProtocolMessage::ScreenFrame { data, sequence, timestamp, width, height, format } = message {
                    // Decode frame
                    match decoder_clone.lock().await.decode(&data).await {
                        Ok(mut frame) => {
                            frame.sequence = sequence;
                            frame.timestamp = timestamp;
                            
                            if tx.send(frame).is_err() {
                                warn!("Frame receiver dropped");
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to decode frame: {}", e);
                        }
                    }
                }
            }
        });
        
        Ok(Self {
            session_id: None,
            transport,
            decoder,
            frame_receiver: rx,
        })
    }
    
    /// Connect to a remote agent
    pub async fn connect(&mut self, agent_device_id: String) -> Result<SessionId, ApplicationError> {
        info!("Requesting session with agent: {}", agent_device_id);
        
        // Send session request
        self.transport
            .lock()
            .await
            .send(ProtocolMessage::SessionRequest {
                target_device: agent_device_id,
            })
            .await?;
        
        // Wait for session created response
        // TODO: Implement proper session negotiation
        
        let session_id = SessionId::new();
        self.session_id = Some(session_id);
        
        info!("Session created: {}", session_id);
        
        Ok(session_id)
    }
    
    /// Receive the next frame
    pub async fn receive_frame(&mut self) -> Option<ScreenFrame> {
        self.frame_receiver.recv().await
    }
    
    /// Send an input event
    pub async fn send_input(&mut self, event: InputEvent) -> Result<(), ApplicationError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        self.transport
            .lock()
            .await
            .send(ProtocolMessage::InputEvent { timestamp, event })
            .await?;
        
        Ok(())
    }
    
    /// Disconnect from the session
    pub async fn disconnect(&mut self) -> Result<(), ApplicationError> {
        info!("Disconnecting session");
        
        self.transport
            .lock()
            .await
            .send(ProtocolMessage::Disconnect)
            .await?;
        
        self.transport.lock().await.close().await?;
        
        Ok(())
    }
}
