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

### Multi-Agent System

The system is composed of specialized agents (`src-tauri/src/agents/`):

- **PuzzleAgent**: Analyzes clues and web content to solve puzzles.
- **NavigationAgent**: Tracks browser history and context.
- **SystemAgent**: Monitors OS-level events.

## Model Context Protocol (MCP)

OS Ghost implements MCP abstractions (`src-tauri/src/mcp/`) to standardize how the agents interact with external tools and resources, making it extensible and compatible with the broader agentic ecosystem.

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
