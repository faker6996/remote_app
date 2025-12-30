mod config;
mod state;
mod handlers;

use tracing::{info, error};
use tracing_subscriber;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Starting Remote Desktop Server");
    
    // Load configuration
    let config = config::ServerConfig::load()?;
    info!("Server will bind to: {}", config.bind_address);
    
    // Create server state
    let state = state::ServerState::new();
    
    // Start QUIC server
    let server = rd_transport::quic::QuicServer::new(config.bind_address.parse()?)?;
    
    info!("Server listening on {}", config.bind_address);
    
    // Accept connections loop
    loop {
        if let Some(connection) = server.accept().await {
            let state = state.clone();
            
            tokio::spawn(async move {
                if let Err(e) = handlers::handle_connection(connection, state).await {
                    error!("Connection handler error: {}", e);
                }
            });
        }
    }
}
