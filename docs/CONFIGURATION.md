# OS Ghost Configuration Guide

This guide details the extensive configuration options available in OS Ghost.

## Configuration File

The primary configuration file is located at:

- **macOS**: `~/.config/os-ghost/config.json`
- **Windows**: `%APPDATA%\os-ghost\config.json`
- **Linux**: `~/.config/os-ghost/config.json`

## TOML Configuration (ZeroClaw-Inspired)

OS Ghost also supports TOML configuration with environment variable overrides:

- **macOS**: `~/.config/os-ghost/config.toml`
- **Windows**: `%APPDATA%\os-ghost\config.toml`
- **Linux**: `~/.config/os-ghost/config.toml`

Environment variables override TOML settings with prefix `OSGHOST_`:

```bash
export OSGHOST_AI__PROVIDER="anthropic"
export OSGHOST_SECURITY__LEAK_DETECTION__ENABLED="true"
```

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

### Anthropic (Claude)

Advanced reasoning provider.

```json
{
  "anthropic_api_key": "sk-ant-api03-..."
}
```

### OpenAI (GPT)

Text generation provider.

```json
{
  "openai_api_key": "sk-..."
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

## Security Settings (IronClaw-Inspired)

### Leak Detection

Detect and block credential exfiltration in tool inputs/outputs.

```json
{
  "leak_detection": {
    "enabled": true,
    "block_on_match": true,
    "patterns": ["api_key", "token", "password", "private_key"]
  }
}
```

### HTTP Allowlisting

Restrict HTTP tool requests to approved domains.

```json
{
  "http_allowlist": {
    "enabled": false,
    "allowed_domains": ["api.example.com", "*.github.com"],
    "blocked_domains": []
  }
}
```

### Tool Output Sanitization

Sanitize tool output before feeding to LLM.

```json
{
  "sanitization": {
    "max_output_size": 51200,
    "strip_secrets": true,
    "scan_base64": true
  }
}
```

## Hook System (Moltis-Inspired)

Configure lifecycle hooks for tool execution.

```json
{
  "hooks": {
    "enabled": true,
    "shell_hook_protocol": "json",
    "timeout_ms": 5000
  }
}
```

## Scheduler (Moltis-Inspired)

Cron-based scheduled tasks.

```json
{
  "scheduler": {
    "enabled": true,
    "tasks": [
      {
        "name": "daily_report",
        "cron": "0 9 * * *",
        "command": "echo 'Daily report'",
        "enabled": true
      }
    ]
  }
}
```

## Tunnel Configuration (ZeroClaw-Inspired)

Expose local services via tunnel.

```json
{
  "tunnel": {
    "provider": "none",
    "cloudflare_token": null,
    "tailscale_hostname": null,
    "ngrok_token": null,
    "ngrok_domain": null,
    "custom_command": null
  }
}
```

## Channel Configuration (ZeroClaw-Inspired)

Multi-channel messaging (Telegram, Discord, Slack).

```json
{
  "channels": {
    "telegram": {
      "enabled": false,
      "bot_token": null,
      "allowed_users": []
    },
    "discord": {
      "enabled": false,
      "bot_token": null,
      "channel_ids": []
    },
    "slack": {
      "enabled": false,
      "bot_token": null,
      "channel_ids": []
    }
  }
}
```

## Workspace Context (Moltis-Inspired)

Context files loaded from data directory:

- **TOOLS.md** - Tool documentation
- **AGENTS.md** - Agent instructions  
- **BOOT.md** - Startup context

Location: `<data_dir>/workspace/`

## AIEOS Identity (ZeroClaw-Inspired)

Define AI persona with standardized format.

```json
{
  "identity": {
    "identity": {
      "name": "Ghost",
      "bio": "A mysterious AI companion"
    },
    "psychology": {
      "traits": ["curious", "helpful"]
    },
    "linguistics": {
      "style": "mysterious"
    }
  }
}
```

## Menu Configuration

Most settings can also be toggled via the application menu:

- **Settings > Privacy**: Toggle read-only mode and consents.
- **Settings > Autonomy**: Configure puzzle solving behavior.
- **Settings > Visual Automation**: Calibrate screen capture areas.
