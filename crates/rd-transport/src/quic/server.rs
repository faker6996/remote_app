use quinn::{Endpoint, ServerConfig, Connection};
use std::sync::Arc;
use std::net::SocketAddr;
use tracing::{info, error};

/// QUIC server for accepting incoming connections
pub struct QuicServer {
    endpoint: Endpoint,
}

impl QuicServer {
    /// Create a new QUIC server bound to the given address
    pub fn new(bind_addr: SocketAddr) -> anyhow::Result<Self> {
        let server_config = configure_server()?;
        
        let endpoint = Endpoint::server(server_config, bind_addr)?;
        info!("QUIC server listening on {}", bind_addr);
        
        Ok(Self { endpoint })
    }
    
    /// Accept incoming connections
    pub async fn accept(&self) -> Option<Connection> {
        match self.endpoint.accept().await {
            Some(incoming) => {
                match incoming.await {
                    Ok(conn) => {
                        info!("Accepted connection from {}", conn.remote_address());
                        Some(conn)
                    }
                    Err(e) => {
                        error!("Connection failed: {}", e);
                        None
                    }
                }
            }
            None => None,
        }
    }
}

/// Configure the QUIC server with TLS settings
fn configure_server() -> anyhow::Result<ServerConfig> {
    // Generate self-signed certificate (for development)
    // TODO: Use proper certificates in production
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert);
    let priv_key = rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());
    
    let mut server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], priv_key.into())?;
    
    server_crypto.alpn_protocols = vec![b"rdp/1".to_vec()];
    
    let mut server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)?
    ));
    
    // Configure transport
    let mut transport_config = quinn::TransportConfig::default();
    transport_config.max_concurrent_uni_streams(0_u8.into());
    server_config.transport_config(Arc::new(transport_config));
    
    Ok(server_config)
}

// Add rcgen dependency
#[cfg(not(target_family = "wasm"))]
use rcgen;
