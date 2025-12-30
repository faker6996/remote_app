use quinn::Connection;
use tracing::{info, warn, error};
use anyhow::Result;

use rd_transport::{QuicTransport, ProtocolMessage};
use rd_core::domain::ports::Transport;

use crate::state::ServerState;

/// Handle incoming connection
pub async fn handle_connection(connection: Connection, state: ServerState) -> Result<()> {
    let remote_addr = connection.remote_address();
    info!("Handling connection from {}", remote_addr);
    
    let mut transport = QuicTransport::new(connection).await?;
    
    // Wait for Hello message
    match transport.receive().await {
        Ok(ProtocolMessage::Hello { version, device_id, platform }) => {
            info!("Received Hello from device: {} (platform: {:?})", device_id, platform);
            
            // TODO: Validate version
            // TODO: Authenticate device
            
            // Register agent
            let peer_id = rd_core::domain::models::PeerId::new(device_id.clone());
            state.register_agent(device_id, peer_id);
            
            // Handle subsequent messages
            loop {
                match transport.receive().await {
                    Ok(msg) => {
                        if let Err(e) = handle_message(msg, &state, &mut transport).await {
                            error!("Error handling message: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Transport error: {}", e);
                        break;
                    }
                }
            }
        }
        Ok(msg) => {
            warn!("Expected Hello, got: {:?}", msg);
        }
        Err(e) => {
            error!("Failed to receive Hello: {}", e);
        }
    }
    
    Ok(())
}

async fn handle_message(
    msg: ProtocolMessage,
    state: &ServerState,
    transport: &mut QuicTransport,
) -> Result<()> {
    match msg {
        ProtocolMessage::Heartbeat { timestamp } => {
            // Respond with heartbeat
            transport.send(ProtocolMessage::Heartbeat { timestamp }).await?;
        }
        ProtocolMessage::SessionRequest { target_device } => {
            info!("Session request for device: {}", target_device);
            
            // TODO: Create session
            // TODO: Notify target agent
            // TODO: Return session info
        }
        ProtocolMessage::Disconnect => {
            info!("Client disconnecting");
            return Err(anyhow::anyhow!("Client disconnected"));
        }
        _ => {
            warn!("Unexpected message: {:?}", msg);
        }
    }
    
    Ok(())
}
