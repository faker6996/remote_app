# Protocol Specification

**Version:** 1.0  
**Transport:** QUIC over UDP  
**Serialization:** bincode (Rust serde)

---

## Overview

The Remote Desktop Protocol (RDP) uses QUIC as the transport layer, providing built-in encryption (TLS 1.3), multiplexing, and low latency.

Messages are serialized using bincode and sent over QUIC bidirectional streams.

---

## Message Format

Each message consists of:

- **Length Prefix** (4 bytes, big-endian u32): Total message size
- **Message Data** (N bytes): bincode-serialized ProtocolMessage

```
+--------+--------+--------+--------+------------------------+
|  Len (u32, BE)              |  Message Data (bincode)  |
+--------+--------+--------+--------+------------------------+
```

---

## Message Types

### 1. Handshake & Authentication

#### Hello

Sent by client/agent when connecting to server.

```rust
Hello {
    version: u32,        // Protocol version (currently 1)
    device_id: String,   // Unique device identifier
    platform: Platform,  // OS platform (Windows, Linux, macOS)
}
```

**Example:**

```rust
Hello {
    version: 1,
    device_id: "laptop-001",
    platform: Platform::Windows,
}
```

#### Auth

Authentication request with token.

```rust
Auth {
    token: AuthToken {
        token: String,      // Authentication token
        device_id: String,  // Device ID
    }
}
```

#### AuthResponse

Server response to authentication.

```rust
AuthResponse {
    success: bool,
    session_id: Option<SessionId>,  // Set if successful
    error: Option<String>,           // Set if failed
}
```

---

### 2. Session Management

#### SessionRequest

Client requests a session with an agent.

```rust
SessionRequest {
    target_device: String,  // Agent device ID
}
```

#### SessionCreated

Server confirms session creation.

```rust
SessionCreated {
    session_id: SessionId,  // UUID
    endpoint: String,       // Connection endpoint (IP:port or relay)
}
```

#### SessionEnd

Close a session.

```rust
SessionEnd {
    session_id: SessionId,
    reason: String,  // "user_disconnect", "timeout", "error", etc.
}
```

---

### 3. Streaming

#### ScreenFrame

Agent sends screen frame to client.

```rust
ScreenFrame {
    sequence: u64,         // Frame sequence number
    timestamp: u64,        // Unix timestamp (milliseconds)
    data: Vec<u8>,         // Encoded frame data
    width: u32,            // Frame width in pixels
    height: u32,           // Frame height in pixels
    format: FrameFormat,   // Raw, Jpeg, H264, VP8, AV1
}
```

#### FrameChunk (SCTP Packet Chunking)

When frame payload exceeds the WebRTC SCTP MTU limit (~50KB), the frame is split into multiple ordered chunks.

```rust
FrameChunk {
    sequence: u64,         // Frame sequence number
    timestamp: u64,        // Unix timestamp (milliseconds)
    chunk_index: u32,      // 0-based chunk index
    total_chunks: u32,     // Total chunk count for this frame
    data: Vec<u8>,         // Fragment payload
    width: u32,            // Frame width
    height: u32,           // Frame height
    format: FrameFormat,   // Frame format
}
```

---

### 4. Advanced Features

#### ClipboardSync

Bidirectional clipboard text synchronization.

```rust
ClipboardSync {
    text: String,          // Clipboard text content
}
```

#### File Transfer Protocol

Chunked file streaming across peers:

```rust
// 1. Negotiation
FileTransferRequest {
    transfer_id: String,   // Unique transfer UUID
    file_name: String,     // Target filename
    file_size: u64,        // Total file size in bytes
}

FileTransferResponse {
    transfer_id: String,
    accepted: bool,        // Accept or decline
}

// 2. Data Streaming (32KB chunks)
FileChunk {
    transfer_id: String,
    chunk_index: u32,
    total_chunks: u32,
    data: Vec<u8>,
}

// 3. Completion
FileTransferComplete {
    transfer_id: String,
}
```

#### AudioFrame

Low-latency audio streaming from host to viewer:

```rust
AudioFrame {
    sequence: u64,         // Audio packet sequence
    timestamp: u64,        // Capture timestamp
    sample_rate: u32,      // e.g., 48000 Hz
    channels: u16,         // e.g., 2 (Stereo)
    data: Vec<u8>,         // 16-bit PCM little-endian samples
}
```

---

### 5. Input Control

#### InputEvent

Client sends input event to agent.

```rust
InputEvent {
    timestamp: u64,      // Unix timestamp (milliseconds)
    event: InputEvent,   // Actual event data
}

enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseButton { button: MouseButton, pressed: bool },
    MouseScroll { delta_x: i32, delta_y: i32 },
    KeyPress { key: KeyCode, pressed: bool },
}
```

**Mouse Buttons:**

- `Left`
- `Right`
- `Middle`
- `X1` (back)
- `X2` (forward)

**KeyCode:** Platform-agnostic key codes (mapped to OS-specific codes by agent).

---

### 5. Control & Health

#### Heartbeat

Periodic keepalive message.

```rust
Heartbeat {
    timestamp: u64,  // Unix timestamp (milliseconds)
}
```

**Frequency:** Every 10 seconds (configurable)

#### Error

Generic error message.

```rust
Error {
    code: u32,       // Error code
    message: String, // Human-readable error
}
```

**Error Codes:**

- `1000`: Connection error
- `2000`: Authentication error
- `3000`: Session error
- `4000`: Encoding error
- `5000`: Unknown error

