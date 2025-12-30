# Remote Desktop Application - Architecture Documentation

**Version:** 1.0  
**Date:** December 30, 2025  
**Status:** Design Phase

---

## 1. EXECUTIVE SUMMARY

Đây là một ứng dụng remote desktop đa nền tảng, cho phép:

- Xem màn hình máy tính từ xa theo thời gian thực (low latency)
- Điều khiển chuột và bàn phím từ xa
- Hỗ trợ Windows, Linux (Ubuntu), macOS

**Tech Stack:**

- **Core:** Rust (1.80+) với Clean Architecture/Hexagonal pattern
- **Transport:** QUIC (quinn) với TLS encryption
- **Desktop UI:** Tauri v2 (Rust backend + React frontend)
- **Async Runtime:** Tokio
- **Platforms:** Windows (DXGI capture), Linux (X11/Wayland)

---

## 2. SYSTEM ARCHITECTURE

### 2.1. High-Level Components

```
┌─────────────────────────────────────────────────────────────────┐
│                     REMOTE DESKTOP SYSTEM                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐         ┌──────────────┐      ┌────────────┐ │
│  │   Desktop    │◄───────►│   Server     │◄────►│   Agent    │ │
│  │   Client     │         │  (Signaling  │      │  (Host)    │ │
│  │   (Tauri)    │         │   + Relay)   │      │            │ │
│  └──────────────┘         └──────────────┘      └────────────┘ │
│        │                                               │         │
│        │  QUIC (Direct or via Relay)                  │         │
│        └───────────────────────────────────────────────┘         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2. Component Responsibilities

| Component          | Type           | Responsibility                                              |
| ------------------ | -------------- | ----------------------------------------------------------- |
| **Desktop Client** | Tauri App      | UI for viewing screen, sending input, session management    |
| **Server**         | Binary         | Signaling, session broker, relay coordinator                |
| **Agent**          | Binary/Service | Screen capture, input injection, runs on controlled machine |
| **Core Libraries** | Rust Crates    | Domain logic, codecs, transport, platform abstractions      |

---

## 3. MONOREPO STRUCTURE

```
remote-desktop/
├── Cargo.toml                    # Rust workspace manifest
├── .gitignore
├── README.md
├── LICENSE
│
├── crates/                       # Rust core libraries & binaries
│   ├── rd-core/                  # ⚡ Domain & Application Layer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── domain/           # Pure domain models & ports
│   │       │   ├── mod.rs
│   │       │   ├── models.rs     # Session, Peer, Frame, Event, etc.
│   │       │   ├── ports.rs      # Trait definitions (interfaces)
│   │       │   └── error.rs      # Domain errors
│   │       └── application/      # Use cases & orchestration
│   │           ├── mod.rs
│   │           ├── session_manager.rs
│   │           ├── stream_controller.rs
│   │           └── auth_service.rs
│   │
│   ├── rd-codec/                 # 🎬 Encoding/Decoding
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs         # Encoder/Decoder traits
│   │       ├── jpeg.rs           # JPEG codec (V1)
│   │       └── h264.rs           # H.264 (future/stub)
│   │
│   ├── rd-transport/             # 🌐 Network Transport
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs         # Transport trait
│   │       ├── quic/
│   │       │   ├── mod.rs
│   │       │   ├── client.rs     # QUIC client (quinn)
│   │       │   ├── server.rs     # QUIC server
│   │       │   └── config.rs     # TLS config
│   │       ├── protocol/         # Protocol messages
│   │       │   ├── mod.rs
│   │       │   ├── messages.rs   # Frame, Input, Auth, etc.
│   │       │   └── codec.rs      # Serialization (bincode/protobuf)
│   │       └── relay/
│   │           └── mod.rs        # Simple relay logic
│   │
│   ├── rd-platform/              # 🖥️ OS-Specific Implementations
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── screen_capture/
│   │       │   ├── mod.rs
│   │       │   ├── windows.rs    # DXGI Desktop Duplication
│   │       │   ├── linux.rs      # X11 (x11rb) / Wayland stub
│   │       │   └── macos.rs      # CoreGraphics (future)
│   │       └── input_injection/
│   │           ├── mod.rs
│   │           ├── windows.rs    # WinAPI SendInput
│   │           ├── linux.rs      # XTest / uinput
│   │           └── macos.rs      # CGEvent (future)
│   │
│   ├── rd-server/                # 🖧 Signaling & Relay Server
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── config.rs
│   │       ├── api/              # API layer (REST future)
│   │       │   └── mod.rs
│   │       ├── handlers/         # Connection handlers
│   │       │   ├── mod.rs
│   │       │   ├── registration.rs
│   │       │   └── session.rs
│   │       └── state/            # Server state
│   │           └── mod.rs
│   │
│   ├── rd-agent/                 # 🤖 Agent (Host-side Service)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── config.rs         # Config file loading
│   │       ├── capture_loop.rs   # Screen capture thread
│   │       ├── input_handler.rs  # Input event handler
│   │       └── network.rs        # QUIC client logic
│   │
│   ├── rd-client/                # 📱 Client Library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── session.rs        # Session management
│   │       ├── renderer.rs       # Frame decoding abstraction
│   │       └── input.rs          # Input event builder
│   │
│   └── rd-cli/                   # 🔧 CLI Tool
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           └── commands/
│               ├── mod.rs
│               ├── list.rs       # List agents
│               ├── connect.rs    # Connect to agent
│               └── debug.rs      # Debug transport
│
├── desktop/                      # 🖼️ Desktop UI (Tauri)
│   ├── src-tauri/                # Rust backend
│   │   ├── Cargo.toml            # Depends on: rd-client, rd-core
│   │   ├── tauri.conf.json
│   │   ├── build.rs
│   │   ├── icons/
│   │   └── src/
│   │       ├── main.rs           # Tauri app setup
│   │       ├── commands.rs       # Tauri commands
│   │       ├── state.rs          # App state
│   │       └── lib.rs
│   │
│   ├── src/                      # Frontend (React + TypeScript)
│   │   ├── main.tsx
│   │   ├── App.tsx
│   │   ├── components/
│   │   │   ├── ScreenViewer.tsx
│   │   │   ├── InputController.tsx
│   │   │   ├── AgentList.tsx
│   │   │   └── SessionPanel.tsx
│   │   ├── hooks/
│   │   │   └── useRemoteSession.ts
│   │   ├── api/
│   │   │   └── tauri.ts          # Tauri IPC wrapper
│   │   └── styles/
│   │       └── app.css
│   │
│   ├── package.json
│   ├── tsconfig.json
│   └── vite.config.ts
│
├── docs/                         # Documentation
│   ├── architecture.md           # This file
│   ├── protocol.md               # Protocol specification
│   ├── api.md                    # API reference
│   ├── deployment.md             # Deployment guide
│   └── development.md            # Development setup
│
├── examples/                     # Code examples
│   ├── mock_capture.rs
│   ├── mock_transport.rs
│   └── simple_client.rs
│
└── scripts/                      # Build & dev scripts
    ├── build-all.sh
    ├── dev.sh
    └── release.sh
