# Remote Desktop Platform (AnyDesk Alternative)

A high-performance, cross-platform remote desktop application built with Rust and Tauri v2, featuring low-latency P2P screen streaming, remote mouse/keyboard control, bidirectional clipboard synchronization, audio streaming, and chunked file transfer.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri v2](https://img.shields.io/badge/tauri-v2-blue.svg)](https://tauri.app/)

---

## 🚀 Implementation Status (v0.2.0 Beta)

- ✅ **Clean Architecture Monorepo**: 9 modular Rust crates adhering to Hexagonal/Ports-and-Adapters principles.
- ✅ **WebRTC P2P DataChannel Transport**: Low-latency peer-to-peer transport with SCTP MTU chunking (`FrameChunk`) and frame reassembly.
- ✅ **Signaling Server (`rd-signaling`)**: High-concurrency Axum WebSocket signaling server on port 3030 with automated room-based SDP/ICE routing.
- ✅ **Cross-Platform Screen Capture**:
  - **macOS**: ScreenCaptureKit & CoreGraphics.
  - **Windows**: Win32 GDI Desktop Capture with top-down BGRA DIB bitmap.
  - **Linux**: X11 `x11rb` `ZPixmap` root window capture.
- ✅ **Cross-Platform Remote Input Injection**:
  - **macOS**: Native `CGEvent` mouse movement, clicks, scrolling, and keyboard events.
  - **Windows**: Win32 `SendInput` API with normalized $0..65535$ coordinates.
  - **Linux**: X11 XTest extension (`xtest_fake_input`).
- ✅ **AnyDesk-Grade Security & Access Control**:
  - **9-Digit Peer IDs**: Fast copy with visual confirmation.
  - **Unattended Access**: Remote password protection with SHA-256 hash verification.
  - **Incoming Connection Prompt**: Real-time modal with Accept/Decline and granular input permission toggle (`allow_input`).
- ✅ **Advanced Features**:
  - 📁 **Bidirectional File Transfer**: 32KB streaming chunks, live progress bar, auto-saving to system `Downloads`.
  - 📋 **Bidirectional Clipboard Sharing**: Background polling with anti-reflection loop protection and visual sync badges.
  - 🔊 **Low-Latency Audio Streaming**: System audio capture via `cpal` (16-bit PCM 48kHz Stereo) and browser playback via Web Audio API (`AudioContext`).
- ✅ **Modern Tauri v2 Desktop App**: React 18, TypeScript, TailwindCSS, Tabler Icons, dynamic quality selector (Speed/Balanced/Best), and live FPS monitoring.

---

## 🏛️ Architecture Overview

The codebase is organized as a Rust workspace monorepo:

```
remote_app/
├── crates/
│   ├── rd-core/         # Domain models, ports/traits, and protocol message definitions
│   ├── rd-codec/        # Zero-copy SIMD JPEG encoder/decoder with dynamic quality
│   ├── rd-transport/    # WebRTC (webrtc-rs) DataChannel & QUIC (quinn) transports
│   ├── rd-platform/     # OS-specific screen capture, input injection, audio, and clipboard
│   ├── rd-signaling/    # Axum WebSocket signaling server (port 3030)
│   ├── rd-server/       # QUIC relay server (port 4433)
│   ├── rd-agent/        # Host background service
│   ├── rd-client/       # Remote session client library
│   └── rd-cli/          # Command-line diagnostics tool
├── rd-desktop/          # Tauri v2 desktop application
│   ├── src/             # React 18 + TypeScript + TailwindCSS frontend
│   └── src-tauri/       # Tauri backend commands and background async tasks
└── docs/                # Project documentation and specifications
    ├── PLAN.md          # Project milestones and roadmap
    ├── architecture.md  # Detailed architectural diagrams
    ├── protocol.md      # Binary protocol specification
    └── development.md   # Setup and contribution guide
```

---

## ⚡ Quick Start

### 1. Prerequisites

- **Rust 1.92+**: [Install Rust](https://rustup.rs/)
- **Node.js 20+**: [Install Node.js](https://nodejs.org/)
- **Platform Dependencies**:
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`).
  - **Windows**: Visual Studio 2019+ with C++ desktop development tools.
  - **Linux**: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libasound2-dev`, `libx11-dev`, `libxtst-dev`.

### 2. Running Locally (Step-by-Step)

#### Step 1: Start the Signaling Server
Open a terminal and launch the signaling server:
```bash
cargo run --bin rd-signaling
```
*The signaling server listens on `ws://127.0.0.1:3030/ws`.*

#### Step 2: Run the Tauri Desktop Client
In another terminal, start the desktop application:
```bash
cd rd-desktop
npm install
npm run tauri dev
```

#### Step 3: Connect & Control
1. The app displays your **9-Digit Peer ID** (e.g. `123 456 789`).
2. Set an **Unattended Access Password** or leave it blank to require interactive confirmation.
3. Open a second client instance (or connect from another device pointing to your signaling server IP).
4. Enter the Host's Peer ID and click **Connect**.
5. Control the screen, use mouse/keyboard, copy-paste across machines, send files, or listen to audio!

---

## 🎛️ Key User Interface Features

- **Floating Toolbar**: Hoverable control bar during an active session with:
  - 🔊 **Audio Toggle**: Mute or unmute host audio streaming in real time.
  - 📁 **File Transfer**: Send files directly to the remote machine's `Downloads` directory with progress feedback.
  - 📋 **Clipboard Synced Badge**: Visual confirmation whenever text is copied across sessions.
  - 🎚️ **Stream Quality Switcher**: Toggle dynamically between **Speed** ($Q=50$), **Balanced** ($Q=70$), and **Best** ($Q=85$).
  - 📈 **Real-Time FPS**: Live render framerate indicator.
  - 🖥️ **Fullscreen Mode & Disconnect**: Quick session exit and viewport maximization.

---

## 🔒 Security & Privacy

- **WebRTC DTLS / SRTP**: All peer-to-peer data and media streams are end-to-end encrypted using DTLS 1.2/1.3.
- **Access Authorization**:
  - Interactive Mode: Explicit **Accept / Decline** dialog on the host with optional mouse/keyboard permission toggling (`allow_input`).
  - Unattended Mode: SHA-256 hashed password verification before establishing the session.
- **OS Permissions**:
  - macOS requires **Screen Recording** and **Accessibility** permissions granted in *System Settings -> Privacy & Security*.

---

## 🛠️ Testing & Verification

```bash
# Run all workspace unit tests
cargo test --workspace

# Check compilation across platforms
cargo check -p rd-platform --target aarch64-pc-windows-msvc
cargo check -p rd-platform --target aarch64-unknown-linux-gnu

# Run Tauri frontend build
cd rd-desktop && npm run build
```

---

## 📄 License

Dual-licensed under either:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))
