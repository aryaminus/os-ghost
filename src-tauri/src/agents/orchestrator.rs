//! Agent Orchestrator - Coordinates all agents
//! Central coordinator for the multi-agent system

use super::narrator::NarratorAgent;
use super::observer::ObserverAgent;
use super::traits::{AgentContext, AgentOutput, AgentResult, NextAction};
use super::verifier::VerifierAgent;
use crate::ai_client::GeminiClient;
use crate::memory::{LongTermMemory, MemoryStore, SessionMemory};
use crate::workflow::{
    loop_agent::create_hotcold_loop, parallel::create_parallel_checks,
    sequential::create_puzzle_pipeline, SequentialWorkflow, Workflow,
};
use std::sync::Arc;

/// The main orchestrator that coordinates all agents
pub struct AgentOrchestrator {
    workflow: SequentialWorkflow,
    narrator: Arc<NarratorAgent>,
    // Keep references for ad-hoc workflows (parallel/loop)
    observer: Arc<ObserverAgent>,
    verifier: Arc<VerifierAgent>,
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

        // Create agents
        let observer = Arc::new(ObserverAgent::new(Arc::clone(&gemini)));
        let verifier = Arc::new(VerifierAgent::new());
        let narrator = Arc::new(NarratorAgent::new(gemini));

        // Build workflow pipeline: Observer -> Verifier -> Narrator
        let workflow = create_puzzle_pipeline(
            observer.clone(),
            verifier.clone(),
            narrator.clone(),
        );

        Ok(Self {
            workflow,
            narrator,
            observer,
            verifier,
            session: SessionMemory::new(store.clone()),
            long_term: LongTermMemory::new(store),
        })
    }

    /// Run the full agent pipeline sequentially
    pub async fn process(&self, context: &AgentContext) -> AgentResult<OrchestrationResult> {
        // Execute workflow
        let outputs = self.workflow.execute(context).await?;

        let mut solved = false;
        let mut show_hint = None;
        let mut proximity = 0.0;
        let mut message = String::new();

        // Extract results from outputs
        for output in &outputs {
            // Update proximity if agent provided confidence
            if output.confidence > 0.0 {
                proximity = output.confidence;
            }

            // Check actions
            if let Some(action) = &output.next_action {
                match action {
                    NextAction::PuzzleSolved => solved = true,
                    NextAction::ShowHint(idx) => show_hint = Some(*idx),
                    _ => {}
                }
            }

            // The last output usually contains the narrative message
            // or we specificially look for narrator output
            if output.agent_name == "Narrator" {
                message = output.result.clone();
            }
        }

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
    pub async fn run_autonomous_loop(
        &self,
        context: &AgentContext,
    ) -> AgentResult<Vec<AgentOutput>> {
        let observer = self.observer.clone() as Arc<dyn crate::agents::Agent>;
        let workflow = create_hotcold_loop(observer, 5, 2000);
        workflow.execute(context).await
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
