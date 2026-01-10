# IPC Layer Audit Report (2024 Senior Research Audit)

## 1. Executive Summary

This audit focused on the `/src-tauri/src/ipc` directory, which acts as the interface between the frontend and the Rust backend.

**Overall Health:** Improved (Partial Refactor Complete)
**Architecture:** `mod.rs` acts as the main registry, with domain logic slowly being moved to submodules.
**Key Achievements:**
- **Puzzle Logic Extracted:** All puzzle generation, sponsorship logic, and history analysis commands have been successfully moved to `src/ipc/puzzles.rs`. This reduced `mod.rs` complexity by ~20%.
- **Module Structure Established:** The directory structure `ipc/mod.rs` + `ipc/puzzles.rs` is now compliant with Rust 2018+ module standards.

---

## 2. Code Organization

### 2.1 Current Structure
| File | Lines | Responsibilities |
|------|-------|------------------|
| `mod.rs` | ~1000 | **Registry**: Exports commands.<br>**System**: Chrome detection, API Keys.<br>**Orchestration**: `process_agent_cycle`, `enable_autonomous_mode`.<br>**HITL**: Feedback commands. |
| `puzzles.rs` | ~300 | **Domain**: Puzzle structs (`Puzzle`, `GeneratedPuzzle`).<br>**Logic**: `start_investigation` (AI mystery generation).<br>**State**: managing the `puzzles` RwLock. |

### 2.2 Findings
*   **Hybrid AI Support**: The IPC layer explicitly handles routing between Gemini (cloud) and Ollama (local), exposing status checks (`get_ollama_status`) to the frontend.
*   **Privacy Awareness**: Commands like `generate_puzzle_from_history` call `crate::privacy::redact_pii` before sending data to the AI, ensuring user privacy is respected at the boundary.
*   **Macro Limitations**: The `tauri::generate_handler!` macro requires fully qualified paths to the *defining* module (e.g., `ipc::puzzles::start_investigation`), not just re-exports. This was identified and fixed during the refactor.

---

## 3. Refactoring Roadmap

To fully modernize the IPC layer, the remaining "God Object" (`mod.rs`) should be split into focused controllers.

### 3.1 Proposed Modules

1.  **`src/ipc/system.rs`**
    *   *Commands*: `detect_chrome`, `launch_chrome`, `check_api_key`, `set_api_key`, `validate_api_key`.
    *   *Purpose*: OS-level interactions and credentials.

2.  **`src/ipc/config.rs`**
    *   *Commands*: `get_ollama_config`, `set_ollama_config`, `reset_ollama_config`, `get_ollama_status`.
    *   *Purpose*: Managing local AI settings and persistence.

3.  **`src/ipc/autonomous.rs`**
    *   *Commands*: `enable_autonomous_mode`, `start_background_checks`.
    *   *Structs*: `AutonomousTask`, `AutonomousProgress`.
    *   *Purpose*: Long-running background agents and thread management.

4.  **`src/ipc/hitl.rs`** (Human-in-the-Loop)
    *   *Commands*: `submit_feedback`, `submit_escalation`, `resolve_escalation`, `get_player_stats`.
    *   *Purpose*: Feedback loops for long-term memory.

### 3.2 Future `lib.rs` Example
After full refactoring, the handler registration in `lib.rs` would look cleaner:

```rust
.invoke_handler(tauri::generate_handler![
    // System
    ipc::system::detect_chrome,
    ipc::system::set_api_key,
    
    // Config
    ipc::config::get_ollama_config,
    
    // Puzzles (Done)
    ipc::puzzles::start_investigation,
    
    // Autonomous
    ipc::autonomous::enable_autonomous_mode,
    
    // HITL
    ipc::hitl::submit_feedback,
])
```

## 4. Conclusion

The IPC layer is functional and safe, but requires further subdivision to remain maintainable as the application grows. The successful extraction of `puzzles.rs` proves the viability of this approach.

**Auditor:** OpenCode (Agentic AI Assistant)
**Date:** Jan 10, 2026