```

---

## 4. CLEAN ARCHITECTURE LAYERS

### 4.1. Layer Separation

```
┌─────────────────────────────────────────────────────────┐
│                    INTERFACE LAYER                       │
│         (Tauri UI, CLI, FFI - Future Mobile)            │
├─────────────────────────────────────────────────────────┤
│                   APPLICATION LAYER                      │
│      (Use Cases: SessionManager, StreamController)      │
├─────────────────────────────────────────────────────────┤
│                     DOMAIN LAYER                         │
│        (Models, Ports/Traits, Business Rules)           │
├─────────────────────────────────────────────────────────┤
│                 INFRASTRUCTURE LAYER                     │
│   (Platform, Transport, Codec - Implement Ports)        │
└─────────────────────────────────────────────────────────┘
```

### 4.2. Dependency Rules

- **Domain**: No dependencies (pure models + traits)
- **Application**: Depends ONLY on Domain
- **Infrastructure**: Implements Domain ports
- **Interface**: Depends on Application + Infrastructure (composition)

---

## 5. DOMAIN MODEL

### 5.1. Core Domain Entities

```rust
// Session
pub struct Session {
    pub id: SessionId,
    pub client: PeerId,
    pub agent: PeerId,
    pub created_at: Timestamp,
    pub status: SessionStatus,
}

