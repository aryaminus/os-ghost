# Agentic Architecture Review: OS Ghost vs. Hera Design Patterns

**Date:** January 09, 2026
**Scope:** Evaluation of `os-ghost/src-tauri/src` against `hera/docs/references/agentic-design-patterns`
**Status:** Comprehensive Analysis

---

## 1. Executive Summary

The **OS Ghost** application currently implements a robust **Reactive Agentic Architecture**. It excels at "Level 2" agentic behaviors (Routing, Chaining, Memory, Tool Use) but has not yet evolved into a "Level 3" **Autonomous Agent** (Planning, Reflection, Self-Correction).

The system successfully creates an immersive "Game Monitor" that watches the user and reacts with context-aware dialogue. However, it lacks the **Reasoning Engine** required to actively *guide* the user or solve complex problems autonomously. The current workflows are static pipelines rather than dynamic plans.

**Verdict:** The foundation is solid and modular. The "Body" (Perception/Action) and "Memory" are well-implemented. The "Brain" (Planning/Reflection) is the primary gap preventing this from being a fully agentic system as defined in the Hera documentation.

---

## 2. Pattern Compliance Matrix

We evaluated the codebase against the core patterns defined in *Agentic Design Patterns*.

| Pattern | Implementation in `os-ghost` | Status | Notes |
| :--- | :--- | :--- | :--- |
| **Chaining** | `workflow/sequential.rs` | ✅ **Excellent** | Clear `SequentialWorkflow` pipelines (Observer → Verifier → Narrator). |
| **Routing** | `ai_provider.rs` (`SmartAiRouter`) | ✅ **Excellent** | Intelligent routing between Gemini (Cloud) and Ollama (Local) based on task complexity and availability. |
| **Tool Use** | `verifier.rs`, `capture.rs` | 🟢 **Good** | Agents use "tools" (Regex, Screen Capture). However, the LLM itself does not *decide* which tool to use (Function Calling is static). |
| **Memory** | `memory/` (`LongTerm`, `Session`) | ✅ **Excellent** | Robust implementation of Short-term (Session) and Long-term (Sled DB) memory. Context is injected into prompts effectively. |
| **Parallelization** | `workflow/parallel.rs` | 🟢 **Good** | Supports concurrent execution (e.g., Background Monitors). |
| **Planning** | *None* | 🔴 **Missing** | Workflows are hardcoded (`create_puzzle_pipeline`). The agent cannot generate its own multi-step plan to solve a novel problem. |
| **Reflection** | *Partial* (`workflow/loop.rs`) | 🟡 **Partial** | `LoopWorkflow` exists for polling, but there is no **Generator-Critic** loop where the agent evaluates and improves its own output. |
| **Reasoning** | *Implicit only* | 🟡 **Weak** | Prompts are mostly Zero-shot. No implementation of **Chain-of-Thought (CoT)** or **ReAct** loops for complex logic. |

---

## 3. Deep Dive: The Reasoning Engine

### Current State (`ai_provider.rs`)
The `SmartAiRouter` is a sophisticated **Model Router**. It successfully abstracts the underlying LLM providers.
- **Routing Logic:**
    - `Light` tasks (Dialogue) → Ollama (Cost/Speed)
    - `Heavy` tasks (Puzzle Gen, Vision) → Gemini (Quality)
- **Fallback:** Automatic circuit breakers are implemented.

### The Gap (vs. Appendix F)
*Appendix F* describes the "Reasoning Engine" as a system that deconstructs prompts, retrieves knowledge, and structures answers.
- **Missing CoT:** The current prompts in `narrator.rs` and `observer.rs` are direct instruction prompts. They do not force the model to "Think step by step" (Chain of Thought) before acting.
- **Missing Meta-Cognition:** The system accepts the first output from the LLM. It does not use **Self-Consistency** (sampling multiple times) or **Reflection** (critiquing the output) to ensure quality.

---

## 4. Gap Analysis: The Missing "Cortex"

To align with the Hera reference architecture, `os-ghost` requires two major architectural additions:

### A. The Planner (Chapter 6)
**Current:** `AgentOrchestrator` runs a fixed sequence: `Observer -> Verifier -> Narrator`.
**Ideal:** A **Planner Agent** runs *before* the loop.
1.  **Input:** User's current context + Puzzle Goal.
2.  **Process:** Planner generates a sequence of *Actions*.
    *   *Example:* "1. Check Browser URL. 2. If Wikipedia, check for 'Alan Turing'. 3. If found, trigger 'Success' dialogue."
3.  **Execution:** The Orchestrator executes this dynamic plan.

### B. The Critic (Chapter 4)
**Current:** `Narrator` generates dialogue and sends it immediately to the UI.
**Ideal:** A **Reflection Loop**.
1.  **Generator:** Narrator drafts a message.
2.  **Critic:** A lightweight agent (or system prompt) evaluates: "Is this too vague? Is it in character? Is it safe?"
3.  **Refinement:** If rejected, Narrator regenerates with feedback.

---

## 5. Recommendations & Roadmap

### Phase 1: Enhanced Reasoning (Prompt Engineering)
*   **Action:** Update `agents/narrator.rs` and `agents/observer.rs` to use **Chain-of-Thought (CoT)** prompting.
*   **Implementation:** Inject "Let's think step by step:" into the system prompts. Parse the "Reasoning" section separately from the "Final Answer".
*   **Reference:** *Appendix A: Advanced Prompting Techniques*.

### Phase 2: The Self-Correction Loop (Reflection)
*   **Action:** Implement `ReflectiveAgent` wrapper in `agents/mod.rs`.
*   **Logic:**
    ```rust
    // Pseudo-code
    let draft = agent.generate(context).await;
    let critique = critic.evaluate(draft).await;
    if critique.score < threshold {
        return agent.regenerate(context, critique.feedback).await;
    }
    ```
*   **Reference:** *Chapter 4: Reflection*.

### Phase 3: Dynamic Planning (Autonomy)
*   **Action:** Create a `PlannerAgent` that outputs a JSON list of `NextAction` steps.
*   **Integration:** The `AgentOrchestrator` should consume this plan and dynamically construct the `SequentialWorkflow`.
*   **Reference:** *Chapter 6: Planning*.

---

## 6. Conclusion

`os-ghost` is a well-engineered application with a strong foundation in **System 1** thinking (Fast, Reactive, Pattern-Matching). To become a true "Agent" as defined in the Hera docs, it must implement **System 2** thinking (Slow, Deliberate, Planned, Reflective).

The existing `SmartAiRouter` and `Memory` systems provide the perfect infrastructure to support these advanced capabilities without significant refactoring of the core logic.
