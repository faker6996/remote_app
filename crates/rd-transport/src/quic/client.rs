use quinn::{Endpoint, ClientConfig, Connection};
use std::sync::Arc;
use std::net::SocketAddr;
use tracing::{info, error};

/// QUIC client for connecting to remote endpoints
pub struct QuicClient {
    endpoint: Endpoint,
}

impl QuicClient {
    /// Create a new QUIC client
    pub fn new() -> anyhow::Result<Self> {
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        
        // Configure client with default settings
        let client_config = configure_client();
        endpoint.set_default_client_config(client_config);
        
        Ok(Self { endpoint })
    }
    
    /// Connect to a remote server
    pub async fn connect(&self, server_addr: SocketAddr) -> anyhow::Result<Connection> {
        info!("Connecting to {}", server_addr);
        
        let connection = self.endpoint
            .connect(server_addr, "localhost")?
            .await?;
        
        info!("Connected to {}", server_addr);
        
        Ok(connection)
    }
}

impl Default for QuicClient {
    fn default() -> Self {
        Self::new().expect("Failed to create QUIC client")
    }
}

/// Configure the QUIC client with TLS settings
fn configure_client() -> ClientConfig {
    let mut crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_no_client_auth();
    
    // Must match server's ALPN protocol
    crypto.alpn_protocols = vec![b"rdp/1".to_vec()];
    
    ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto).unwrap()
    ))
}

/// Skip server certificate verification (for development)
/// TODO: Replace with proper certificate validation in production
#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
