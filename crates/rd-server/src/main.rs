mod config;
mod state;
mod handlers;

use clap::Parser;
use tracing::{info, error};
use tracing_subscriber;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "rd-server")]
#[command(version = "0.1.0")]
#[command(about = "Remote Desktop Server - signaling and relay server", long_about = None)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "config/server.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _cli = Cli::parse();
    // Install default crypto provider for rustls
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    
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