// Screen Frame
pub struct ScreenFrame {
    pub sequence: u64,
    pub timestamp: u64,
    pub data: Vec<u8>,         // Raw or encoded
    pub width: u32,
    pub height: u32,
    pub format: FrameFormat,
}

// Input Event
pub enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseButton { button: MouseButton, pressed: bool },
    MouseScroll { delta_x: i32, delta_y: i32 },
    KeyPress { key: KeyCode, pressed: bool },
}

// Peer
pub struct Peer {
    pub id: PeerId,
    pub device_id: String,
    pub display_name: String,
    pub platform: Platform,
    pub capabilities: Capabilities,
}
```

### 5.2. Domain Ports (Traits)

```rust
// Screen Capture
#[async_trait]
pub trait ScreenCapture: Send + Sync {
    async fn capture(&mut self) -> Result<ScreenFrame, CaptureError>;
    async fn get_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError>;
}

// Input Injection
#[async_trait]
pub trait InputInjector: Send + Sync {
    async fn inject(&mut self, event: InputEvent) -> Result<(), InjectionError>;
}

// Codec
#[async_trait]
pub trait Encoder: Send + Sync {
    async fn encode(&mut self, frame: &ScreenFrame) -> Result<Vec<u8>, CodecError>;
}

#[async_trait]
pub trait Decoder: Send + Sync {
    async fn decode(&mut self, data: &[u8]) -> Result<ScreenFrame, CodecError>;
}

// Transport
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&mut self, msg: ProtocolMessage) -> Result<(), TransportError>;
    async fn receive(&mut self) -> Result<ProtocolMessage, TransportError>;
    async fn close(&mut self) -> Result<(), TransportError>;
}
```

---

## 6. PROTOCOL DESIGN

### 6.1. Protocol Messages

```rust
pub enum ProtocolMessage {
    // ===== Handshake & Auth =====
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

    // ===== Session Management =====
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

    // ===== Streaming =====
    ScreenFrame {
        sequence: u64,
        timestamp: u64,
        data: Vec<u8>,
        width: u32,
        height: u32,
        format: FrameFormat,
    },

    // ===== Input Control =====
    InputEvent {
        timestamp: u64,
        event: InputEventData,
    },

    // ===== Control & Health =====
    Heartbeat {
        timestamp: u64,
    },
    Error {
        code: u32,
        message: String,
    },
    Disconnect,
}
```

### 6.2. Serialization

**Choice:** `bincode` (serde-based)

**Reasons:**

- Simple & fast
- Zero-copy deserialization
- Small payload size
- Type-safe with Rust

**Future:** Can switch to Protobuf if need cross-language compatibility

---

## 7. SEQUENCE FLOWS

### 7.1. Session Creation Flow

```
┌─────────┐         ┌─────────┐         ┌─────────┐
│ Desktop │         │ Server  │         │  Agent  │
│ Client  │         │         │         │         │
└────┬────┘         └────┬────┘         └────┬────┘
     │                   │                    │
     │  1. List Agents   │                    │
     ├──────────────────>│                    │
     │<──────────────────┤                    │
     │   Agent List      │                    │
     │                   │                    │
     │  2. Create Session│                    │
     │   (target agent)  │                    │
     ├──────────────────>│                    │
     │                   │  3. Notify Agent   │
     │                   ├───────────────────>│
     │                   │                    │
     │                   │<───────────────────┤
     │<──────────────────┤   4. Agent ACK     │
     │  Session Created  │                    │
     │  (session_id, ep) │                    │
     │                   │                    │
     │  5. QUIC Connect (Direct or Relay)    │
     ├───────────────────────────────────────>│
     │<───────────────────────────────────────┤
     │         QUIC Handshake + TLS           │
     │                   │                    │
     │  6. Start Stream  │                    │
     ├──────────────────────────────────────> │
     │<──────────────────────────────────────┤│
     │         Screen Frames Flow             │
     │                                         │
