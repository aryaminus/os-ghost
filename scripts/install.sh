#!/bin/bash
set -e

echo "🎮 Installing The OS Ghost (Tauri Edition)..."
echo ""

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "📦 Rust not found. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo "✅ Rust version: $(cargo --version)"

# Check for Node.js
if ! command -v node &> /dev/null; then
    echo "❌ Node.js is required. Please install Node.js first."
    echo "   Visit: https://nodejs.org/"
    exit 1
fi

echo "✅ Node.js version: $(node --version)"

# Install Node dependencies
echo ""
echo "📦 Installing Node dependencies..."
cd "$PROJECT_DIR"
npm install

# Build Tauri app
echo ""
echo "🔨 Building Tauri application..."
npm run tauri build

# Determine the native_bridge binary path (separate from main app)
if [[ "$OSTYPE" == "darwin"* ]]; then
    BINARY_PATH="$PROJECT_DIR/src-tauri/target/release/native_bridge"
    APP_PATH="$PROJECT_DIR/src-tauri/target/release/bundle/macos/The OS Ghost.app"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    BINARY_PATH="$PROJECT_DIR/src-tauri/target/release/native_bridge"
    APP_PATH="$PROJECT_DIR/src-tauri/target/release/os-ghost"
else
    echo "⚠️  Windows detected. Run install.bat instead."
    exit 1
fi

# Ensure native_bridge binary exists
if [[ ! -f "$BINARY_PATH" ]]; then
    echo "⚠️  native_bridge binary not found. Building separately..."
    cd "$PROJECT_DIR/src-tauri"
    cargo build --release --bin native_bridge
fi

echo ""
echo "📱 Chrome Extension Installation"
echo "================================"
echo "1. Install 'OS Ghost Bridge' from the Chrome Web Store:"
echo "   https://chromewebstore.google.com/detail/os-ghost-bridge/iakaaklohlcdhoalipmmljopmjnhbcdn"
echo ""

# Fixed Extension ID from Web Store
EXTENSION_ID="iakaaklohlcdhoalipmmljopmjnhbcdn"

# Register Native Messaging host
echo ""
echo "🔗 Registering Native Messaging host..."

if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    MANIFEST_DIR="$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"
    mkdir -p "$MANIFEST_DIR"
    
    cat > "$MANIFEST_DIR/com.osghost.game.json" <<EOF
{
  "name": "com.osghost.game",
  "description": "OS Ghost Native Messaging Bridge",
  "path": "$BINARY_PATH",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$EXTENSION_ID/"]
}
EOF
    echo "✅ Native Messaging manifest registered at:"
    echo "   $MANIFEST_DIR/com.osghost.game.json"

elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux
    MANIFEST_DIR="$HOME/.config/google-chrome/NativeMessagingHosts"
    mkdir -p "$MANIFEST_DIR"
    
    cat > "$MANIFEST_DIR/com.osghost.game.json" <<EOF
{
  "name": "com.osghost.game",
  "description": "OS Ghost Native Messaging Bridge",
  "path": "$BINARY_PATH",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$EXTENSION_ID/"]
}
EOF
    echo "✅ Native Messaging manifest registered at:"
    echo "   $MANIFEST_DIR/com.osghost.game.json"
fi

# API Key setup reminder
echo ""
echo "🔑 API Key Configuration"
echo "========================"
echo "To enable AI features, set your Gemini API key:"
echo ""
echo "  export GEMINI_API_KEY='your-api-key-here'"
echo ""
echo "Add this to your ~/.zshrc or ~/.bashrc for persistence."
echo ""

# Completion message
echo ""
echo "✅ Installation complete!"
echo ""
echo "To start the Ghost in development mode:"
echo "  cd $PROJECT_DIR"
echo "  export GEMINI_API_KEY='your-key'"
echo "  npm run tauri dev"
echo ""
echo "To run the production build:"
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "  open \"$APP_PATH\""
else
    echo "  $APP_PATH"
fi
echo ""
echo "👻 The Ghost awaits..."
