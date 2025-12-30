use thiserror::Error;
use super::models::{SessionId, PeerId};

// ============================================================================
// Domain Errors
// ============================================================================

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),
    
    #[error("Peer not found: {0}")]
    PeerNotFound(PeerId),
    
    #[error("Unauthorized access")]
    Unauthorized,
    
    #[error("Invalid session state: {0}")]
    InvalidState(String),
    
    #[error("Session already exists: {0}")]
    SessionAlreadyExists(SessionId),
}

// ============================================================================
// Infrastructure Errors
// ============================================================================

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("Display not found: {0}")]
    DisplayNotFound(u32),
    
    #[error("Screen capture failed: {0}")]
    CaptureFailed(String),
    
    #[error("Unsupported platform")]
    UnsupportedPlatform,
    
    #[error("Permission denied")]
    PermissionDenied,
    
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
}

#[derive(Debug, Error)]
pub enum InjectionError {
    #[error("Input injection failed: {0}")]
    InjectionFailed(String),
    
    #[error("Unsupported platform")]
    UnsupportedPlatform,
    
    #[error("Permission denied")]
    PermissionDenied,
    
    #[error("Invalid input event: {0}")]
    InvalidEvent(String),
    
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Unsupported event type")]
    UnsupportedEvent,
}

#[derive(Debug, Error)]
pub enum CodecError {
    #[error("Encoding failed: {0}")]
    EncodingFailed(String),
    
    #[error("Decoding failed: {0}")]
    DecodingFailed(String),
    
    #[error("Unsupported codec: {0:?}")]
    UnsupportedCodec(super::models::CodecType),
    
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    
    #[error("Codec error: {0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Connection timeout")]
    Timeout,
    
    #[error("Connection closed")]
    Closed,
    
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid token")]
    InvalidToken,
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Token generation failed: {0}")]
    TokenGenerationFailed(String),
}

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Not found")]
    NotFound,
    
    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),
}

// ============================================================================
// Application Errors
// ============================================================================

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),
    
    #[error("Capture error: {0}")]
    Capture(#[from] CaptureError),
    
    #[error("Injection error: {0}")]
    Injection(#[from] InjectionError),
    
    #[error("Codec error: {0}")]
    Codec(#[from] CodecError),
    
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),
    
    #[error("Auth error: {0}")]
    Auth(#[from] AuthError),
    
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

// Convenience type alias
pub type Result<T> = std::result::Result<T, ApplicationError>;
