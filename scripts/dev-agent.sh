#!/bin/bash

echo "=== Starting Remote Desktop Agent ==="

# Build first
cargo build --bin rd-agent

# Run agent
RUST_LOG=info cargo run --bin rd-agent