```

### 7.2. Screen Streaming + Input Flow

```
┌──────────────┐                              ┌──────────────┐
│ Agent        │                              │ Desktop      │
│              │                              │ Client       │
└──────┬───────┘                              └──────┬───────┘
       │                                             │
       │ ┌─────────────────────────┐                │
       │ │ Capture Loop (Thread 1) │                │
       │ └─────────────────────────┘                │
       │                                             │
   ┌───▼────┐                                       │
   │Capture │                                       │
   │Screen  │                                       │
   └───┬────┘                                       │
       │                                             │
   ┌───▼────┐                                       │
   │ Encode │                                       │
   │ (JPEG) │                                       │
   └───┬────┘                                       │
       │                                             │
       │ ┌─────────────────────────┐                │
       │ │ Network Thread          │                │
       │ └─────────────────────────┘                │
       │                                             │
   ┌───▼────────┐                              ┌────▼──────┐
   │ Send Frame ├─────────QUIC Stream─────────>│  Receive  │
   └────────────┘                              └─────┬─────┘
       │                                             │
       │                                        ┌────▼─────┐
       │                                        │  Decode  │
       │                                        └─────┬────┘
       │                                             │
       │                                        ┌────▼─────┐
       │                                        │  Render  │
       │                                        └──────────┘
       │                                             │
       │                                        ┌────▼─────┐
       │                                        │User Input│
       │                                        └─────┬────┘
       │                                             │
   ┌───▼────────┐                              ┌────▼──────┐
   │  Receive   │<────────QUIC Stream──────────┤Send Input │
   └─────┬──────┘                              └───────────┘
       │                                             │
   ┌───▼────┐
   │ Inject │
   │ to OS  │
   └────────┘
```

---

## 8. TAURI INTEGRATION

### 8.1. Architecture

```
┌──────────────────────────────────────────────────────┐
│                  Tauri Desktop App                    │
├──────────────────────────────────────────────────────┤
│                                                       │
│  ┌─────────────────┐         ┌──────────────────┐   │
│  │   Frontend      │         │   Backend        │   │
│  │   (React)       │◄───IPC──►│   (Rust)        │   │
│  │                 │         │                  │   │
│  │ - ScreenViewer  │         │ - rd-client      │   │
│  │ - InputPanel    │         │ - Session mgmt   │   │
│  │ - AgentList     │         │ - State handler  │   │
│  └─────────────────┘         └──────────────────┘   │
│                                       │              │
└───────────────────────────────────────┼──────────────┘
                                        │
                                        ▼
                            ┌───────────────────────┐
                            │  Rust Core Libraries  │
                            │  (rd-client, etc.)    │
                            └───────────────────────┘
```

### 8.2. Tauri Commands (IPC)

```rust
// src-tauri/src/commands.rs

#[tauri::command]
async fn list_agents(state: State<'_, AppState>) -> Result<Vec<AgentInfo>, String> {
    // Call rd-client to fetch agent list
}

#[tauri::command]
async fn connect_to_agent(
    agent_id: String,
    state: State<'_, AppState>
) -> Result<SessionInfo, String> {
    // Create session via rd-client
}

#[tauri::command]
async fn send_input_event(
    session_id: String,
    event: InputEvent,
    state: State<'_, AppState>
) -> Result<(), String> {
    // Send input to agent
}

