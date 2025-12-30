# 🚀 Quick Reference - Remote Desktop Platform

## 📁 Project Location

```
/home/bachtv/Data/Desktop/project/remote_app
```

## ⚡ Quick Commands

### Build & Test

```bash
# Check compilation
cargo check --workspace

# Build all
cargo build --workspace

# Run tests
cargo test --workspace

# Build release
cargo build --workspace --release
```

### Run Components

```bash
# Server
RUST_LOG=info cargo run --bin rd-server

# Agent
RUST_LOG=info cargo run --bin rd-agent

# CLI - Debug transport
cargo run --bin rd-cli -- debug -s 127.0.0.1:4433

# CLI - Connect
cargo run --bin rd-cli -- connect <device-id>
```

### Development

```bash
# Format code
cargo fmt --all

# Lint
cargo clippy --workspace

# Watch mode (install cargo-watch first)
cargo watch -x check
```

## 📚 Key Files

| File                   | Purpose                         |
| ---------------------- | ------------------------------- |
| `README.md`            | Project overview                |
| `SUMMARY.md`           | Completion summary              |
| `PROJECT_STATUS.md`    | Status & roadmap                |
| `docs/architecture.md` | Full architecture (18 sections) |
| `docs/protocol.md`     | QUIC protocol spec              |
| `docs/development.md`  | Dev guide                       |

## 🗂️ Crate Structure

```
rd-core       → Domain models + ports
rd-codec      → JPEG encoder/decoder
rd-transport  → QUIC client/server
rd-platform   → Windows/Linux/macOS adapters
rd-server     → Signaling server binary
rd-agent      → Agent binary
rd-client     → Client library
rd-cli        → CLI tool
```

## 🎯 Next Steps

1. **Install Rust**

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Build**

   ```bash
   cd /home/bachtv/Data/Desktop/project/remote_app
   cargo build --workspace
   ```

3. **Implement platform code**

   - `crates/rd-platform/src/screen_capture/windows.rs`
   - `crates/rd-platform/src/input_injection/windows.rs`

4. **Create Tauri UI**
   - Initialize in `desktop/` folder
   - Use `rd-client` library

## 🔍 Find Things

```bash
# Find all TODOs
rg "TODO" --type rust

# Find specific function
rg "async fn capture" --type rust

# Find error definitions
rg "pub enum.*Error" --type rust
```

## 🐛 Debug

```bash
# Full debug logs
RUST_LOG=trace cargo run --bin rd-server

# Specific module
RUST_LOG=rd_transport=debug cargo run --bin rd-agent

# With backtrace
RUST_BACKTRACE=1 cargo run --bin rd-cli
```

## 📊 Stats

- **9** crates
- **46** Rust files
- **~7,700** lines total
- **291 KB** size

## 💡 Remember

- Domain layer không phụ thuộc infrastructure
- Tất cả I/O đi qua traits
- Config qua TOML + env vars
- Logging qua `tracing` crate
- Errors qua `thiserror` (libs) / `anyhow` (bins)

---

**Quick Link:** [Full Architecture](docs/architecture.md)
