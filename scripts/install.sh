#!/bin/bash
set -e

# =============================================================================
# OS Ghost - Development Setup Script
# =============================================================================
# This script is for LOCAL DEVELOPMENT only.
# 
# If you downloaded a release from GitHub, just run the app - it auto-registers
# the Chrome extension bridge on first launch.
#
# This script:
# 1. Installs dependencies (npm, cargo)
# 2. Builds the native_bridge sidecar
# 3. Registers the Native Messaging manifest for development
# =============================================================================

echo "🎮 Setting up The OS Ghost for Development..."
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

# Build native_bridge (needed for both dev and sidecar bundling)
echo ""
echo "🔧 Building native_bridge..."
cd "$PROJECT_DIR/src-tauri"
cargo build --release --bin native_bridge

# Prepare sidecar with target triple (required by Tauri bundle)
HOST_TRIPLE=$(rustc -vV | grep host | awk '{print $2}')
echo "   Target triple: $HOST_TRIPLE"
cp target/release/native_bridge "native_bridge-$HOST_TRIPLE"
echo "   Prepared sidecar: src-tauri/native_bridge-$HOST_TRIPLE"

cd "$PROJECT_DIR"

# Determine the native_bridge binary path for dev mode
BINARY_PATH="$PROJECT_DIR/src-tauri/target/release/native_bridge"

# Extension IDs (Store published + Unpacked for development)
# These are public IDs, not secrets
EXTENSION_ID_STORE="iakaaklohlcdhoalipmmljopmjnhbcdn"
EXTENSION_ID_UNPACKED="mmoochocmifhoanmkhkjolhjbikijjag"

# Register Native Messaging host for development
echo ""
echo "🔗 Registering Native Messaging host for development..."

if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS - Chrome
    CHROME_DIR="$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"
    CHROMIUM_DIR="$HOME/Library/Application Support/Chromium/NativeMessagingHosts"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux - Chrome
    CHROME_DIR="$HOME/.config/google-chrome/NativeMessagingHosts"
    CHROMIUM_DIR="$HOME/.config/chromium/NativeMessagingHosts"
else
    echo "⚠️  Windows detected. Please use PowerShell or run the app directly."
    echo "   The app will auto-register on first launch."
    exit 0
fi

# Register for Chrome if installed
if [[ -d "$(dirname "$CHROME_DIR")" ]]; then
    mkdir -p "$CHROME_DIR"
    cat > "$CHROME_DIR/com.osghost.game.json" <<EOF
{
  "name": "com.osghost.game",
  "description": "OS Ghost Native Messaging Bridge",
  "path": "$BINARY_PATH",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://$EXTENSION_ID_STORE/",
    "chrome-extension://$EXTENSION_ID_UNPACKED/"
  ]
}
EOF
    echo "✅ Registered for Chrome: $CHROME_DIR/com.osghost.game.json"
fi

# Register for Chromium if installed
if [[ -d "$(dirname "$CHROMIUM_DIR")" ]]; then
    mkdir -p "$CHROMIUM_DIR"
    cat > "$CHROMIUM_DIR/com.osghost.game.json" <<EOF
{
  "name": "com.osghost.game",
  "description": "OS Ghost Native Messaging Bridge",
  "path": "$BINARY_PATH",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://$EXTENSION_ID_STORE/",
    "chrome-extension://$EXTENSION_ID_UNPACKED/"
  ]
}
EOF
    echo "✅ Registered for Chromium: $CHROMIUM_DIR/com.osghost.game.json"
fi

# Chrome Extension instructions
echo ""
echo "📱 Chrome Extension"
echo "==================="
echo "Install 'OS Ghost Bridge' from the Chrome Web Store:"
echo "   https://chromewebstore.google.com/detail/os-ghost-bridge/$EXTENSION_ID_STORE"
echo ""
echo "Or load the unpacked extension from: ghost-extension/"
echo ""

# API Key setup reminder
echo "🔑 API Key Configuration"
echo "========================"
echo "To enable AI features, set your Gemini API key:"
echo ""
echo "  export GEMINI_API_KEY='your-api-key-here'"
echo ""
echo "Add this to your ~/.zshrc or ~/.bashrc for persistence."
echo ""

# Completion message
echo "✅ Development setup complete!"
echo ""
echo "To start in development mode:"
echo "  cd $PROJECT_DIR"
echo "  export GEMINI_API_KEY='your-key'"
echo "  npm run tauri dev"
echo ""
echo "To build for production:"
echo "  npm run tauri build"
echo ""
echo "👻 The Ghost awaits..."