#### Disconnect

Graceful disconnect.

```rust
Disconnect
```

---

## Connection Flow

### Agent Registration

```
Agent                    Server
  |                         |
  |---- Hello ------------->|
  |                         |
  |<--- AuthResponse -------|  (success=true)
  |                         |
  |---- Heartbeat --------->|  (every 10s)
  |<--- Heartbeat ----------|
```

### Session Creation

```
Client                Server                 Agent
  |                      |                      |
  |--- SessionRequest -->|                      |
  |    (target=agent-1)  |                      |
  |                      |--- Notify ---------->|
  |                      |                      |
  |<-- SessionCreated ---|                      |
  |                      |                      |
  |<======== Direct QUIC Connection =========>|
  |                                            |
  |<------- ScreenFrame ----------------------|
  |-------- InputEvent ---------------------->|
```

### Streaming Session

```
Agent                                    Client
  |                                         |
  |-- ScreenFrame (seq=1) ----------------->|
  |                                         |
  |<- InputEvent (MouseMove) --------------|
  |                                         |
  |-- ScreenFrame (seq=2) ----------------->|
  |                                         |
  |<- InputEvent (KeyPress) ---------------|
  |                                         |
  |-- ScreenFrame (seq=3) ----------------->|
```

---

## QUIC Details

### Connection Parameters

```rust
// Client
ClientConfig {
    alpn_protocols: ["rdp/1"],
    max_concurrent_bidi_streams: 10,
    max_concurrent_uni_streams: 0,
    max_idle_timeout: 30_000ms,
}

// Server
ServerConfig {
    alpn_protocols: ["rdp/1"],
    max_concurrent_bidi_streams: 100,
    max_concurrent_uni_streams: 0,
    max_idle_timeout: 60_000ms,
}
```

### Stream Usage

- **Control Stream** (stream 0): Handshake, session management, heartbeats
- **Data Streams** (stream 1+): Screen frames, input events

### Multiplexing

Multiple concurrent sessions can share a single QUIC connection by using different streams.

---

## Security

### TLS 1.3

All QUIC connections use TLS 1.3 for encryption. Server uses self-signed certificates (development) or proper CA-signed certificates (production).

### Authentication

V1: Pre-shared token (device token)  
Future: OAuth2, user accounts, session tokens

### Authorization

Agents only accept connections from authenticated clients via server.

---

## Error Handling

### Connection Errors

- **Timeout**: No response within 30 seconds
- **Refused**: Port closed or server not running
- **TLS Error**: Certificate validation failed

**Client behavior:** Retry with exponential backoff (1s, 2s, 4s, 8s, max 60s)

### Frame Errors

- **Decode Error**: Invalid frame data
- **Sequence Gap**: Missing frames

**Client behavior:** Request keyframe or continue with next frame

### Input Errors

- **Injection Failed**: OS-level injection error

**Agent behavior:** Log error, ignore event

---

## Performance Considerations

### Bandwidth Estimation

```
Bandwidth (Mbps) = FPS × Frame_Size × 8 / 1,000,000

Example (1080p @ 30 FPS, JPEG quality 80):
  Frame_Size ≈ 50 KB
  Bandwidth ≈ 30 × 50,000 × 8 / 1,000,000 = 12 Mbps
```

### Latency Budget

```
Total Latency = Capture + Encode + Network + Decode + Render
Target:         10-60ms   5-20ms   10-100ms  5-15ms   16ms
                                                      (60 FPS)
```

### Frame Dropping

If client can't keep up:

- Agent drops oldest frames in send queue
- Client requests keyframe to recover

---

## Future Extensions

### Implemented Features (v0.2.0)

1. **Audio Streaming**: `AudioFrame` message with 16-bit PCM (48kHz Stereo)
2. **File Transfer**: `FileTransferRequest`, `FileTransferResponse`, `FileChunk` (32KB), `FileTransferComplete`
3. **Clipboard Sync**: `ClipboardSync` bidirectional synchronization with anti-echo protection
4. **SCTP Chunking**: `FrameChunk` packet splitting for MTU protection

### Planned Features

1. **Hardware H.264 / AV1 Codec**: Video packetization over WebRTC MediaStream
2. **Multi-monitor**: Add `display_id` to ScreenFrame / dynamic screen switcher
3. **Region / Dirty Rect Updates**: Send only changed screen tile bounding boxes

### Protocol Versioning

When breaking changes occur:

- Increment version in Hello message
- Server/Agent check version compatibility
- Return error if versions incompatible

---

## Appendix

### Example Message Sizes

| Message Type              | Typical Size |
| ------------------------- | ------------ |
| Hello                     | ~100 bytes   |
| Auth                      | ~200 bytes   |
| Heartbeat                 | ~20 bytes    |
| InputEvent                | ~50 bytes    |
| ScreenFrame (JPEG, 1080p) | ~50 KB       |
| ScreenFrame (H264, 1080p) | ~10 KB       |

### Bincode Serialization

Efficient binary format:

- Integers: Fixed-size (u32 = 4 bytes)
- Strings: Length-prefixed (u64 length + UTF-8 bytes)
- Enums: Variant index (u32) + data
- Vectors: Length (u64) + elements

**Example:**

```rust
Heartbeat { timestamp: 1234567890 }
→ [0, 0, 0, 0, 73, 150, 2, 210]  (9 bytes: variant + u64)
```

---

**End of Protocol Specification**
