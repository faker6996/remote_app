mod config;
mod capture_loop;
mod input_handler;

use clap::Parser;
use rd_core::domain::ports::Transport;
use tracing::{info, error};
use tracing_subscriber;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "rd-agent")]
#[command(version = "0.1.0")]
#[command(about = "Remote Desktop Agent - host-side screen capture and control", long_about = None)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "config/agent.toml")]
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
    
    info!("Starting Remote Desktop Agent");
    
    // Load configuration
    let config = config::AgentConfig::load()?;
    info!("Agent config: device_id={}, server={}", 
        config.device_id, config.server_url);
    
    // Connect to server
    info!("Connecting to server: {}", config.server_url);
    let client = rd_transport::quic::QuicClient::new()?;
    let connection = client.connect(config.server_url.parse()?).await?;
    
    let mut transport = rd_transport::QuicTransport::new(connection).await?;
    
    // Send Hello
    let platform = if cfg!(target_os = "windows") {
        rd_core::domain::models::Platform::Windows
    } else if cfg!(target_os = "linux") {
        rd_core::domain::models::Platform::Linux
    } else if cfg!(target_os = "macos") {
        rd_core::domain::models::Platform::MacOS
    } else {
        rd_core::domain::models::Platform::Linux
    };
    
    transport.send(rd_core::domain::ports::ProtocolMessage::Hello {
        version: 1,
        device_id: config.device_id.clone(),
        platform,
    }).await?;
    
    info!("Connected to server");
    
    // Create screen capture and encoder
    let screen_capture = rd_platform::create_screen_capture()?;
    let encoder = std::sync::Arc::new(tokio::sync::Mutex::new(
        rd_codec::JpegEncoder::with_quality(config.encoder_quality)
    ));
    
    // Create input injector
    let input_injector = rd_platform::create_input_injector()?;
    
    // Start capture loop
    let transport_clone = std::sync::Arc::new(tokio::sync::Mutex::new(transport));
    let capture_handle = tokio::spawn(capture_loop::run_capture_loop(
        screen_capture,
        encoder,
        transport_clone.clone(),
        config.max_fps,
    ));
    
    // Start input handler
    let input_handle = tokio::spawn(input_handler::run_input_handler(
        input_injector,
        transport_clone,
    ));
    
    // Wait for tasks
    tokio::select! {
        result = capture_handle => {
            if let Err(e) = result {
                error!("Capture loop failed: {}", e);
            }
        }
        result = input_handle => {
            if let Err(e) = result {
                error!("Input handler failed: {}", e);
            }
        }
    }
    
    Ok(())
}
