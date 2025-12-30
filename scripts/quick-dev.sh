#!/bin/bash

# Quick development start - all in one terminal with tmux

set -e

echo "🚀 Starting Remote Desktop App in Development Mode"
echo ""

# Check if tmux is installed
if ! command -v tmux &> /dev/null; then
    echo "⚠️  tmux not installed. Installing recommended."
    echo ""
    echo "Manual start instead:"
    echo ""
    echo "Terminal 1: RUST_LOG=info cargo run --bin rd-server"
    echo "Terminal 2: RUST_LOG=info cargo run --bin rd-agent"
    echo "Terminal 3: cargo run --bin rd-cli -- --help"
    exit 1
fi

# Kill existing session if any
tmux kill-session -t rd-dev 2>/dev/null || true

# Create new session
tmux new-session -d -s rd-dev -n "server"

# Window 1: Server
tmux send-keys -t rd-dev:server "cd $(pwd)" C-m
tmux send-keys -t rd-dev:server "echo '📡 Starting Server...'" C-m
tmux send-keys -t rd-dev:server "RUST_LOG=info cargo run --bin rd-server" C-m

# Window 2: Agent
tmux new-window -t rd-dev -n "agent"
tmux send-keys -t rd-dev:agent "cd $(pwd)" C-m
tmux send-keys -t rd-dev:agent "sleep 2" C-m
tmux send-keys -t rd-dev:agent "echo '🖥️  Starting Agent...'" C-m
tmux send-keys -t rd-dev:agent "RUST_LOG=info cargo run --bin rd-agent" C-m

# Window 3: CLI
tmux new-window -t rd-dev -n "cli"
tmux send-keys -t rd-dev:cli "cd $(pwd)" C-m
tmux send-keys -t rd-dev:cli "echo '🎮 CLI Ready. Examples:'" C-m
tmux send-keys -t rd-dev:cli "echo '  cargo run --bin rd-cli -- --help'" C-m
tmux send-keys -t rd-dev:cli "echo '  cargo run --bin rd-cli -- debug -s 127.0.0.1:4433'" C-m

# Attach to session
echo "✅ Started tmux session 'rd-dev'"
echo ""
echo "Controls:"
echo "  Ctrl+B then 0,1,2 - Switch windows (server/agent/cli)"
echo "  Ctrl+B then d     - Detach (keep running)"
echo "  tmux attach -t rd-dev - Reattach"
echo "  tmux kill-session -t rd-dev - Stop all"
echo ""

tmux attach -t rd-dev
