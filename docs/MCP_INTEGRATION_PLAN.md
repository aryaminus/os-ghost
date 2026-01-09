# MCP Integration Plan

**Based on:** Hera Agentic Design Patterns (Chapter 10: Model Context Protocol)
**Target:** Modernizing the `os-ghost` Bridge Architecture

---

## 1. The Problem: "Siloed Tools"

Currently, `os-ghost` interacts with the browser via a custom TCP bridge (`bridge.rs`).
*   **Pros:** Fast, currently working.
*   **Cons:** Non-standard. Only works for Chrome. Hard to extend to other apps (VS Code, Terminal, File System). The Agent cannot "ask" the browser questions; it only receives a stream of events.

## 2. The Solution: Model Context Protocol (MCP)

Adopting MCP transforms the browser from a "Event Stream" into a "Server" that the Agent can query.

### Architecture Shift

| Feature | Current (`bridge.rs`) | Proposed (MCP) |
| :--- | :--- | :--- |
| **Communication** | Custom TCP (JSON) | JSON-RPC (Stdio/SSE) |
| **Discovery** | Hardcoded | `mcp_client.discover_capabilities()` |
| **Interaction** | Push (Browser -> Agent) | Query (Agent <-> Browser) |
| **Data Model** | `BrowserMessage` struct | `Resource` (Page), `Tool` (GetContent) |

---

## 3. Implementation Roadmap

### Phase 1: The "MCP-Lite" Adapter
Instead of rewriting the Chrome Extension immediately, we wrap the existing `bridge.rs` in an internal MCP interface.

1.  **Define Resources:**
    *   `browser://current_tab` (The active page content)
    *   `browser://history/recent` (Last 10 sites)
2.  **Define Tools:**
    *   `get_page_content(url)`
    *   `capture_screenshot()`
3.  **Refactor `AgentOrchestrator`:**
    *   Instead of listening for raw TCP events, the Orchestrator instantiates an `McpClient`.
    *   When the generic "Navigation" event fires, the Agent *decides* whether to call `get_page_content`.

### Phase 2: Full MCP Server
Create a standalone `os-ghost-browser-server` (Node.js/Python) that implements the official MCP spec.
*   This server connects to the Chrome Extension.
*   The Rust Tauri app connects to this server as an **MCP Client**.

### Phase 3: Ecosystem Expansion
Once the `AgentOrchestrator` speaks MCP, we can easily plug in other standard servers:
*   **File System MCP:** Allow the ghost to read "clue files" on the desktop.
*   **Terminal MCP:** Allow the ghost to see if the user is running specific commands.

---

## 4. Immediate Action Items

1.  **Update `agents/traits.rs`**: Add `Tools` capability to the `AgentContext`.
2.  **Create `mcp_client.rs`**: A Rust module to handle JSON-RPC communication (see *Chapter 10*).
3.  **Refactor `bridge.rs`**: Keep the TCP connection, but format the internal data as MCP `Resources` before passing them to the Memory/Agents.

---

**Ref:** `hera/docs/references/agentic-design-patterns/Chapter 10_ Model Context Protocol (MCP).md`