#[tauri::command]
async fn disconnect_session(
    session_id: String,
    state: State<'_, AppState>
) -> Result<(), String> {
    // Close session
}
```

### 8.3. Frontend Components

```typescript
// src/components/ScreenViewer.tsx
export function ScreenViewer({ sessionId }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Listen for frame updates via Tauri event
  useEffect(() => {
    const unlisten = listen("screen-frame", (event) => {
      renderFrame(event.payload);
    });
    return () => unlisten();
  }, []);

  // Send input events
  const handleMouseMove = (e: MouseEvent) => {
    invoke("send_input_event", {
      sessionId,
      event: { MouseMove: { x: e.clientX, y: e.clientY } },
    });
  };

  return <canvas ref={canvasRef} onMouseMove={handleMouseMove} />;
}
```

---

## 9. TRANSPORT LAYER

### 9.1. QUIC with quinn

**Features:**

- TLS 1.3 built-in (encryption by default)
- Multiplexed streams (control + data)
- 0-RTT reconnection
- NAT-friendly UDP

**Configuration:**

```rust
// Client
let mut client_config = ClientConfig::with_native_roots();
client_config.alpn_protocols = vec![b"rdp/1".to_vec()];

// Server
let server_config = ServerConfig::with_crypto(Arc::new(crypto_config));
```

### 9.2. NAT Traversal Strategy

**V1 (Simple):**

- Direct connection nếu LAN hoặc có port forwarding
- Relay server nếu không thể kết nối trực tiếp

**Future (Advanced):**

- STUN for reflexive address discovery
- TURN for full relay
- ICE-like candidate gathering

---

## 10. PLATFORM-SPECIFIC IMPLEMENTATIONS

### 10.1. Windows

**Screen Capture:**

- DXGI Desktop Duplication API
- Benefits: GPU-accelerated, efficient dirty region tracking
- Crate: `windows` (Microsoft official bindings)

**Input Injection:**

- WinAPI `SendInput`
- Virtual key codes mapping

### 10.2. Linux (Ubuntu)

**Screen Capture:**

- X11: `x11rb` crate (XGetImage)
- Wayland: PipeWire desktop capture (future)

**Input Injection:**

- XTest extension (X11)
- uinput (Wayland)

### 10.3. macOS (Future)

**Screen Capture:**

- CGDisplayCreateImage (CoreGraphics)
- ScreenCaptureKit (macOS 12.3+)

**Input Injection:**

- CGEvent

---

## 11. PERFORMANCE CONSIDERATIONS

### 11.1. Low Latency Pipeline

```
Capture (10-60ms) → Encode (5-20ms) → Network (10-100ms) → Decode (5-15ms) → Render (16ms)
Total Target: < 200ms for good UX
```

**Optimizations:**

- Use frame dropping when client can't keep up
- Adaptive quality based on network conditions
- Region-based capture (only changed areas) - Future

### 11.2. Backpressure Handling

```rust
// Bounded channel between capture and network threads
let (tx, rx) = mpsc::channel(2); // Only buffer 2 frames
                                  // Drop old if full
```

### 11.3. Resource Configuration

```toml
# agent.toml
[capture]
max_fps = 30
resolution = "1920x1080"
region = "full"  # or "primary_monitor"

[encoder]
codec = "jpeg"
quality = 80
```

---

## 12. SECURITY

### 12.1. Authentication (V1)

- Pre-shared token (device token)
- Token stored securely (OS keychain)
- Token sent during handshake

### 12.2. Encryption

- QUIC = TLS 1.3 by default
- All traffic encrypted end-to-end

### 12.3. Future Enhancements

- User accounts with password/OAuth
- Session-specific one-time tokens
- Permission system (view-only vs control)
- Audit logging

---

## 13. ERROR HANDLING

### 13.1. Error Types

```rust
// Domain errors
pub enum DomainError {
    SessionNotFound(SessionId),
    UnauthorizedAccess,
    InvalidState,
}

// Infrastructure errors
pub enum CaptureError {
    DisplayNotFound,
    CaptureFailed(String),
    UnsupportedPlatform,
}

