//! Agent Orchestrator - Coordinates all agents
//! Central coordinator for the multi-agent system
//!
//! Enhanced with Planning and Reflection patterns:
//! - **Planning**: PlannerAgent generates sub-goals before the game loop
//! - **Reflection**: CriticAgent validates Narrator output in a Generator-Critic loop
//! - **Self-Correction**: LoopWorkflow tracks failed approaches for plan revision
//!
//! ADK Integration:
//! - **Callbacks**: Before/after hooks for agent execution
//! - **Monitoring**: InvocationMetrics for each orchestration cycle
//! - **Events**: EventStream for observability
//! - **ScopedState**: Temp state cleared at start of each invocation
//!
//! MCP Integration (Chapter 10):
//! - Supports dynamic tool discovery via `get_available_tools()`
//! - Can invoke browser tools through MCP interface
//! - Resources are accessible via MCP URIs

use super::callbacks::{CallbackContext, CallbackRegistry};
use super::critic::CriticAgent;
use super::events::{AgentEvent, EventActions, EventStream};
use super::guardrail::GuardrailAgent;
use super::narrator::NarratorAgent;
use super::observer::ObserverAgent;
use super::planner::PlannerAgent;
use super::traits::{
    Agent, AgentContext, AgentMode, AgentOutput, AgentResult, NextAction, PlanningContext,
};
use super::verifier::VerifierAgent;
use super::watchdog::WatchdogAgent;
use crate::ai_provider::SmartAiRouter;
use crate::mcp::{McpServer, ResourceDescriptor, ToolDescriptor};
use crate::memory::{LongTermMemory, SessionMemory};
use crate::monitoring::{InvocationMetrics, MetricsCollector};
use crate::workflow::{
    loop_agent::create_adaptive_loop, parallel::create_parallel_checks,
    planning::create_intelligent_pipeline, reflection::create_narrator_with_reflection,
    sequential::create_puzzle_pipeline, PlanningWorkflow, ReflectionWorkflow, SequentialWorkflow,
    Workflow,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::Mutex as AsyncMutex;

/// Shared session memory types
pub type SharedLongTermMemory = Arc<Mutex<LongTermMemory>>;
pub type SharedSessionMemory = Arc<Mutex<SessionMemory>>;

// Configuration constants
const AUTONOMOUS_LOOP_MAX_ITERATIONS: usize = 5;
const AUTONOMOUS_LOOP_DELAY_MS: u64 = 2000;
const PLANNED_LOOP_MAX_ITERATIONS: usize = 10;
const PLANNED_LOOP_DELAY_MS: u64 = 1500;

/// Atomic counter for unique invocation IDs
static INVOCATION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique invocation ID
fn generate_invocation_id() -> String {
    let counter = INVOCATION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("inv_{}", counter)
}

/// The main orchestrator that coordinates all agents
/// Enhanced with Planning, Reflection, and Guardrails capabilities
pub struct AgentOrchestrator {
    /// Legacy sequential workflow (Observer → Verifier → Narrator)
    workflow: SequentialWorkflow,
    /// Intelligent planning workflow (Planner → Observer → Verifier → Narrator)
    planning_workflow: PlanningWorkflow,
    /// Reflection workflow for quality control (Narrator + Critic loop)
    reflection_workflow: ReflectionWorkflow,
    /// Individual agent references
    narrator: Arc<NarratorAgent>,
    observer: Arc<ObserverAgent>,
    verifier: Arc<VerifierAgent>,
    planner: Arc<PlannerAgent>,
    critic: Arc<CriticAgent>,
    guardrail: Arc<GuardrailAgent>,
    /// Security watchdog agent
    watchdog: Arc<WatchdogAgent>,
    /// AI router reference for telemetry access
    ai_router: Arc<SmartAiRouter>,
    /// Memory stores
    session: SharedSessionMemory,
    long_term: SharedLongTermMemory,
    /// Agent mode - consolidated runtime toggle (thread-safe via AtomicU8)
    /// 0 = Legacy, 1 = Standard, 2 = Full, 3 = Minimal
    mode: AtomicU8,
    /// ADK-style callback registry for lifecycle hooks
    callbacks: Arc<AsyncMutex<CallbackRegistry>>,
    /// ADK-style metrics collector for monitoring
    metrics: Arc<MetricsCollector>,
}

/// Result of a full orchestration cycle
#[derive(Debug, Clone, serde::Serialize)]
pub struct OrchestrationResult {
    /// Combined dialogue/message
    pub message: String,
    /// Proximity score
    pub proximity: f32,
    /// Whether puzzle was solved
    pub solved: bool,
    /// Suggested hint index (if any)
    pub show_hint: Option<usize>,
    /// Ghost state (idle, thinking, searching, celebrate)
    pub ghost_state: String,
    /// All agent outputs for debugging
    pub agent_outputs: Vec<AgentOutput>,
}

impl AgentOrchestrator {
    /// Create a new orchestrator with all agents and shared memory
    /// Now includes PlannerAgent and CriticAgent for intelligent behavior
    pub fn new(
        ai_router: Arc<SmartAiRouter>,
        long_term: SharedLongTermMemory,
        session: SharedSessionMemory,
    ) -> anyhow::Result<Self> {
        // Create core agents
        let observer = Arc::new(ObserverAgent::new(Arc::clone(&ai_router)));
        let verifier = Arc::new(VerifierAgent::new());
        let narrator = Arc::new(NarratorAgent::new(Arc::clone(&ai_router)));

        // Create new intelligent agents
        let planner = Arc::new(PlannerAgent::new(Arc::clone(&ai_router)));
        let critic = Arc::new(CriticAgent::new(Arc::clone(&ai_router)));
        // Semantic PII detection is disabled by default; will be enabled dynamically in Full mode
        let guardrail = Arc::new(GuardrailAgent::new(Arc::clone(&ai_router)));
        let watchdog = Arc::new(WatchdogAgent::new(Arc::clone(&ai_router)));

        // Build legacy workflow pipeline: Observer -> Verifier -> Narrator
        let workflow = create_puzzle_pipeline(observer.clone(), verifier.clone(), narrator.clone());

        // Build intelligent planning workflow: Planner -> Observer -> Verifier -> Narrator
        let planning_workflow = create_intelligent_pipeline(
            planner.clone(),
            observer.clone() as Arc<dyn crate::agents::Agent>,
            verifier.clone() as Arc<dyn crate::agents::Agent>,
            narrator.clone() as Arc<dyn crate::agents::Agent>,
        );

        // Build reflection workflow: Narrator + Critic loop (max 3 iterations)
        let reflection_workflow = create_narrator_with_reflection(
            narrator.clone() as Arc<dyn crate::agents::Agent>,
            critic.clone(),
            3,
        );

        let callbacks = Arc::new(AsyncMutex::new(CallbackRegistry::new()));
        if let Ok(mut registry) = callbacks.try_lock() {
            registry.register_extension_tools();
        }

        Ok(Self {
            workflow,
            planning_workflow,
            reflection_workflow,
            narrator,
            observer,
            verifier,
            planner,
            critic,
            guardrail,
            watchdog,
            ai_router,
            session,
            long_term,
            mode: AtomicU8::new(AgentMode::Standard as u8), // Default to Standard mode
            callbacks,
            metrics: Arc::new(MetricsCollector::default()),
        })
    }

    // -------------------------------------------------------------------------
    // ADK-Style Accessors
    // -------------------------------------------------------------------------

    /// Get the callback registry for registering lifecycle hooks
    pub fn callbacks(&self) -> Arc<AsyncMutex<CallbackRegistry>> {
        Arc::clone(&self.callbacks)
    }

    /// Get the metrics collector for monitoring
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        Arc::clone(&self.metrics)
    }

    // -------------------------------------------------------------------------
    // Mode Getters/Setters (thread-safe)
    // -------------------------------------------------------------------------

    /// Get the current agent mode
    pub fn agent_mode(&self) -> AgentMode {
        match self.mode.load(Ordering::Relaxed) {
            0 => AgentMode::Legacy,
            1 => AgentMode::Standard,
            2 => AgentMode::Full,
            3 => AgentMode::Minimal,
            _ => AgentMode::Standard, // Default fallback
        }
    }

    /// Set the agent mode
    pub fn set_agent_mode(&self, mode: AgentMode) {
        self.mode.store(mode as u8, Ordering::Relaxed);
        tracing::info!("Agent mode set to: {:?}", mode);
    }

    /// Check if intelligent planning mode is enabled (derived from mode)
    pub fn use_intelligent_mode(&self) -> bool {
        self.agent_mode().use_planning()
    }

    /// Check if reflection is enabled (derived from mode)
    pub fn use_reflection(&self) -> bool {
        self.agent_mode().use_reflection()
    }

    /// Check if guardrails are enabled (derived from mode)
    pub fn use_guardrails(&self) -> bool {
        self.agent_mode().use_guardrails()
    }

    // -------------------------------------------------------------------------
    // Legacy Setters (for backward compatibility with IPC commands)
    // These convert individual flags to the closest matching AgentMode
    // -------------------------------------------------------------------------

    /// Enable or disable intelligent planning mode (legacy API)
    pub fn set_intelligent_mode(&self, enabled: bool) {
        let current = self.agent_mode();
        let new_mode =
            AgentMode::from_flags(enabled, current.use_reflection(), current.use_guardrails());
        self.set_agent_mode(new_mode);
    }

    /// Enable or disable reflection for narrator (legacy API)
    pub fn set_reflection(&self, enabled: bool) {
        let current = self.agent_mode();
        let new_mode =
            AgentMode::from_flags(current.use_planning(), enabled, current.use_guardrails());
        self.set_agent_mode(new_mode);
    }

    /// Enable or disable guardrails for safety (legacy API)
    pub fn set_guardrails(&self, enabled: bool) {
        let current = self.agent_mode();
        let new_mode =
            AgentMode::from_flags(current.use_planning(), current.use_reflection(), enabled);
        self.set_agent_mode(new_mode);
    }

    /// Get the planner agent for direct access
    pub fn planner(&self) -> &Arc<PlannerAgent> {
        &self.planner
    }

    /// Get the guardrail agent for direct access
    pub fn guardrail(&self) -> &Arc<GuardrailAgent> {
        &self.guardrail
    }

    /// Get the critic agent for direct access
    pub fn critic(&self) -> &Arc<CriticAgent> {
        &self.critic
    }

    // -------------------------------------------------------------------------
    // Telemetry Methods
    // -------------------------------------------------------------------------

    /// Get LLM call counts for cost tracking/transparency
    /// Returns (gemini_calls, ollama_calls) since session start or last reset
    pub fn get_llm_call_counts(&self) -> (u64, u64) {
        self.ai_router.get_call_counts()
    }

    /// Reset LLM call counters (e.g., at session start)
    pub fn reset_llm_call_counts(&self) {
        self.ai_router.reset_call_counts()
    }

    /// Run the full agent pipeline
    /// Uses intelligent planning workflow if enabled, otherwise legacy sequential
    /// Applies guardrails for input/output safety filtering
    /// 
    /// ADK Integration:
    /// - Clears temp: scoped state at start of each invocation
    /// - Runs before/after agent callbacks
    /// - Records InvocationMetrics for monitoring
    pub async fn process(
        &self,
        context: &AgentContext,
        mcp_server: Option<&crate::mcp::BrowserMcpServer>,
    ) -> AgentResult<OrchestrationResult> {
        let invocation_id = generate_invocation_id();
        let start_time = Instant::now();
        let workflow_name = if self.use_intelligent_mode() { "planning" } else { "sequential" };
        
        // ADK: Clear temp-scoped state at start of each invocation
        if let Ok(session) = self.session.lock() {
            if let Err(e) = session.clear_temp_state() {
                tracing::debug!("Failed to clear temp state: {}", e);
            }
        }

        // ADK: Run before_agent callbacks
        let callback_context = CallbackContext::new(context.clone(), "Orchestrator", &invocation_id);
        if let Some(override_output) = {
            let registry = self.callbacks.lock().await;
            registry.run_before_agent(&callback_context).await
        } {
            let override_message = override_output.result.clone();
            let override_confidence = override_output.confidence;
            let override_next_action = override_output.next_action.clone();
            let elapsed_ms = start_time.elapsed().as_millis() as u64;
            let metrics = InvocationMetrics::new(&invocation_id, "Orchestrator")
                .complete_success(elapsed_ms, override_confidence);
            self.metrics.record(metrics);

            return Ok(OrchestrationResult {
                message: override_message,
                proximity: override_confidence,
                solved: matches!(override_next_action.as_ref(), Some(NextAction::PuzzleSolved)),
                show_hint: override_next_action.as_ref().and_then(|action| {
                    if let NextAction::ShowHint(idx) = action {
                        Some(*idx)
                    } else {
                        None
                    }
                }),
                ghost_state: "idle".to_string(),
                agent_outputs: vec![override_output],
            });
        }

        // Apply input guardrails if enabled
        if self.use_guardrails() {
            // Check current URL and content for safety
            let url_safety = self
                .guardrail
                .evaluate_safety(
                    &context.current_url,
                    super::guardrail::ContentType::Url,
                    context,
                )
                .await?;
            if !url_safety.is_safe {
                tracing::warn!("Guardrail blocked URL: {}", context.current_url);
                // Record blocked metrics
                let metrics = InvocationMetrics::new(&invocation_id, "Orchestrator")
                    .complete_failure(start_time.elapsed().as_millis() as u64, "guardrail_blocked");
                self.metrics.record(metrics);
                return Ok(OrchestrationResult {
                    message: "The ghost senses something... unsettling. Let's move elsewhere."
                        .to_string(),
                    proximity: 0.0,
                    solved: false,
                    show_hint: None,
                    ghost_state: "cautious".to_string(),
                    agent_outputs: vec![],
                });
            }
        }

        // Security watchdog (lightweight patterns). Run when guardrails are enabled.
        let mut outputs: Vec<AgentOutput> = Vec::new();
        if self.use_guardrails() {
            if let Ok(watchdog_output) = self.watchdog.process(context).await {
                outputs.push(watchdog_output.clone());
                if let Some(next_action) = watchdog_output.next_action.as_ref() {
                    match next_action {
                        NextAction::Abort => {
                            let elapsed_ms = start_time.elapsed().as_millis() as u64;
                            let metrics = InvocationMetrics::new(&invocation_id, "watchdog")
                                .complete_failure(elapsed_ms, "watchdog_blocked");
                            self.metrics.record(metrics);
                            return Ok(OrchestrationResult {
                                message: "The ghost senses a security risk and refuses to proceed.".to_string(),
                                proximity: 0.0,
                                solved: false,
                                show_hint: None,
                                ghost_state: "cautious".to_string(),
                                agent_outputs: vec![watchdog_output.clone()],
                            });
                        }
                        NextAction::PauseForConfirmation => {
                            let elapsed_ms = start_time.elapsed().as_millis() as u64;
                            let metrics = InvocationMetrics::new(&invocation_id, "watchdog")
                                .complete_failure(elapsed_ms, "watchdog_confirmation_required");
                            self.metrics.record(metrics);
                            return Ok(OrchestrationResult {
                                message: "Potential risk detected. Please review before continuing.".to_string(),
                                proximity: 0.0,
                                solved: false,
                                show_hint: None,
                                ghost_state: "cautious".to_string(),
                                agent_outputs: vec![watchdog_output.clone()],
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        // Choose workflow based on mode
        let mut workflow_outputs = if self.use_intelligent_mode() {
            tracing::debug!("Using intelligent planning workflow");
            self.planning_workflow.execute(context).await?
        } else {
            tracing::debug!("Using legacy sequential workflow");
            self.workflow.execute(context).await?
        };
        outputs.append(&mut workflow_outputs);

        let mut solved = false;
        let mut show_hint = None;
        let mut proximity = 0.0;
        let mut message = String::new();
        let mut planning_context: Option<PlanningContext> = None;
        let mut requires_confirmation = false;
        let mut abort_requested = false;
        let mut event_stream = EventStream::new(&invocation_id);

        // Extract results from outputs
        for output in &outputs {
            // Create an ADK-style event for this output
            let mut actions = EventActions::default();
            if let Some(delta) = output.data.get("state_delta").and_then(|v| v.as_object()) {
                for (key, value) in delta {
                    actions.state_delta.insert(key.clone(), value.clone());
                }
            }
            if let Some(next_action) = &output.next_action {
                match next_action {
                    NextAction::Abort => actions.terminate = true,
                    NextAction::PauseForConfirmation => actions.escalate = true,
                    _ => {}
                }
            }
            event_stream.push(
                AgentEvent::text(&output.agent_name, &invocation_id, output.result.clone())
                    .with_actions(actions),
            );

            // Check for tool calls and execute them if MCP is available
            if let Some(tool_call) = output.data.get("tool_call") {
                if let Some(server) = mcp_server {
                    if let Some(tool_name) = tool_call.get("tool").and_then(|v| v.as_str()) {
                        if let Some(args) = tool_call.get("arguments") {
                            tracing::info!("Executing MCP tool: {} {:?}", tool_name, args);
                            // Fire and forget - tool execution is a side effect
                            if let Err(e) = self
                                .invoke_browser_tool(server, tool_name, args.clone())
                                .await
                            {
                                tracing::error!("Failed to execute tool {}: {}", tool_name, e);
                            }
                        }
                    }
                }
            }

            // Update proximity if agent provided confidence
            if output.confidence > 0.0 {
                proximity = output.confidence;
            }

            // Extract planning context if present
            if let Some(pc) = output.data.get("planning_context") {
                if let Ok(parsed) = serde_json::from_value(pc.clone()) {
                    planning_context = Some(parsed);
                }
            }

            // Check actions
            if let Some(action) = &output.next_action {
                match action {
                    NextAction::PuzzleSolved => solved = true,
                    NextAction::ShowHint(idx) => show_hint = Some(*idx),
                    NextAction::PauseForConfirmation => requires_confirmation = true,
                    NextAction::Abort => abort_requested = true,
                    _ => {}
                }
            }

            // The last output usually contains the narrative message
            // or we specifically look for narrator output
            if output.agent_name == "Narrator" {
                message = output.result.clone();
            }
        }

        // Apply any state deltas collected from events
        let state_delta = event_stream.collect_state_deltas();
        if !state_delta.is_empty() {
            if let Ok(session) = self.session.lock() {
                if let Err(e) = session.apply_state_delta(&state_delta) {
                    tracing::warn!("Failed to apply state delta: {}", e);
                }
            }
        }

        // If any agent requested abort/confirmation, short-circuit response
        if abort_requested {
            let elapsed_ms = start_time.elapsed().as_millis() as u64;
            let metrics = InvocationMetrics::new(&invocation_id, workflow_name)
                .complete_failure(elapsed_ms, "aborted");
            self.metrics.record(metrics);
            return Ok(OrchestrationResult {
                message: "The ghost detected a critical risk and halted the attempt.".to_string(),
                proximity: 0.0,
                solved: false,
                show_hint: None,
                ghost_state: "cautious".to_string(),
                agent_outputs: outputs,
            });
        }
        if requires_confirmation {
            let elapsed_ms = start_time.elapsed().as_millis() as u64;
            let metrics = InvocationMetrics::new(&invocation_id, workflow_name)
                .complete_failure(elapsed_ms, "confirmation_required");
            self.metrics.record(metrics);
            return Ok(OrchestrationResult {
                message: "The ghost needs confirmation before continuing.".to_string(),
                proximity: 0.0,
                solved: false,
                show_hint: None,
                ghost_state: "cautious".to_string(),
                agent_outputs: outputs,
            });
        }

        // Apply reflection if enabled and we have a narrator message
        if self.use_reflection() && !message.is_empty() && !solved {
            let mut reflection_context = context.clone();
            reflection_context.previous_outputs.push(message.clone());

            // Run reflection to validate/improve the message
            if let Ok(reflection_outputs) =
                self.reflection_workflow.execute(&reflection_context).await
            {
                if let Some(last) = reflection_outputs.last() {
                    // Check if reflection approved the output
                    let approved = last
                        .data
                        .get("reflection_approved")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);

                    if approved {
                        // Use the (potentially refined) output
                        message = last.result.clone();
                    }
                }
            }
        }

        // Apply output guardrails if enabled
        if self.use_guardrails() && !message.is_empty() {
            let safety = self.guardrail.quick_safety_check(&message);
            if !safety.is_safe {
                tracing::warn!("Guardrail filtered output: {:?}", safety.triggered_policies);
                // Replace with safe fallback message
                message = self.guardrail.redact_unsafe_content(&message, &safety);
            }
        }

        // Determine ghost state based on proximity and planning strategy
        let ghost_state = if solved {
            "celebrate".to_string()
        } else if let Some(ref pc) = planning_context {
            match pc.strategy {
                crate::agents::traits::SearchStrategy::Celebrate => "celebrate".to_string(),
                crate::agents::traits::SearchStrategy::Verify => "excited".to_string(),
                crate::agents::traits::SearchStrategy::Focus => "searching".to_string(),
                crate::agents::traits::SearchStrategy::Explore => {
                    if proximity > 0.3 {
                        "thinking".to_string()
                    } else {
                        "idle".to_string()
                    }
                }
            }
        } else if proximity > 0.7 {
            "searching".to_string()
        } else if proximity > 0.3 {
            "thinking".to_string()
        } else {
            "idle".to_string()
        };

        // Update session memory
        if let Ok(session) = self.session.lock() {
            if let Err(e) = session.set_proximity(proximity) {
                tracing::warn!("Failed to update session proximity: {}", e);
            }
        }

        // ADK: Record invocation metrics
        let elapsed_ms = start_time.elapsed().as_millis() as u64;
        let metrics = InvocationMetrics::new(&invocation_id, workflow_name)
            .complete_success(elapsed_ms, proximity);
        self.metrics.record(metrics);

        // ADK: Run after_agent callbacks
        let result = OrchestrationResult {
            message,
            proximity,
            solved,
            show_hint,
            ghost_state,
            agent_outputs: outputs,
        };
        
        if let Some(override_output) = {
            let registry = self.callbacks.lock().await;
            let output = AgentOutput {
                agent_name: "Orchestrator".to_string(),
                result: result.message.clone(),
                confidence: result.proximity,
                next_action: if result.solved { Some(NextAction::PuzzleSolved) } else { None },
                data: HashMap::new(),
            };
            registry.run_after_agent(&callback_context, &output).await
        } {
            let override_message = override_output.result.clone();
            let override_confidence = override_output.confidence;
            let override_next_action = override_output.next_action.clone();
            return Ok(OrchestrationResult {
                message: override_message,
                proximity: override_confidence,
                solved: matches!(override_next_action.as_ref(), Some(NextAction::PuzzleSolved)),
                show_hint: override_next_action.as_ref().and_then(|action| {
                    if let NextAction::ShowHint(idx) = action {
                        Some(*idx)
                    } else {
                        None
                    }
                }),
                ghost_state: "idle".to_string(),
                agent_outputs: vec![override_output],
            });
        }

        Ok(result)
    }

    /// Run parallel background checks (Safety + Analysis)
    pub async fn run_parallel_checks(
        &self,
        context: &AgentContext,
    ) -> AgentResult<Vec<AgentOutput>> {
        let workflow = create_parallel_checks(vec![
            self.verifier.clone() as Arc<dyn crate::agents::Agent>,
            self.observer.clone() as Arc<dyn crate::agents::Agent>,
        ]);

        workflow.execute(context).await
    }

    /// Run an autonomous monitoring loop
    /// Now uses adaptive loop with self-correction capabilities
    pub async fn run_autonomous_loop(
        &self,
        context: &AgentContext,
    ) -> AgentResult<Vec<AgentOutput>> {
        // Use the enhanced adaptive loop for intelligent self-correction
        let observer = self.observer.clone() as Arc<dyn crate::agents::Agent>;
        let workflow = create_adaptive_loop(
            observer,
            AUTONOMOUS_LOOP_MAX_ITERATIONS,
            AUTONOMOUS_LOOP_DELAY_MS,
        );
        workflow.execute(context).await
    }

    /// Run autonomous loop with planning
    /// Creates a plan first, then monitors with self-correction
    pub async fn run_autonomous_loop_with_plan(
        &self,
        context: &AgentContext,
    ) -> AgentResult<Vec<AgentOutput>> {
        let mut current_context = context.clone();

        // Step 1: Generate initial plan
        let planning_output = self.planner.process(&current_context).await?;
        if let Some(pc) = planning_output.data.get("planning_context") {
            if let Ok(parsed) = serde_json::from_value(pc.clone()) {
                current_context.planning = parsed;
            }
        }

        // Step 2: Run adaptive loop with planning context
        let observer = self.observer.clone() as Arc<dyn crate::agents::Agent>;
        let workflow =
            create_adaptive_loop(observer, PLANNED_LOOP_MAX_ITERATIONS, PLANNED_LOOP_DELAY_MS);

        let mut outputs = vec![planning_output];
        let loop_outputs = workflow.execute(&current_context).await?;
        outputs.extend(loop_outputs);

        Ok(outputs)
    }

    /// Handle puzzle solved
    pub async fn on_puzzle_solved(&self, context: &AgentContext) -> AgentResult<String> {
        // Generate success dialogue via specific agent capability
        let dialogue = self.narrator.generate_success_dialogue(context).await?;

        // Record solved puzzle
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Calculate time to solve based on session start time
        let time_to_solve = if let Ok(session) = self.session.lock() {
            if let Ok(state) = session.load() {
                let start = state.puzzle_started_at;
                if start > 0 && now > start {
                    now - start
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        let solved_puzzle = crate::memory::long_term::SolvedPuzzle {
            puzzle_id: context.puzzle_id.clone(),
            solved_at: now,
            time_to_solve_secs: time_to_solve,
            hints_used: context.hints_revealed,
            solution_url: context.current_url.clone(),
        };

        if let Ok(ltm) = self.long_term.lock() {
            if let Err(e) = ltm.record_solved(solved_puzzle) {
                tracing::warn!("Failed to record solved puzzle: {}", e);
            }
        }

        Ok(dialogue)
    }

    /// Get session memory reference (returns the Arc for shared access)
    pub fn session(&self) -> &SharedSessionMemory {
        &self.session
    }

    /// Get long-term memory reference (returns the Arc for shared access)
    pub fn long_term(&self) -> &SharedLongTermMemory {
        &self.long_term
    }

    /// Record a URL visit
    pub fn record_url(&self, url: &str) -> anyhow::Result<()> {
        let session = self
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        session.add_url(url)
    }

    /// Get current session state
    pub fn get_session_state(&self) -> anyhow::Result<crate::memory::session::SessionState> {
        let session = self
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        session.load()
    }

    /// Set the app mode (Game or Companion)
    pub fn set_mode(&self, mode: crate::memory::AppMode) -> anyhow::Result<()> {
        let session = self
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        session.set_mode(mode)
    }

    /// Get current app mode
    pub fn get_mode(&self) -> anyhow::Result<crate::memory::AppMode> {
        let session = self
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        session.get_mode()
    }

    /// Get recent activity entries
    pub fn get_recent_activity(
        &self,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::memory::ActivityEntry>> {
        let session = self
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        session.get_recent_activity(limit)
    }

    /// Record a screenshot capture
    pub fn record_screenshot(&self) -> anyhow::Result<()> {
        let session = self
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        session.record_screenshot()
    }

    /// Record puzzle solved in session
    pub fn record_puzzle_solved_session(&self) -> anyhow::Result<()> {
        let session = self
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        session.record_puzzle_solved()
    }

    /// Generate a puzzle plan (for new puzzles)
    /// Returns sub-goals and keywords that can be used by the frontend
    pub async fn generate_puzzle_plan(
        &self,
        context: &AgentContext,
    ) -> AgentResult<PlanningContext> {
        self.planner.analyze_puzzle(context).await
    }

    /// Revise the current plan based on failure
    /// Used when the user has tried multiple approaches without success
    pub async fn revise_plan(
        &self,
        context: &AgentContext,
        failed_reason: &str,
    ) -> AgentResult<PlanningContext> {
        self.planner.revise_plan(context, failed_reason).await
    }

    /// Validate narrator dialogue through the critic
    /// Returns feedback on quality and safety
    pub async fn validate_dialogue(
        &self,
        dialogue: &str,
        context: &AgentContext,
    ) -> AgentResult<crate::agents::traits::ReflectionFeedback> {
        self.critic.critique(dialogue, context).await
    }

    /// Get improved dialogue suggestion from critic
    pub async fn improve_dialogue(
        &self,
        dialogue: &str,
        feedback: &crate::agents::traits::ReflectionFeedback,
        context: &AgentContext,
    ) -> AgentResult<String> {
        self.critic
            .suggest_improvement(dialogue, feedback, context)
            .await
    }

    // =========================================================================
    // MCP Integration Methods (Chapter 10: Model Context Protocol)
    // =========================================================================

    /// Get all available browser tools from the MCP server
    /// Returns tool descriptors that agents can use for capability discovery
    pub fn get_available_browser_tools(
        &self,
        mcp_server: &crate::mcp::BrowserMcpServer,
    ) -> Vec<ToolDescriptor> {
        mcp_server.discover_tools(None)
    }

    /// Get browser tools filtered by category
    /// Categories: "navigation", "effects", "content"
    pub fn get_browser_tools_by_category(
        &self,
        mcp_server: &crate::mcp::BrowserMcpServer,
        category: &str,
    ) -> Vec<ToolDescriptor> {
        mcp_server.discover_tools(Some(category))
    }

    /// Get available browser resources from the MCP server
    /// Resources: browser://current-page, browser://history, browser://top-sites
    pub fn get_available_browser_resources(
        &self,
        mcp_server: &crate::mcp::BrowserMcpServer,
    ) -> Vec<ResourceDescriptor> {
        mcp_server.discover_resources()
    }

    /// Get MCP manifest for LLM context injection
    /// This provides a complete description of all available capabilities
    pub fn get_mcp_manifest(
        &self,
        mcp_server: &crate::mcp::BrowserMcpServer,
    ) -> crate::mcp::McpManifest {
        mcp_server.manifest()
    }

    /// Invoke a browser tool by name through MCP
    /// This is the preferred way for agents to interact with the browser
    pub async fn invoke_browser_tool(
        &self,
        mcp_server: &crate::mcp::BrowserMcpServer,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        crate::bridge::invoke_mcp_tool(mcp_server, tool_name, arguments).await
    }

    /// Read current page content through MCP resource
    /// Returns the current page URL, title, and body text
    pub async fn read_browser_page(
        &self,
        mcp_server: &crate::mcp::BrowserMcpServer,
    ) -> Option<serde_json::Value> {
        crate::bridge::read_current_page(mcp_server).await
    }

    /// Read browsing history through MCP resource
    /// Returns recent history with optional limit
    pub async fn read_browser_history(
        &self,
        mcp_server: &crate::mcp::BrowserMcpServer,
        limit: Option<usize>,
    ) -> Option<serde_json::Value> {
        crate::bridge::read_browsing_history(mcp_server, limit).await
    }

    /// Generate LLM-friendly tool description for prompting
    /// Converts MCP tool descriptors into a format suitable for LLM context
    pub fn format_tools_for_llm(&self, tools: &[ToolDescriptor]) -> String {
        let mut output = String::from("Available Browser Tools:\n\n");
        for tool in tools {
            output.push_str(&format!("## {}\n", tool.name));
            output.push_str(&format!("Description: {}\n", tool.description));
            output.push_str(&format!("Category: {}\n", tool.category));
            output.push_str(&format!("Has Side Effects: {}\n", tool.is_side_effect));

            if let Some(props) = &tool.input_schema.properties {
                output.push_str("Parameters:\n");
                for (name, schema) in props {
                    let desc = schema.description.as_deref().unwrap_or("No description");
                    output.push_str(&format!("  - {}: {} ({})\n", name, schema.prop_type, desc));
                }
            }
            output.push('\n');
        }
        output
    }

    /// Generate LLM-friendly resource description for prompting
    pub fn format_resources_for_llm(&self, resources: &[ResourceDescriptor]) -> String {
        let mut output = String::from("Available Browser Resources:\n\n");
        for resource in resources {
            output.push_str(&format!("## {} ({})\n", resource.name, resource.uri));
            output.push_str(&format!("Description: {}\n", resource.description));
            output.push_str(&format!("MIME Type: {}\n", resource.mime_type));
            output.push_str(&format!("Dynamic: {}\n\n", resource.is_dynamic));
        }
        output
    }

    // =========================================================================
    // Lifecycle Hooks (Best Practice from Anthropic's Agent Guide)
    // =========================================================================

    /// Initialize the orchestrator and all agents
    /// Call this once at application startup to warm up services
    pub async fn initialize(&self) -> AgentResult<()> {
        tracing::info!("Initializing AgentOrchestrator...");
        let start = Instant::now();

        // Initialize AI router (checks provider availability)
        self.ai_router.initialize().await;

        // Initialize each agent (they have default no-op implementations)
        // We call them in parallel for efficiency
        let init_futures = vec![
            self.observer.initialize(),
            self.verifier.initialize(),
            self.narrator.initialize(),
            self.planner.initialize(),
            self.critic.initialize(),
            self.guardrail.initialize(),
        ];

        // Wait for all initializations
        let results = futures::future::join_all(init_futures).await;

        // Check for any failures
        for (idx, result) in results.into_iter().enumerate() {
            if let Err(e) = result {
                let agent_names = [
                    "Observer",
                    "Verifier",
                    "Narrator",
                    "Planner",
                    "Critic",
                    "Guardrail",
                ];
                tracing::warn!("{} initialization failed: {}", agent_names[idx], e);
            }
        }

        tracing::info!("AgentOrchestrator initialized in {:?}", start.elapsed());
        Ok(())
    }

    /// Shutdown the orchestrator gracefully
    /// Call this before application exit to clean up resources
    pub async fn shutdown(&self) -> AgentResult<()> {
        tracing::info!("Shutting down AgentOrchestrator...");

        // Shutdown each agent
        let shutdown_futures = vec![
            self.observer.shutdown(),
            self.verifier.shutdown(),
            self.narrator.shutdown(),
            self.planner.shutdown(),
            self.critic.shutdown(),
            self.guardrail.shutdown(),
        ];

        let results = futures::future::join_all(shutdown_futures).await;

        for (idx, result) in results.into_iter().enumerate() {
            if let Err(e) = result {
                let agent_names = [
                    "Observer",
                    "Verifier",
                    "Narrator",
                    "Planner",
                    "Critic",
                    "Guardrail",
                ];
                tracing::warn!("{} shutdown failed: {}", agent_names[idx], e);
            }
        }

        // Flush memory stores
        if let Ok(session) = self.session.lock() {
            if let Err(e) = session.flush() {
                tracing::warn!("Session memory flush failed: {}", e);
            }
        }

        if let Ok(ltm) = self.long_term.lock() {
            if let Err(e) = ltm.flush() {
                tracing::warn!("Long-term memory flush failed: {}", e);
            }
        }

        tracing::info!("AgentOrchestrator shutdown complete");
        Ok(())
    }

    /// Check health of all agents and services
    /// Returns a map of component name -> is_healthy
    pub fn health_check(&self) -> HashMap<String, bool> {
        let mut health = HashMap::new();

        // Check each agent's health
        health.insert("Observer".to_string(), self.observer.health_check());
        health.insert("Verifier".to_string(), self.verifier.health_check());
        health.insert("Narrator".to_string(), self.narrator.health_check());
        health.insert("Planner".to_string(), self.planner.health_check());
        health.insert("Critic".to_string(), self.critic.health_check());
        health.insert("Guardrail".to_string(), self.guardrail.health_check());

        // Check AI provider availability
        health.insert("AI_Provider".to_string(), self.ai_router.is_available());
        health.insert("Gemini".to_string(), self.ai_router.has_gemini());
        health.insert("Ollama".to_string(), self.ai_router.has_ollama());

        // Check memory stores
        let session_ok = self.session.lock().is_ok();
        let ltm_ok = self.long_term.lock().is_ok();
        health.insert("SessionMemory".to_string(), session_ok);
        health.insert("LongTermMemory".to_string(), ltm_ok);

        health
    }

    /// Check if the orchestrator is healthy overall
    /// Returns true if critical components are functioning
    pub fn is_healthy(&self) -> bool {
        // Critical: at least one AI provider must be available
        if !self.ai_router.is_available() {
            return false;
        }

        // Critical: memory stores must be accessible
        if self.session.lock().is_err() || self.long_term.lock().is_err() {
            return false;
        }

        // Check all agents
        self.observer.health_check() && self.verifier.health_check() && self.narrator.health_check()
    }
}
