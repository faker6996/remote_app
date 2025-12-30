mod commands;

use clap::{Parser, Subcommand};
use tracing_subscriber;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "rd-cli")]
#[command(about = "Remote Desktop CLI Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available agents
    List {
        /// Server address
        #[arg(short, long, default_value = "127.0.0.1:4433")]
        server: String,
    },
    
    /// Connect to an agent
    Connect {
        /// Agent device ID
        agent_id: String,
        
        /// Server address
        #[arg(short, long, default_value = "127.0.0.1:4433")]
        server: String,
        
        /// Number of frames to receive before disconnecting
        #[arg(short, long, default_value = "100")]
        frames: usize,
    },
    
    /// Debug transport
    Debug {
        /// Server address
        #[arg(short, long, default_value = "127.0.0.1:4433")]
        server: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install default crypto provider for rustls
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::List { server } => {
            commands::list_agents(&server).await?;
        }
        Commands::Connect { agent_id, server, frames } => {
            commands::connect_to_agent(&agent_id, &server, frames).await?;
        }
        Commands::Debug { server } => {
            commands::debug_transport(&server).await?;
        }
    }
    
    Ok(())
}
