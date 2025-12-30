# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned

- Platform-specific screen capture implementations
- Platform-specific input injection implementations
- H.264 codec integration
- NAT traversal (STUN/TURN)
- Production authentication system
- Multi-monitor support

## [0.1.0] - 2025-12-30

### Added

- **Core Architecture**

  - Clean Architecture / Hexagonal pattern implementation
  - 9 Rust crates workspace structure
  - Domain models: Session, Peer, ScreenFrame, InputEvent
  - Ports/traits for dependency inversion
  - Comprehensive error handling with ApplicationError

- **Transport Layer**

  - QUIC client and server implementation using quinn 0.11
  - TLS 1.3 encryption with rustls and aws-lc-rs
  - ALPN protocol "rdp/1" support
  - Self-signed certificate generation for development
  - QuicTransport wrapper for QUIC connections

- **Protocol**

  - 13 protocol message types (bincode serialization)
  - Hello/HelloAck for device registration
  - SessionRequest/SessionResponse for session management
  - ScreenFrame for frame data
  - InputEvent for mouse/keyboard events
  - Ping/Pong for keep-alive

- **Codec**

  - JPEG encoder implementation (quality 1-100)
  - JPEG decoder implementation
  - ScreenFrame encoding/decoding

- **Server Binary** (`rd-server`)

  - QUIC server on 0.0.0.0:4433
  - Connection handler with state management
  - CLI with clap (--version, --help)
  - Logging with tracing/env_logger

- **Agent Binary** (`rd-agent`)

  - QUIC client connection to server
  - Device registration with hostname
  - Platform detection (Windows/Linux/macOS)
  - Screen capture loop (stub implementation)

- **Client Library** (`rd-client`)

  - RemoteSession API for connection management
  - Frame receiver task with async channel
  - Input event sending
  - Connection lifecycle management

- **CLI Tool** (`rd-cli`)

  - Debug command for QUIC transport testing
  - Connection diagnostics
  - List and connect commands (stubs)

- **Desktop Application** (`rd-desktop`)

  - Tauri v2 + React 18 + TypeScript stack
  - Connection panel with server/agent inputs
  - Connect/Disconnect buttons
  - Status bar with connection state
  - Canvas viewer for remote screen (UI only)
  - 4 Tauri commands: connect_agent, disconnect, get_frame, send_input

- **Documentation**

  - Architecture documentation (18 sections)
  - Protocol specification
  - Development guide
  - README with quick start guide

- **Build System**
  - Cargo workspace with 9 members
  - Shared dependencies configuration
  - npm/Vite for Tauri frontend
  - .gitignore for Rust and Node.js

### Changed

- Updated Rust requirement to 1.92+
- Switched from custom Result type to std::result::Result in traits
- Fixed ALPN protocol configuration in QUIC client

### Fixed

- Result type compilation errors in domain ports
- Missing rustls crypto provider initialization
- QUIC TLS handshake failures
- Tauri dependency issues (rustls, lib name)
- Port conflicts in development server

### Security

- ⚠️ Using self-signed certificates (development only)
- ⚠️ No authentication beyond device ID
- ⚠️ Not suitable for production use

## [0.0.1] - 2025-12-29

### Added

- Initial project scaffolding
- Basic project structure planning

---

**Legend**:

- ✅ Fully implemented and tested
- 🚧 Partially implemented or in progress
- ⏳ Planned but not started
- ⚠️ Known limitations or warnings
