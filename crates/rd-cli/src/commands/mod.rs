use tracing::{info, error};
use anyhow::Result;
use std::sync::Arc;

use rd_transport::{QuicClient, QuicTransport};
use rd_client::RemoteSession;
use rd_core::domain::ports::{Transport, ProtocolMessage};

pub async fn list_agents(server: &str) -> Result<()> {
    info!("Listing agents from server: {}", server);
    
    // TODO: Implement agent list retrieval
    println!("Agent list feature not yet implemented");
    
    Ok(())
}

pub async fn connect_to_agent(agent_id: &str, server: &str, max_frames: usize) -> Result<()> {
    info!("Connecting to agent {} via server {}", agent_id, server);
    
    // Connect to server
    let client = QuicClient::new()?;
    let connection = client.connect(server.parse()?).await?;
    let transport = QuicTransport::new(connection).await?;
    
    // Create remote session
    let transport = Arc::new(tokio::sync::Mutex::new(transport));
    let mut session = RemoteSession::new(transport).await?;
    
    // Connect to agent
    let session_id = session.connect(agent_id.to_string()).await?;
    info!("Connected with session ID: {}", session_id);
    
    // Receive frames
    println!("Receiving frames (max: {})...", max_frames);
    let mut count = 0;
    
    while count < max_frames {
        if let Some(frame) = session.receive_frame().await {
            count += 1;
            println!(
                "Frame {}: {}x{}, {} bytes, seq={}",
                count,
                frame.width,
                frame.height,
                frame.data.len(),
                frame.sequence
            );
        } else {
            break;
        }
    }
    
    // Disconnect
    session.disconnect().await?;
    
    Ok(())
}

pub async fn debug_transport(server: &str) -> Result<()> {
    info!("Testing QUIC transport to: {}", server);
    
    let client = QuicClient::new()?;
    let connection = client.connect(server.parse()?).await?;
    
    println!("✓ QUIC connection established");
    println!("  Remote address: {}", connection.remote_address());
    
    let mut transport = QuicTransport::new(connection).await?;
    
    // Send Hello
    transport.send(ProtocolMessage::Hello {
        version: 1,
        device_id: "test-client".to_string(),
        platform: rd_core::domain::models::Platform::Linux,
    }).await?;
    
    println!("✓ Sent Hello message");
    
    // Send Heartbeat
    transport.send(ProtocolMessage::Heartbeat { timestamp: 12345 }).await?;
    
    println!("✓ Sent Heartbeat message");
    
    // Receive response
    match transport.receive().await {
        Ok(msg) => {
            println!("✓ Received response: {:?}", msg);
        }
        Err(e) => {
            error!("Failed to receive response: {}", e);
        }
    }
    
    transport.close().await?;
    
    println!("✓ Transport test completed");
    
    Ok(())
}
