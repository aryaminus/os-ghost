//! Agent Orchestrator - Coordinates all agents
//! Central coordinator for the multi-agent system

use super::narrator::NarratorAgent;
use super::observer::ObserverAgent;
use super::traits::{Agent, AgentContext, AgentOutput, AgentResult, NextAction};
use super::verifier::VerifierAgent;
use crate::ai_client::GeminiClient;
use crate::memory::{LongTermMemory, MemoryStore, SessionMemory};
use std::sync::Arc;

/// The main orchestrator that coordinates all agents
pub struct AgentOrchestrator {
    observer: ObserverAgent,
    narrator: NarratorAgent,
    verifier: VerifierAgent,
    session: SessionMemory,
    long_term: LongTermMemory,
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
    /// Create a new orchestrator with all agents
    pub fn new(gemini: Arc<GeminiClient>) -> anyhow::Result<Self> {
        let store = MemoryStore::new()?;

        Ok(Self {
            observer: ObserverAgent::new(Arc::clone(&gemini)),
            narrator: NarratorAgent::new(gemini),
            verifier: VerifierAgent::new(),
            session: SessionMemory::new(store.clone()),
            long_term: LongTermMemory::new(store),
        })
    }

    /// Run the full agent pipeline sequentially
    pub async fn process(&self, context: &AgentContext) -> AgentResult<OrchestrationResult> {
        let mut outputs = Vec::new();
        let mut solved = false;
        let mut show_hint = None;
        let mut proximity = 0.0;
        let message: String;

        // Step 1: Observer analyzes the current state
        if self.observer.can_handle(context) {
            let observer_output = self.observer.process(context).await?;

            proximity = observer_output.confidence;

            if let Some(NextAction::PuzzleSolved) = &observer_output.next_action {
                // Don't mark solved yet, let verifier confirm
            }

            outputs.push(observer_output);
        }

        // Step 2: Verifier checks for solution
        if self.verifier.can_handle(context) {
            let verifier_output = self.verifier.process(context).await?;

            if let Some(NextAction::PuzzleSolved) = &verifier_output.next_action {
                solved = true;
            }

            outputs.push(verifier_output);
        }

        // Step 3: Narrator generates appropriate dialogue
        let mut narrator_context = context.clone();
        narrator_context.proximity = proximity;
        let narrator_output = self.narrator.process(&narrator_context).await?;
        message = narrator_output.result.clone();

        // Check if hint should be shown
        for output in &outputs {
            if let Some(NextAction::ShowHint(idx)) = &output.next_action {
                show_hint = Some(*idx);
            }
        }

        outputs.push(narrator_output);

        // Determine ghost state
        let ghost_state = if solved {
            "celebrate".to_string()
        } else if proximity > 0.7 {
            "searching".to_string()
        } else if proximity > 0.3 {
            "thinking".to_string()
        } else {
            "idle".to_string()
        };

        // Update session memory
        if let Err(e) = self.session.set_proximity(proximity) {
            tracing::warn!("Failed to update session proximity: {}", e);
        }

        Ok(OrchestrationResult {
            message,
            proximity,
            solved,
            show_hint,
            ghost_state,
            agent_outputs: outputs,
        })
    }

    /// Handle puzzle solved
    pub async fn on_puzzle_solved(&self, context: &AgentContext) -> AgentResult<String> {
        // Generate success dialogue
        let dialogue = self.narrator.generate_success_dialogue(context).await?;

        // Record solved puzzle
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let solved_puzzle = crate::memory::long_term::SolvedPuzzle {
            puzzle_id: context.puzzle_id.clone(),
            solved_at: now,
            time_to_solve_secs: 0, // Would need to track start time
            hints_used: context.hints_revealed,
            solution_url: context.current_url.clone(),
        };

        if let Err(e) = self.long_term.record_solved(solved_puzzle) {
            tracing::warn!("Failed to record solved puzzle: {}", e);
        }

        Ok(dialogue)
    }

    /// Get session memory reference
    pub fn session(&self) -> &SessionMemory {
        &self.session
    }

    /// Get long-term memory reference
    pub fn long_term(&self) -> &LongTermMemory {
        &self.long_term
    }

    /// Record a URL visit
    pub fn record_url(&self, url: &str) -> anyhow::Result<()> {
        self.session.add_url(url)
    }
}
