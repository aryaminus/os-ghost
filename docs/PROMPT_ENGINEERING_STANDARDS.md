# OS Ghost Prompt Engineering Standards

**Based on:** Hera Agentic Design Patterns (Appendix A & F)
**Scope:** All agent prompts within `src-tauri/src/agents/`

---

## 1. Core Principles

All prompts in `os-ghost` must adhere to the following principles to ensure reliability, safety, and intelligence.

### A. Clarity & Specificity
*   **Rule:** Do not assume the model "knows" the game context. Always inject the `AgentContext` explicitely.
*   **Bad:** "Give me a hint."
*   **Good:** "You are the Narrator. The user is currently viewing [URL]. The target is [TARGET]. Provide a cryptic hint related to [KEYWORD]."

### B. Structured Output (JSON)
*   **Rule:** Agents must return machine-parseable output (JSON) whenever possible, not free text. This allows the `Orchestrator` to programmatically handle the response.
*   **Implementation:** Use `serde_json` schemas in the system prompt.
*   **Pattern:**
    ```text
    Response must be valid JSON:
    {
      "thought_process": "...",
      "action": "...",
      "confidence": 0.0 to 1.0
    }
    ```

### C. System 2 Thinking (Chain of Thought)
*   **Rule:** For complex tasks (Puzzle Generation, Verification, Clue Analysis), force the model to reason *before* answering.
*   **Pattern:** Use the "Thought/Action" loop or explicit "Let's think step by step" instruction.
*   **Example:**
    > "First, analyze the user's current page content against the puzzle criteria. Second, evaluate the semantic similarity. Finally, determine if the puzzle is solved."

---

## 2. Standard Prompt Templates

### A. The "Reasoning" Prompt (for Verifier/Observer)
Used when the agent needs to make a logical decision (e.g., Is this puzzle solved?).

```rust
format!(r#"
You are the {agent_name}.
Context: {context_json}

INSTRUCTIONS:
1. Analyze the input data step-by-step.
2. Identify key entities and patterns.
3. Compare against the success criteria.
4. Output your reasoning trace followed by the final verdict.

FORMAT:
{{
  "reasoning": "Step 1: ..., Step 2: ...",
  "verdict": true/false,
  "confidence": 0.95
}}
"#)
```

### B. The "Persona" Prompt (for Narrator)
Used when the agent needs to generate immersive content.

```rust
format!(r#"
ROLE: You are the Ghost in the machine.
MOOD: {current_mood} (e.g., Mysterious, Urgent, Playful)

TASK: Generate a response to the user's recent action: {action}.

CONSTRAINTS:
- Max length: 150 chars.
- Tone: Ethereal, Glitchy, Cryptic.
- NO help text or explanations. Only the character's voice.
"#)
```

---

## 3. Advanced Techniques (to be implemented)

### A. Self-Correction (Reflection)
Before returning a final answer, complex agents should be prompted to critique their own output.
> "Review your generated clue. Is it too obvious? If so, rewrite it to be more subtle."

### B. Few-Shot Prompting
Do not rely on Zero-Shot for complex logic. Provide 1-3 examples of "Perfect" inputs and outputs in the system prompt.

### C. Context Pruning
To save context window (and cost), redact PII and irrelevant HTML boilerplate *before* sending the prompt. (See `privacy.rs`).

---

## 4. Security Guardrails

1.  **PII Redaction:** All URLs and Page Content must pass through `privacy::redact_pii()` before entering the prompt.
2.  **Prompt Injection:** Do not concatenate user input directly into control statements. Use delimited blocks (e.g., `<user_input>{input}</user_input>`).
3.  **Output Validation:** Always validate that the JSON output matches the expected schema before acting on it.

---

**Ref:** `hera/docs/references/agentic-design-patterns/Appendix A_ Advanced Prompting Techniques.md`
