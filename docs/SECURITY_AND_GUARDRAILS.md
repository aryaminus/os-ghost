# Security & Guardrails Strategy

**Based on:** Hera Agentic Design Patterns (Chapter 18)
**Context:** OS Ghost Application

---

## 1. Safety Philosophy

"Guardrails are not just restrictions; they are the definition of a reliable agent."
In `os-ghost`, where the agent observes the user's private desktop, **Safety** and **Privacy** are the primary features, not secondary requirements.

---

## 2. Layered Defense Architecture

We implement safety at three distinct layers:

### Layer 1: Input/Output Filtering (The "Reflex" Layer)
*   **Location:** `src-tauri/src/privacy.rs`
*   **Mechanism:** Regex-based PII Redaction.
*   **Scope:** 
    *   Scrub Emails, Phone Numbers, IPs, and Credit Cards from **all** prompts before they leave the device.
    *   **Rule:** If `privacy::redact_pii()` has not been called, the `SmartAiRouter` must refuse to send the request.

### Layer 2: The Semantic Guardrail (The "Judge" Layer)
*   **Location:** `src-tauri/src/ai_provider.rs` (Proposed)
*   **Mechanism:** A lightweight, local LLM (Ollama) acts as a dedicated "Safety Agent."
*   **Prompt:**
    ```text
    Role: Safety Officer.
    Input: [Agent Response]
    Task: Check for:
    1. Dangerous instructions.
    2. Revealing private user data (passwords, keys).
    3. Hostile/Toxic tone.
    Output: "SAFE" or "UNSAFE: [Reason]"
    ```
*   **Action:** If UNSAFE, the `Orchestrator` suppresses the message and logs the incident.

### Layer 3: Behavioral Constraints (The "Persona" Layer)
*   **Location:** System Prompts (`narrator.rs`, `observer.rs`)
*   **Mechanism:** "Instructions Over Constraints" (Appendix A).
*   **Implementation:**
    *   Instead of "Do not be rude," use "Maintain a helpful, mysterious, and respectful tone."
    *   Instead of "Do not leak data," use "Focus strictly on the puzzle clues found in the public web content."

---

## 3. Privacy-First "Blind" Mode

To ensure user trust, `os-ghost` operates on a **Need-to-Know** basis (Principle of Least Privilege).

*   **Window Filtering:** The `ObserverAgent` should filter out sensitive window titles (e.g., "Password Manager", "Bank", "Incognito") *before* capturing screenshots.
*   **Local-First Processing:** 
    *   Image Analysis preferences: **Ollama (Local)** > **Gemini (Cloud)**.
    *   If the user has not explicitly opted-in to Cloud AI, `SmartAiRouter` **must** enforce the Local circuit.

---

## 4. Implementation Checklist

- [ ] **Audit `privacy.rs`**: Ensure Regex covers SSNs and API Keys (sk_live...).
- [ ] **Update `SmartAiRouter`**: Add a `pre_flight_check()` that verifies PII redaction.
- [ ] **Create `SafetyAgent`**: A specialized agent in the `Orchestrator` that runs *after* the `Narrator` but *before* the UI update.
- [ ] **MCP Compliance**: Ensure any future tool integrations (File System, etc.) have explicit "Allow/Deny" permission prompts for the user.

---

**Ref:** `hera/docs/references/agentic-design-patterns/Chapter 18_ Guardrails_Safety Patterns.md`
