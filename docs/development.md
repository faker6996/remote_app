# Development Guide

## Prerequisites

### Rust Development

- **Rust 1.80+**: Install via [rustup](https://rustup.rs/)
- **Cargo**: Comes with Rust installation

### Platform-Specific Requirements

#### Windows

- Visual Studio 2019+ with "Desktop development with C++" workload
- Windows SDK

#### Linux (Ubuntu/Debian)

```bash
sudo apt install -y \
    build-essential \
    pkg-config \
    libx11-dev \
    libxrandr-dev \
    libxtst-dev
```

#### macOS

```bash
xcode-select --install
```

## Getting Started

### 1. Clone and Setup

```bash
git clone <repo-url>
cd remote-desktop

# Run setup script (installs Rust if needed)
./scripts/setup.sh
```

### 2. Build Project

```bash
# Check for compilation errors
cargo check --workspace

# Build all crates
cargo build --workspace

# Build in release mode
cargo build --workspace --release
```

### 3. Run Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for specific crate
cargo test -p rd-core

# Run with logging
RUST_LOG=debug cargo test
```

## Project Structure

```
remote-desktop/
├── crates/           # Rust workspace
│   ├── rd-core       # Domain models & business logic
│   ├── rd-codec      # Video encoding/decoding
│   ├── rd-transport  # QUIC network layer
│   ├── rd-platform   # OS-specific implementations
│   ├── rd-server     # Signaling server
│   ├── rd-agent      # Agent service
│   ├── rd-client     # Client library
│   └── rd-cli        # CLI tool
├── desktop/          # Tauri desktop app (TODO)
├── config/           # Configuration files
├── scripts/          # Build & dev scripts
└── docs/             # Documentation
```

## Running Components

### Server

```bash
# Development mode
./scripts/dev-server.sh

# Or manually
RUST_LOG=info cargo run --bin rd-server
```

### Agent

```bash
# Development mode
./scripts/dev-agent.sh

# Or manually
RUST_LOG=info cargo run --bin rd-agent
```

### CLI Client

```bash
# List agents
cargo run --bin rd-cli -- list

# Connect to agent
cargo run --bin rd-cli -- connect <agent-id>

# Debug transport
cargo run --bin rd-cli -- debug -s 127.0.0.1:4433
```

## Development Workflow

### Adding a New Feature

1. **Domain-first**: Start with domain models in `rd-core`
2. **Define ports**: Add trait definitions if needed
3. **Implement adapters**: Add concrete implementations in infrastructure crates
4. **Wire up**: Integrate in application layer or binaries
5. **Test**: Add unit tests and integration tests

### Example: Adding a New Codec

```rust
// 1. Add codec type to rd-core/src/domain/models.rs
pub enum CodecType {
    Jpeg,
    H264,
    VP9,  // NEW
}

// 2. Create implementation in rd-codec/src/vp9.rs
pub struct VP9Encoder { ... }

#[async_trait]
impl Encoder for VP9Encoder {
    async fn encode(&mut self, frame: &ScreenFrame) -> Result<Vec<u8>, CodecError> {
        // Implementation
    }
}

// 3. Export in rd-codec/src/lib.rs
pub mod vp9;
pub use vp9::VP9Encoder;

// 4. Use in agent config
[encoder]
codec = "vp9"
```

## Code Style

### Formatting

```bash
# Format all code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check
```

### Linting

```bash
# Run clippy
cargo clippy --workspace -- -D warnings

# Auto-fix some issues
cargo clippy --workspace --fix
```

### Pre-commit Checks

```bash
# Run before committing
cargo fmt --all
cargo clippy --workspace
cargo test --workspace
```

## Debugging

### Logging Levels

```bash
# All logs
RUST_LOG=trace cargo run --bin rd-server

# Specific module
RUST_LOG=rd_transport=debug cargo run --bin rd-agent

# Multiple modules
RUST_LOG=rd_core=info,rd_transport=debug cargo run
```

### VS Code Debugging

Install [rust-analyzer](https://rust-analyzer.github.io/) extension and use launch configurations:

```json
{
  "type": "lldb",
  "request": "launch",
  "name": "Debug rd-server",
  "cargo": {
    "args": ["build", "--bin=rd-server"],
    "filter": {
      "name": "rd-server",
      "kind": "bin"
    }
  },
  "args": [],
  "cwd": "${workspaceFolder}",
  "env": {
    "RUST_LOG": "debug"
  }
}
```

## Performance Profiling

### CPU Profiling

```bash
# Install flamegraph
cargo install flamegraph

# Profile agent
sudo flamegraph -- cargo run --release --bin rd-agent
```

### Memory Profiling

```bash
# Install valgrind
sudo apt install valgrind

# Run with memcheck
valgrind --leak-check=full ./target/debug/rd-agent
```

## Common Issues

### Issue: QUIC connection fails

**Solution**: Check firewall settings, ensure port 4433 is open.

### Issue: Screen capture returns black frames (Windows)

**Solution**: Ensure app is running with proper permissions. DXGI requires desktop access.

### Issue: Input injection not working (Linux)

**Solution**: Check X11 permissions, ensure XTEST extension is enabled.

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for contribution guidelines.

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Quinn Documentation](https://docs.rs/quinn/)
- [Clean Architecture](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html)
