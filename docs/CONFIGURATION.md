# OS Ghost Configuration Guide

This guide details the extensive configuration options available in OS Ghost.

## Configuration File

The primary configuration file is located at:

- **macOS**: `~/.config/os-ghost/config.json`
- **Windows**: `%APPDATA%\os-ghost\config.json`
- **Linux**: `~/.config/os-ghost/config.json`

## AI Providers

### Google Gemini (Cloud)

Primary provider for high-quality vision analysis.

```json
{
  "gemini_api_key": "your-api-key-here"
}
```

### Ollama (Local)

Fallback or privacy-focused local provider.

```json
{
  "ollama_url": "http://localhost:11434",
  "ollama_vision_model": "llava",
  "ollama_text_model": "llama3"
}
```

## System Settings

### General

- **check_updates**: (Boolean) Automatically check for updates on startup.
- **start_hidden**: (Boolean) Start app minimized to tray.

### Privacy & Security

- **read_only_mode**: (Boolean) If true, the Ghost will not perform any active actions (clicks, typing).
- **capture_consent**: (Boolean) User consent for screen analysis.
- **ai_analysis_consent**: (Boolean) User consent for sending data to AI.

### Autonomy

- **auto_puzzle_solver**: (Boolean) Allow the Ghost to attempt solving puzzles automatically.
- **max_autonomy_level**: (Integer) 0-5 scale of allowed autonomy.

## Sandbox Settings

The sandbox restricts what the Ghost can do.

- **allowed_domains**: List of domains the Ghost can interact with.
- **file_system_access**: (Boolean) Allow access to specific directories.

## Menu Configuration

Most settings can also be toggled via the application menu:

- **Settings > Privacy**: Toggle read-only mode and consents.
- **Settings > Autonomy**: Configure puzzle solving behavior.
- **Settings > Visual Automation**: Calibrate screen capture areas.
