use async_trait::async_trait;
use super::models::*;
use super::error::*;

// ============================================================================
// Screen Capture Port
// ============================================================================

/// Trait for capturing screen content
#[async_trait]
pub trait ScreenCapture: Send + Sync {
    /// Capture a single frame from the screen
    async fn capture(&mut self) -> std::result::Result<ScreenFrame, CaptureError>;
    
    /// Get information about available displays
    async fn get_displays(&self) -> std::result::Result<Vec<DisplayInfo>, CaptureError>;
    
    /// Set the target display to capture (if multiple displays exist)
    async fn set_target_display(&mut self, display_id: u32) -> std::result::Result<(), CaptureError>;
}

// ============================================================================
// Input Injection Port
// ============================================================================

/// Trait for injecting input events into the OS
#[async_trait]
pub trait InputInjector: Send + Sync {
    /// Inject a single input event
    async fn inject(&mut self, event: InputEvent) -> std::result::Result<(), InjectionError>;
    
    /// Inject multiple input events in sequence
    async fn inject_batch(&mut self, events: Vec<InputEvent>) -> std::result::Result<(), InjectionError> {
        for event in events {
            self.inject(event).await?;
        }
        Ok(())
    }
}

// ============================================================================
// Encoder/Decoder Ports
// ============================================================================

/// Trait for encoding screen frames
#[async_trait]
pub trait Encoder: Send + Sync {
    /// Encode a screen frame
    async fn encode(&mut self, frame: &ScreenFrame) -> std::result::Result<Vec<u8>, CodecError>;
    
    /// Get encoder configuration
    fn config(&self) -> &EncoderConfig;
    
    /// Update encoder configuration
    fn set_config(&mut self, config: EncoderConfig) -> std::result::Result<(), CodecError>;
}

/// Trait for decoding screen frames
#[async_trait]
pub trait Decoder: Send + Sync {
    /// Decode encoded data back to a screen frame
    async fn decode(&mut self, data: &[u8]) -> std::result::Result<ScreenFrame, CodecError>;
    
    /// Get the codec type this decoder handles
    fn codec_type(&self) -> CodecType;
}

// ============================================================================
// Transport Port
// ============================================================================

/// Protocol message wrapper
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ProtocolMessage {
    // Handshake & Auth
    Hello {
        version: u32,
        device_id: String,
        platform: Platform,
    },
    Auth {
        token: AuthToken,
    },
    AuthResponse {
        success: bool,
        session_id: Option<SessionId>,
        error: Option<String>,
    },
    
    // Session Management
    SessionRequest {
        target_device: String,
    },
    SessionCreated {
        session_id: SessionId,
        endpoint: String,
    },
    SessionEnd {
        session_id: SessionId,
        reason: String,
    },
    
    // Streaming
    ScreenFrame {
        sequence: u64,
        timestamp: u64,
        data: Vec<u8>,
        width: u32,
        height: u32,
        format: FrameFormat,
    },
    
    // Input
    InputEvent {
        timestamp: u64,
        event: InputEvent,
    },
    
    // Control
    Heartbeat {
        timestamp: u64,
    },
    Error {
        code: u32,
        message: String,
    },
    Disconnect,
}

/// Trait for network transport
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a protocol message
    async fn send(&mut self, message: ProtocolMessage) -> std::result::Result<(), TransportError>;
    
    /// Receive a protocol message
    async fn receive(&mut self) -> std::result::Result<ProtocolMessage, TransportError>;
    
    /// Close the transport connection
    async fn close(&mut self) -> std::result::Result<(), TransportError>;
    
    /// Check if the transport is still connected
    fn is_connected(&self) -> bool;
}

// ============================================================================
// Authentication Port
// ============================================================================

/// Trait for authentication
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Authenticate a token and return the associated peer ID
    async fn authenticate(&self, token: &AuthToken) -> std::result::Result<PeerId, AuthError>;
    
    /// Generate a new token for a device
    async fn generate_token(&self, device_id: &str) -> std::result::Result<AuthToken, AuthError>;
    
    /// Revoke a token
    async fn revoke_token(&self, token: &AuthToken) -> std::result::Result<(), AuthError>;
}

// ============================================================================
// Session Management Port
// ============================================================================

/// Trait for session repository
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Create a new session
    async fn create(&mut self, session: Session) -> std::result::Result<(), RepositoryError>;
    
    /// Find a session by ID
    async fn find_by_id(&self, id: SessionId) -> std::result::Result<Option<Session>, RepositoryError>;
    
    /// Update a session
    async fn update(&mut self, session: Session) -> std::result::Result<(), RepositoryError>;
    
    /// Delete a session
    async fn delete(&mut self, id: SessionId) -> std::result::Result<(), RepositoryError>;
    
    /// List all active sessions
    async fn list_active(&self) -> std::result::Result<Vec<Session>, RepositoryError>;
}
