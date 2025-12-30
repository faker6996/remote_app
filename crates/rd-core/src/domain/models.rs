use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ============================================================================
// Identifiers
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(pub String);

impl PeerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Session
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub client: PeerId,
    pub agent: PeerId,
    pub created_at: DateTime<Utc>,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Pending,
    Active,
    Paused,
    Closed,
}

// ============================================================================
// Peer & Device Info
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: PeerId,
    pub device_id: String,
    pub display_name: String,
    pub platform: Platform,
    pub capabilities: Capabilities,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Windows,
    Linux,
    MacOS,
    Android,
    IOS,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub screen_capture: bool,
    pub input_injection: bool,
    pub audio_capture: bool,
    pub file_transfer: bool,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            screen_capture: true,
            input_injection: true,
            audio_capture: false,
            file_transfer: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub is_primary: bool,
}

// ============================================================================
// Screen Frame
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenFrame {
    pub sequence: u64,
    pub timestamp: u64,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: FrameFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameFormat {
    Raw,          // RGBA/BGRA raw pixels
    Jpeg,         // JPEG compressed
    H264,         // H.264 encoded
    VP8,          // VP8 encoded
    AV1,          // AV1 encoded
}

// ============================================================================
// Input Events
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEventData {
    pub timestamp: u64,
    pub event: InputEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseButton { button: MouseButton, pressed: bool },
    MouseScroll { delta_x: i32, delta_y: i32 },
    KeyPress { key: KeyCode, pressed: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyCode(pub u32);

impl KeyCode {
    // Common key codes (platform-agnostic, will be mapped to OS-specific)
    pub const ESCAPE: Self = Self(0x01);
    pub const ENTER: Self = Self(0x1C);
    pub const SPACE: Self = Self(0x39);
    pub const BACKSPACE: Self = Self(0x0E);
    pub const TAB: Self = Self(0x0F);
    
    // Letters
    pub const A: Self = Self(0x1E);
    pub const B: Self = Self(0x30);
    pub const C: Self = Self(0x2E);
    // ... (add more as needed)
}

// ============================================================================
// Codec Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderConfig {
    pub codec: CodecType,
    pub quality: u8,          // 0-100
    pub target_fps: u8,       // Max FPS
    pub bitrate: Option<u32>, // For H.264/VP8
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            codec: CodecType::Jpeg,
            quality: 80,
            target_fps: 30,
            bitrate: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodecType {
    Jpeg,
    H264,
    VP8,
    AV1,
}

// ============================================================================
// Auth
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub device_id: String,
}

impl AuthToken {
    pub fn new(token: impl Into<String>, device_id: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            device_id: device_id.into(),
        }
    }
}

// Helper module for serde_bytes compatibility
mod serde_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<u8>::deserialize(deserializer)
    }
}
