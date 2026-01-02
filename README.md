# 👻 The OS Ghost

A screen-aware meta-game where an AI entity lives in your desktop, transforming your browser into an interactive puzzle box.

## Features

- **Transparent overlay** - Ghost floats above your desktop
- **Browser integration** - Chrome extension tracks navigation
- **AI-powered** - Gemini Vision analyzes your screen
- **Hot/cold feedback** - Get closer to solving mysteries
- **Persistent memory** - Progress saves between sessions

## Quick Start

### Prerequisites

- Node.js 18+
- Rust (via rustup)
- Chrome browser
- Gemini API key

### Installation

```bash
# Clone and enter directory
cd os-ghost

# Install dependencies
npm install

# Set your API key
cp .env.example .env
# Edit .env and add your GEMINI_API_KEY

# Run in development
npm run tauri dev
```

### Chrome Extension Setup

1. Open `chrome://extensions`
2. Enable **Developer mode** (top right)
3. Click **Load unpacked**
4. Select `ghost-extension/` folder
5. Note the Extension ID

### Register Native Messaging

```bash
./scripts/install.sh
# Enter your Extension ID when prompted
```

## Project Structure

```
os-ghost/
├── src/                    # React frontend
│   ├── components/Ghost.jsx
│   └── hooks/useTauriCommands.js
├── src-tauri/              # Rust backend
│   └── src/
│       ├── lib.rs          # Entry point
│       ├── window.rs       # Overlay window
│       ├── capture.rs      # Screen capture
│       ├── history.rs      # Chrome history
│       ├── ai_client.rs    # Gemini API
│       └── bridge.rs       # Native messaging
├── ghost-extension/        # Chrome extension
├── config/                 # Puzzles & narrative
└── scripts/                # Installation
```

## How to Play

1. Start the app - Ghost appears on your desktop
2. Read the clue in the Ghost's dialogue box
3. Browse the web to find the answer
4. Watch the proximity indicator heat up as you get closer
5. Find the correct page to unlock the next memory fragment

## Configuration

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `GEMINI_API_KEY` | Yes | Google Gemini API key |

### Puzzles

Edit `config/puzzles.json` to customize puzzles.

## Development

```bash
# Run Rust tests
cd src-tauri && cargo test

# Build for production
npm run tauri build
```

## License

MIT
