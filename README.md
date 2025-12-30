# Remote Desktop Platform

A cross-platform remote desktop application built with Rust and Tauri, featuring low-latency screen streaming and remote control capabilities.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org/)

## 🚧 Implementation Status

**Current Version**: v0.1.0 (Alpha)

- ✅ **Core Architecture**: Clean Architecture with 9 Rust crates
- ✅ **QUIC Transport**: Fully working with TLS 1.3 and ALPN protocol
- ✅ **JPEG Codec**: Screen frame encoding/decoding implemented
- ✅ **Server Binary**: QUIC server running on port 4433
- ✅ **Agent Binary**: Connects to server, sends device info
- ✅ **CLI Tool**: Debug and connection testing utility
- ✅ **Desktop UI**: Tauri v2 + React application with connection panel
- 🚧 **Screen Capture**: Platform-specific implementations (stubs only)
- 🚧 **Input Injection**: Platform-specific implementations (stubs only)
- ⏳ **H.264 Codec**: Planned for higher compression
- ⏳ **NAT Traversal**: STUN/TURN support planned

## Features

- 🚀 **Low Latency**: Target < 200ms end-to-end delay (architecture ready)
- 🔒 **Secure**: QUIC transport with TLS 1.3 encryption (implemented)
- 🎯 **Cross-Platform**: Supports Windows, Linux, macOS (in progress)
- 🖥️ **Modern UI**: Desktop client built with Tauri v2 + React (functional)
- ⚡ **Performance**: Hardware-accelerated screen capture planned (DXGI on Windows)
- 🧩 **Clean Architecture**: Modular design with hexagonal/ports-and-adapters pattern

## Architecture

The project is organized as a Rust workspace monorepo:

```
remote_app/
├── crates/           # Core Rust libraries and binaries
│   ├── rd-core       # ✅ Domain models, ports/traits, error types
│   ├── rd-codec      # ✅ JPEG encoder/decoder (H.264 planned)
│   ├── rd-transport  # ✅ QUIC client/server with TLS 1.3
│   ├── rd-platform   # 🚧 OS-specific implementations (stubs)
│   ├── rd-server     # ✅ Signaling & relay server (running)
│   ├── rd-agent      # ✅ Agent service (connects to server)
│   ├── rd-client     # ✅ Client library for remote sessions
│   └── rd-cli        # ✅ CLI tool for debugging/testing
├── rd-desktop/       # ✅ Tauri v2 desktop application
│   ├── src/          # React + TypeScript frontend
│   └── src-tauri/    # Rust backend with Tauri commands
└── docs/             # ✅ Architecture and API documentation
```

See [docs/architecture.md](docs/architecture.md) for detailed architecture documentation.

## Quick Start

### Prerequisites

