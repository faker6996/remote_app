#!/bin/bash

# Development mode - run server in background

echo "=== Starting Remote Desktop Server ==="

# Build first
cargo build --bin rd-server

# Run server
RUST_LOG=info cargo run --bin rd-server
