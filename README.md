# Remote Desktop Platform

A cross-platform remote desktop application built with Rust and Tauri, featuring low-latency screen streaming and remote control capabilities.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)

## Features

- 🚀 **Low Latency**: Real-time screen streaming with < 200ms end-to-end delay
- 🔒 **Secure**: QUIC transport with TLS 1.3 encryption by default
- 🎯 **Cross-Platform**: Supports Windows, Linux, and macOS
- 🖥️ **Modern UI**: Desktop client built with Tauri (Rust + React)
- ⚡ **Performance**: Hardware-accelerated screen capture (DXGI on Windows)
- 🧩 **Clean Architecture**: Modular design with hexagonal/ports-and-adapters pattern

## Architecture

The project is organized as a Rust workspace monorepo:

```
remote-desktop/
├── crates/           # Core Rust libraries and binaries
│   ├── rd-core       # Domain models and business logic
│   ├── rd-codec      # Video encoding/decoding (JPEG, H.264)
│   ├── rd-transport  # QUIC network transport
│   ├── rd-platform   # OS-specific implementations
│   ├── rd-server     # Signaling & relay server
│   ├── rd-agent      # Agent service (runs on host machine)
│   ├── rd-client     # Client library
│   └── rd-cli        # CLI tool
├── desktop/          # Tauri desktop application
└── docs/             # Documentation
```

See [docs/architecture.md](docs/architecture.md) for detailed architecture documentation.

## Quick Start

### Prerequisites

- **Rust 1.80+**: [Install Rust](https://rustup.rs/)
- **Node.js 20+**: [Install Node.js](https://nodejs.org/) (for Tauri frontend)
- **Platform-specific**:
  - Windows: Visual Studio 2019+ with C++ development tools
  - Linux: `libx11-dev`, `libxrandr-dev`, `libxtst-dev`
  - macOS: Xcode Command Line Tools

### Build & Run

```bash
# Clone the repository
git clone https://github.com/your-org/remote-desktop.git
cd remote-desktop

# Build Rust workspace
cargo build --release

# Run the server
./target/release/rd-server

# Run the agent (on the machine to be controlled)
./target/release/rd-agent

# Run the desktop client
cd desktop
npm install
npm run tauri dev
```

## Components

### 1. Server (`rd-server`)

Signaling and relay server for coordinating connections between clients and agents.

```bash
# Run server
cargo run -p rd-server -- --config config/server.toml

# Default port: 4433 (QUIC)
```

### 2. Agent (`rd-agent`)

Service that runs on the host machine, captures screen, and handles remote input.

```bash
# Run agent
cargo run -p rd-agent -- --config config/agent.toml

# Install as service (Windows)
rd-agent install

# Install as systemd service (Linux)
sudo cp scripts/rd-agent.service /etc/systemd/system/
sudo systemctl enable rd-agent
sudo systemctl start rd-agent
```

### 3. Desktop Client (`desktop/`)

Tauri-based desktop application for viewing and controlling remote machines.

```bash
cd desktop
npm run tauri dev    # Development mode
npm run tauri build  # Production build
```

### 4. CLI (`rd-cli`)

Command-line tool for testing and debugging.

```bash
# List available agents
cargo run -p rd-cli -- list

# Connect to an agent
cargo run -p rd-cli -- connect <agent-id>

# Debug transport
cargo run -p rd-cli -- debug-transport
```

## Configuration

Configuration files use TOML format:

**Server (`config/server.toml`):**

```toml
[server]
bind_address = "0.0.0.0:4433"
cert_path = "certs/server.crt"
key_path = "certs/server.key"

[relay]
enabled = true
max_sessions = 100
```

**Agent (`config/agent.toml`):**

```toml
[agent]
device_id = "my-desktop"
server_url = "https://server.example.com:4433"

[capture]
max_fps = 30
resolution = "1920x1080"

[encoder]
codec = "jpeg"
quality = 80
```

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p rd-core

# Run with logging
RUST_LOG=debug cargo test
```

### Code Style

```bash
# Format code
cargo fmt

# Run clippy
cargo clippy -- -D warnings
```

### Building Documentation

```bash
# Generate API docs
cargo doc --no-deps --open

# See architecture docs
cat docs/architecture.md
```

## Platform Support

| Platform      | Screen Capture           | Input Injection  | Status     |
| ------------- | ------------------------ | ---------------- | ---------- |
| Windows 10/11 | DXGI Desktop Duplication | WinAPI SendInput | ✅ Stable  |
| Ubuntu 22.04+ | X11 (x11rb)              | XTest            | 🚧 Beta    |
| macOS 12+     | CoreGraphics             | CGEvent          | 📋 Planned |

## Security

- **Transport**: QUIC with TLS 1.3 (encrypted by default)
- **Authentication**: Token-based auth (V1), user accounts planned
- **Permissions**: OS-level permissions required for screen capture and input injection

See [docs/security.md](docs/security.md) for security best practices.

## Roadmap

- [x] Core architecture and domain models
- [x] QUIC transport layer
- [x] Windows screen capture (DXGI)
- [ ] Basic Tauri UI
- [ ] Linux X11 support
- [ ] H.264 hardware encoding
- [ ] NAT traversal with STUN/TURN
- [ ] User accounts and authentication
- [ ] Mobile client (Tauri mobile)
- [ ] Audio streaming
- [ ] File transfer
- [ ] Multi-monitor support

See [docs/roadmap.md](docs/roadmap.md) for detailed roadmap.

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

## Contact

- GitHub Issues: [Report bugs or request features](https://github.com/your-org/remote-desktop/issues)
- Documentation: [Full documentation](docs/)

---

**Status**: 🚧 Active Development (Pre-Alpha)
