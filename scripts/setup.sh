#!/bin/bash

echo "=== Remote Desktop Project Setup ==="
echo ""

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "Rust is not installed. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo "✓ Rust installed"
else
    echo "✓ Rust already installed: $(rustc --version)"
fi

# Update Rust to latest stable
echo ""
echo "Updating Rust toolchain..."
rustup update stable
rustup default stable

echo ""
echo "✓ Setup complete!"
echo ""
echo "Next steps:"
echo "  1. cargo check --workspace    # Check for compilation errors"
echo "  2. cargo build --release       # Build all binaries"
echo "  3. cargo test                  # Run tests"