- **Rust 1.92+**: [Install Rust](https://rustup.rs/)
- **Node.js 20+**: [Install Node.js](https://nodejs.org/) (for Tauri frontend)
- **Platform-specific**:
  - **Linux**: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `librsvg2-dev`, `libayatana-appindicator3-dev`
  - **Windows**: Visual Studio 2019+ with C++ development tools
  - **macOS**: Xcode Command Line Tools

### Build & Run

```bash
# Clone the repository
git clone <repo-url>
cd remote_app

# Install Rust (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Build Rust workspace
cargo build --workspace

# Run the server (Terminal 1)
RUST_LOG=info cargo run --bin rd-server

# Run the agent (Terminal 2)
RUST_LOG=info cargo run --bin rd-agent

# Run the desktop client (Terminal 3)
cd rd-desktop
npm install
npm run tauri dev
```

## Components

### 1. Server (`rd-server`) ✅

Signaling and relay server for coordinating connections between clients and agents.

**Status**: Fully functional, accepts QUIC connections on port 4433

```bash
# Run server
RUST_LOG=info cargo run --bin rd-server

# With CLI options
cargo run --bin rd-server -- --version
cargo run --bin rd-server -- --help

# Default: 0.0.0.0:4433 (QUIC with TLS 1.3)
```

**Features**:

- ✅ QUIC server with ALPN protocol "rdp/1"
- ✅ TLS 1.3 encryption with rustls
- ✅ Connection state management
- 🚧 Message routing (partial)

### 2. Agent (`rd-agent`) ✅

Service that runs on the host machine, captures screen, and handles remote input.

**Status**: Connects to server, sends device information

```bash
# Run agent
RUST_LOG=info cargo run --bin rd-agent

# Agent connects to server at 127.0.0.1:4433
# Device ID: <hostname> (e.g., "bachtv")
```

**Features**:

- ✅ QUIC client connection
- ✅ Device registration (Hello message)
- ✅ Platform detection (Windows/Linux/macOS)
- 🚧 Screen capture loop (stub)
- 🚧 Input injection (stub)

### 3. Desktop Client (`rd-desktop`) ✅

Tauri v2 desktop application with React frontend.

**Status**: UI functional, connection logic implemented

```bash
cd rd-desktop
npm run tauri dev    # Development mode
npm run tauri build  # Production build
```

**Features**:

- ✅ Connection panel (Server + Agent ID inputs)
- ✅ Connect/Disconnect functionality
- ✅ Status bar with connection state
- ✅ Canvas viewer for remote screen
- 🚧 Frame rendering (stub)
- 🚧 Mouse/keyboard input events (stub)

### 4. CLI Tool (`rd-cli`) ✅

Command-line utility for testing and debugging.

```bash
# Debug QUIC transport
cargo run --bin rd-cli -- debug -s 127.0.0.1:4433

# List connected agents (planned)
cargo run --bin rd-cli -- list

# Connect to agent (planned)
cargo run --bin rd-cli -- connect <agent-id>
```

## Protocol & Transport

**Current Implementation**: ✅ QUIC with TLS 1.3

- **Protocol**: QUIC (UDP-based)
- **Encryption**: TLS 1.3 via rustls
- **ALPN**: "rdp/1"
- **Serialization**: bincode (binary)
- **Port**: 4433 (default)

**Protocol Messages** (13 types defined):

- `Hello`, `HelloAck` - Device registration
- `Auth`, `AuthResponse` - Authentication
- `SessionRequest`, `SessionResponse` - Session management
- `ScreenFrame` - Frame data with JPEG encoding
- `InputEvent` - Mouse/keyboard events
- `Ping`, `Pong` - Keep-alive
- `Error`, `Disconnect` - Error handling
- `FileTransfer` - File transfer (planned)

See [docs/protocol.md](docs/protocol.md) for detailed protocol specification.

## Development

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p rd-core

# Run with logging
RUST_LOG=debug cargo test
```

### Code Style

```bash
# Format code
cargo fmt --all

# Run clippy
cargo clippy --workspace -- -D warnings

# Fix simple warnings
cargo fix --workspace --allow-dirty
```

### Building Documentation

```bash
# Generate Rust API docs
cargo doc --no-deps --open

# Read architecture documentation
cat docs/architecture.md
cat docs/protocol.md
cat docs/development.md
```

## Platform Support

| Platform      | Screen Capture             | Input Injection     | Status     |
| ------------- | -------------------------- | ------------------- | ---------- |
| Windows 10/11 | DXGI (planned)             | SendInput (planned) | ⏳ Planned |
| Ubuntu 22.04+ | X11 (planned)              | XTest (planned)     | ⏳ Planned |
| macOS 12+     | ScreenCaptureKit (planned) | CGEvent (planned)   | ⏳ Planned |

**Note**: Platform-specific implementations are currently stubs. Transport layer and UI are functional.
| macOS 12+ | CoreGraphics | CGEvent | 📋 Planned |

## Security

- **Transport**: ✅ QUIC with TLS 1.3 (encrypted by default, fully implemented)
- **Certificate**: ⚠️ Self-signed certificates for development (replace for production)
- **Authentication**: 🚧 Basic device ID only (token-based auth planned)
- **Permissions**: OS-level permissions required for screen capture and input injection

⚠️ **Development Warning**: Current version uses self-signed certificates and basic authentication. Not suitable for production use.

## Roadmap

### ✅ Completed (v0.1.0)

- [x] Core architecture with Clean Architecture pattern
- [x] Domain models, ports, and error types
- [x] QUIC transport layer with TLS 1.3
- [x] JPEG codec implementation
- [x] Server binary (QUIC server on port 4433)
- [x] Agent binary (connects and registers)
- [x] CLI debugging tool
- [x] Tauri v2 desktop UI with React

### 🚧 In Progress

- [ ] Platform-specific screen capture (DXGI, X11, ScreenCaptureKit)
- [ ] Platform-specific input injection
- [ ] End-to-end frame streaming
- [ ] Desktop UI frame rendering

### ⏳ Planned (v0.2.0+)

- [ ] H.264 hardware encoding
- [ ] NAT traversal with STUN/TURN
- [ ] User authentication system
- [ ] Production TLS certificates
- [ ] Multi-monitor support
- [ ] File transfer
- [ ] Audio streaming
- [ ] Mobile client (Tauri mobile)
- [ ] Clipboard synchronization

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is dual-licensed under:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

You may choose either license for your use.

## Acknowledgments

- Inspired by [RustDesk](https://github.com/rustdesk/rustdesk)
- Built with [Tauri](https://tauri.app/), [Quinn](https://github.com/quinn-rs/quinn), and [Tokio](https://tokio.rs/)
- Protocol design inspired by modern remote desktop protocols

## Technology Stack

- **Language**: Rust 1.92.0
- **Transport**: QUIC (quinn 0.11)
- **TLS**: rustls 0.23 with aws-lc-rs
- **Serialization**: bincode + serde
- **Async Runtime**: Tokio 1.x
- **Desktop UI**: Tauri v2 + React 18 + TypeScript
- **Codec**: JPEG (jpeg-encoder), H.264 planned
- **Build System**: Cargo workspaces

---

**Status**: 🚧 Alpha Development (v0.1.0)

**Last Updated**: December 30, 2025
