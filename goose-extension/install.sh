#!/bin/bash
set -e

echo "🔧 Installing Atomic MCP Bridge for Goose..."

# Build the MCP bridge
echo "Building atomic-mcp-bridge..."
cd "$(dirname "$0")/.."
cargo build --release --package atomic-mcp-bridge

# Paths
BINARY="$(pwd)/target/release/atomic-mcp-bridge"
GOOSE_DIR="${HOME}/.config/goose"
EXTENSIONS_DIR="${GOOSE_DIR}/extensions"

# Create directories
mkdir -p "$EXTENSIONS_DIR"

# Install binary to goose extensions
if [[ -f "$BINARY" ]]; then
    cp "$BINARY" "$EXTENSIONS_DIR/atomic-mcp-bridge"
    chmod +x "$EXTENSIONS_DIR/atomic-mcp-bridge"
    echo "✅ Installed: $EXTENSIONS_DIR/atomic-mcp-bridge"
else
    echo "❌ Build failed - binary not found"
    exit 1
fi

# Create Goose config if it doesn't exist
GOOSE_CONFIG="$GOOSE_DIR/extensions.toml"

if [[ ! -f "$GOOSE_CONFIG" ]]; then
    echo "Creating Goose config..."
    cat > "$GOOSE_CONFIG" << 'EOF'
# Goose Extensions Configuration

[extensions.atomic]
name = "Atomic Knowledge Base"
description = "Search and chat with your personal Atomic knowledge base"
cmd = "${GOOSE_EXTENSION_PATH}/atomic-mcp-bridge"
type = "stdio"

[extensions.atomic.env]
ATOMIC_HOST = "127.0.0.1"
ATOMIC_PORT = "44380"
# ATOMIC_TOKEN = "your-token-here"
EOF
    echo "✅ Created: $GOOSE_CONFIG"
    echo ""
    echo "⚠️  IMPORTANT: Edit $GOOSE_CONFIG and set your ATOMIC_TOKEN"
    echo "   Get your token from Atomic Settings → API Tokens"
else
    echo "ℹ️  Goose config already exists at: $GOOSE_CONFIG"
    echo "   Add this to [extensions]:"
    echo ""
    echo "[extensions.atomic]"
    echo "name = \"Atomic Knowledge Base\""
    echo "cmd = \"$EXTENSIONS_DIR/atomic-mcp-bridge\""
    echo "type = \"stdio\""
    echo ""
fi

echo ""
echo "✨ Installation Complete!"
echo ""
echo "Next steps:"
echo "1. Edit ~/.config/goose/extensions.toml and set ATOMIC_TOKEN"
echo "2. Make sure Atomic server is running (cargo run --release --bin atomic-server)"
echo "3. Restart Goose"
echo ""
echo "You can then ask Goose:"
echo "  - 'Search my Atomic notes for machine learning papers'"
echo "  - 'Read my notes on project roadmap'"
echo "  - 'What do I know about semantic search?'"
