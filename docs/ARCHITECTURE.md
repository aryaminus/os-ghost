# OS Ghost Architecture Guide

This document outlines the technical architecture of OS Ghost, a screen-aware meta-game and agentic companion.

## System Overview

The system consists of three main components communicating via a secure bridge:

1. **Tauri Backend (Rust)**: The core "brain" handling AI processing, system events, and state management.
2. **React Frontend**: The "face" of the Ghost, providing the UI overlay and visual feedback.
3. **Chrome Extension**: The "eyes" in the browser, tracking navigation and enabling web interactions.

```mermaid
graph TD
    A[Tauri Backend (Rust)] <-->|IPC| B[React Frontend]
    A <-->|Native Messaging| C[Chrome Extension]
    A -->|Screen Capture| D[Desktop Analysis]
    A -->|API Calls| E[AI Providers (Gemini/Ollama)]
```

## AI System

### Smart Router

The `SmartAiRouter` (`src-tauri/src/ai/ai_provider.rs`) intelligently routes requests between:

- **Gemini**: For complex visual analysis and reasoning.
- **Ollama**: For local, fast, or private tasks.
- **Anthropic**: Claude models for advanced reasoning.
- **OpenAI**: GPT models for text generation.

### Multi-Agent System

The system is composed of specialized agents (`src-tauri/src/agents/`):

- **PuzzleAgent**: Analyzes clues and web content to solve puzzles.
- **NavigationAgent**: Tracks browser history and context.
- **SystemAgent**: Monitors OS-level events.
- **Narrator**: Generates dialogue and narrative.
- **Observer**: Monitors user activity and context.
- **Critic**: Evaluates actions for safety and quality.
- **Planner**: Creates action plans.
- **Verifier**: Validates task completion.

## Model Context Protocol (MCP)

OS Ghost implements MCP abstractions (`src-tauri/src/mcp/`) to standardize how the agents interact with external tools and resources, making it extensible and compatible with the broader agentic ecosystem.

## Security (IronClaw-Inspired)

### Leak Detection

The security module (`src-tauri/src/security/leak_detector.rs`) scans tool inputs and outputs for potential credential exfiltration using 20+ pattern detectors:

- API keys (OpenAI, Anthropic, GitHub, AWS, etc.)
- Tokens (JWT, OAuth, session tokens)
- Private keys (SSH, PGP)
- Passwords in config files

### HTTP Allowlisting

Domain and path allowlisting (`src-tauri/src/security/http_allowlist.rs`) restricts tool HTTP access to approved endpoints, preventing data exfiltration.

### Tool Output Sanitization

All tool output is sanitized (`src-tauri/src/mcp/sanitization.rs`) before being fed back to the LLM:
- Secrets stripped
- Base64 decoded content scanned
- Large outputs truncated

## Hook System (Moltis-Inspired)

The plugin/hook system (`src-tauri/src/plugins/hooks.rs`) provides lifecycle events:

- `BeforeToolCall` - Validate/modify tool invocation
- `AfterToolCall` - Process tool results
- `OnError` - Handle tool failures
- `OnCircuitBreakerOpen` - Respond to repeated failures

## Workspace Context (Moltis-Inspired)

Context files (`src-tauri/src/data/workspace_context.rs`) are loaded from the data directory:

- **TOOLS.md** - Tool documentation and policies
- **AGENTS.md** - Agent instructions
- **BOOT.md** - Startup context

These are injected into agent prompts for workspace-specific behavior.

## Scheduler & Heartbeat

The scheduler module (`src-tauri/src/scheduler/`) provides:

- **Cron-based scheduled tasks** - Run commands on schedule
- **Heartbeat engine** - Periodic task execution
- **Daemon management** - Monitor component health

## Tunnel Integration (ZeroClaw-Inspired)

Tunnel support (`src-tauri/src/server/tunnel/`) exposes local services:

- **Cloudflare Tunnel** - Cloudflare Zero Trust
- **Tailscale** - Mesh VPN
- **ngrok** - Simple tunnel

## Channels (ZeroClaw-Inspired)

Multi-channel messaging (`src-tauri/src/channels/`):

- **Telegram** - Bot API integration
- **Discord** - Bot integration
- **Slack** - App integration

## Advanced Memory (HermitClaw-Inspired)

Memory system (`src-tauri/src/memory/advanced.rs`) enhanced with:

- **Three-Factor Retrieval**: Scores memories by recency + importance + relevance
- **Reflection Hierarchy**: Synthesizes insights when cumulative importance exceeds threshold (50)
- **Mood System**: Autonomous behavior modes (Research, DeepDive, Coder, Writer, Explorer, Organizer)
- **Focus Mode**: Task-locked operation - Ghost ignores autonomous behaviors until complete
- **Personality Genome**: Entropy-based unique identity generation from keyboard input

## Hybrid Memory (ZeroClaw-Inspired)

Memory system (`src-tauri/src/memory/hybrid.rs`) combines:

- **SQLite** - Structured data
- **FTS5** - Full-text search
- **Vector search** - Semantic similarity

## Workflow Export (DroidClaw-Inspired)

Workflow system (`src-tauri/src/workflow/mod.rs`) supports:

- **JSON Export**: Export recorded workflows to DroidClaw-compatible JSON format
- **Portable Workflows**: Share and reuse workflows across platforms

## File Drop Processing (HermitClaw-Inspired)

File watcher (`src-tauri/src/data/file_drop.rs`) provides:

- **Directory Monitoring**: Watches for new files in workspace
- **Auto-Processing**: Detects and processes text files, images, PDFs
- **Supported Types**: txt, md, py, json, csv, pdf, png, jpg, gif, webp

## AIEOS Identity (ZeroClaw-Inspired)

AI Entity Object Specification (`src-tauri/src/data/identity.rs`) provides standardized persona definition:

- Identity (names, bio, origin)
- Psychology (traits, MBTI, moral compass)
- Linguistics (text style, formality)
- Motivations (goals, fears)
- **Personality Genome**: Entropy-based unique identity from keyboard input (HermitClaw-inspired)

## IPC & Events

The system relies on a robust event bus:

- **Frontend** emits `intent` events.
- **Backend** processes intents and emits `state_update` events.
- **Extension** sends `navigation` and `dom_snapshot` events via the Native Bridge.

## Privacy & Security

### Sandbox

All AI actions are sandboxed. The `read_only_mode` acts as a hard kill-switch for any active interference (clicking, typing).

### Capture

Screen capture is only performed when:

1. User has explicitly consented.
2. The Ghost is in an "active" state requiring vision.