pub enum TransportError {
    ConnectionFailed,
    Timeout,
    ProtocolError,
}
```

### 13.2. Error Propagation

- Libraries: Use `thiserror` for typed errors
- Binaries: Use `anyhow` for context propagation
- Tauri commands: Convert to `String` error for IPC

---

## 14. TESTING STRATEGY

### 14.1. Unit Tests

- Domain models (pure logic)
- Protocol message serialization
- Mock implementations of ports

### 14.2. Integration Tests

```rust
// Test with mock transport
#[tokio::test]
async fn test_session_creation() {
    let mock_transport = MockTransport::new();
    let session_mgr = SessionManager::new(mock_transport);
    let session = session_mgr.create_session("agent-1").await.unwrap();
    assert_eq!(session.agent, "agent-1");
}
```

### 14.3. End-to-End Tests

- Start local server + agent + client
- Verify frame transmission
- Verify input injection

---

## 15. BUILD & DEPLOYMENT

### 15.1. Development Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Node.js (for Tauri frontend)
nvm install 20

# Install Tauri CLI
cargo install tauri-cli

# Clone and build
git clone <repo>
cd remote-desktop
cargo build
cd desktop && npm install && npm run tauri dev
```

### 15.2. Release Build

```bash
# Build all Rust binaries
cargo build --release

# Build Tauri app
cd desktop && npm run tauri build

# Outputs:
# - desktop/src-tauri/target/release/bundle/ (installers)
# - target/release/rd-server, rd-agent, rd-cli (binaries)
```

### 15.3. Deployment

**Agent:**

- Windows: Install as service (via installer)
- Linux: Systemd service

**Server:**

- Docker container
- Kubernetes deployment (future)

**Desktop Client:**

- Installers: .exe (Windows), .deb/.rpm (Linux), .dmg (macOS)

---

## 16. ROADMAP

### Phase 1: Foundation (Current)

- ✅ Architecture design
- 🚧 Core libraries scaffold
- 🚧 Basic QUIC transport
- 🚧 Tauri UI skeleton

### Phase 2: MVP (1-2 months)

- Windows screen capture + input injection
- JPEG codec
- Desktop client with basic UI
- Local network testing

### Phase 3: Cross-Platform (2-3 months)

- Linux (X11) support
- macOS support (if needed)
- H.264 codec for better performance

### Phase 4: Production (3-6 months)

- Relay server with NAT traversal
- User accounts & auth
- Performance optimizations
- Mobile client (Tauri mobile or Flutter)

### Phase 5: Advanced (6+ months)

- WebRTC transport option
- Multi-monitor support
- File transfer
- Clipboard sync
- Audio streaming

---

## 17. REFERENCES

### Technology Documentation

- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Async Runtime](https://tokio.rs/)
- [Quinn QUIC](https://github.com/quinn-rs/quinn)
- [Tauri Framework](https://tauri.app/)
- [React Documentation](https://react.dev/)

### Inspiration

- [RustDesk](https://github.com/rustdesk/rustdesk)
- [Moonlight](https://github.com/moonlight-stream)
- [Parsec](https://parsec.app/)

### APIs

- [DXGI Desktop Duplication](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- [X11 Protocol](https://www.x.org/releases/current/doc/)
- [QUIC RFC 9000](https://www.rfc-editor.org/rfc/rfc9000.html)

---

## 18. APPENDIX

### A. Key Design Decisions

| Decision           | Rationale                                          |
| ------------------ | -------------------------------------------------- |
| Clean Architecture | Testability, flexibility, maintainability          |
| Rust               | Performance, safety, cross-platform                |
| QUIC               | Modern, secure, NAT-friendly                       |
| Tauri              | Native performance, small binary, Rust integration |
| Monorepo           | Code sharing, atomic changes, easier development   |

### B. Trade-offs

| Choice                  | Pros                | Cons                         |
| ----------------------- | ------------------- | ---------------------------- |
| JPEG codec (V1)         | Simple, compatible  | Large payload, lower quality |
| bincode serialization   | Fast, type-safe     | Not cross-language           |
| Direct QUIC (no WebRTC) | Simpler, fewer deps | Harder NAT traversal         |
| Tauri vs Electron       | Smaller, faster     | Smaller ecosystem            |

### C. Future Considerations

- WebAssembly for web client
- Hardware encoding (NVENC, QuickSync)
- Adaptive bitrate streaming
- Multi-user sessions
- Recording & playback

---

**End of Architecture Documentation**
