#!/bin/bash

set -e

echo "=== Building Remote Desktop Project ==="

# Build workspace
echo ""
echo "Building Rust workspace..."
cargo build --workspace

echo ""
echo "✓ Build complete!"
echo ""
echo "Binaries:"
echo "  - target/debug/rd-server"
echo "  - target/debug/rd-agent"
echo "  - target/debug/rd-cli"
