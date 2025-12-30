use std::sync::Arc;
use tracing::{info, warn, error};

use rd_core::domain::ports::{InputInjector, Transport, ProtocolMessage};

pub async fn run_input_handler(
    input_injector: Arc<tokio::sync::Mutex<dyn InputInjector>>,
    transport: Arc<tokio::sync::Mutex<dyn Transport>>,
) -> anyhow::Result<()> {
    info!("Starting input event handler");
    
    loop {
        let message = match transport.lock().await.receive().await {
            Ok(msg) => msg,
            Err(e) => {
                error!("Failed to receive message: {}", e);
                break;
            }
        };
        
        match message {
            ProtocolMessage::InputEvent { event, .. } => {
                if let Err(e) = input_injector.lock().await.inject(event).await {
                    warn!("Failed to inject input event: {}", e);
                }
            }
            ProtocolMessage::Disconnect => {
                info!("Received disconnect signal");
                break;
            }
            _ => {
                // Ignore other messages
            }
        }
    }
    
    Ok(())
}
