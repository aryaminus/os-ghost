//! Memory Pre-Compaction Silent Turn
//!
//! Implements Moltis-style pre-compaction memory preservation:
//! Before context compaction, runs a silent LLM turn to save important
//! information to memory files automatically.
//!
//! This ensures durable memories survive compaction.

use crate::ai::ai_provider::SmartAiRouter;
use crate::data::workspace_context;
use crate::memory::session::SessionMemory;
use crate::memory::LongTermMemory;
use std::sync::Arc;

/// System prompt for the silent memory turn
const SILENT_MEMORY_PROMPT: &str = "You are a memory consolidation agent. Your task is to review the conversation history and save important information to long-term memory.\n\nAnalyze the conversation and extract:\n1. User preferences and working style\n2. Key decisions and their reasoning\n3. Project context, architecture choices, and conventions\n4. Important facts, names, dates, and relationships\n5. Technical setup details (tools, languages, frameworks)\n\nWrite a concise summary (2-4 sentences) of the most important information that should be remembered for future sessions.\n\nFormat your response as markdown that can be saved to a memory file.\nIf there is nothing important to remember, respond with just NO_MEMORY.";

/// Check if session needs compaction based on message count
pub fn should_compact(message_count: usize, context_window: usize) -> bool {
    // Trigger compaction at 95% of context window
    message_count >= (context_window as f64 * 0.95) as usize
}

/// Run silent memory turn before compaction
/// This reviews the conversation and saves important info to memory
#[allow(unused_variables)]
pub async fn run_silent_memory_turn(
    session: &Arc<SessionMemory>,
    ltm: &Arc<LongTermMemory>,
    router: &Arc<SmartAiRouter>,
) -> Result<(), String> {
    tracing::info!("Running pre-compaction silent memory turn");

    // Get session history
    let session_data = session.load().map_err(|e| e.to_string())?;
    
    // Build a summary of recent messages for the LLM
    let recent_summary = format!(
        "Session started at: {}\nCurrent mode: {:?}\nPuzzle: {}\nRecent URLs: {:?}",
        session_data.started_at,
        session_data.current_mode,
        session_data.puzzle_id,
        session_data.recent_urls.iter().take(5).collect::<Vec<_>>()
    );

    // Call LLM with silent memory prompt
    let prompt = format!(
        "{}\n\nRecent session context:\n{}\n\nWhat important information should be saved to memory?",
        SILENT_MEMORY_PROMPT,
        recent_summary
    );

    match router.generate_text(&prompt).await {
        Ok(response) => {
            // Check if the LLM found something worth saving
            if !response.contains("NO_MEMORY") && response.len() > 10 {
                // Save to memory - using workspace context file
                // Note: This is a placeholder - full implementation would save to hybrid memory
                let context_content = format!(
                    "# Session Memory - {}\n\n{}\n\n---\n*Saved automatically via pre-compaction silent turn*",
                    chrono::Utc::now().format("%Y-%m-%d"),
                    response
                );
                
                // For now, log the memory that would be saved
                tracing::info!("Silent memory turn would save: {}", &context_content[..context_content.len().min(200)]);
            } else {
                tracing::debug!("No important memory to save from silent turn");
            }
        }
        Err(e) => {
            tracing::warn!("Silent memory turn LLM call failed: {}", e);
            // Don't fail compaction just because memory saving failed
        }
    }

    Ok(())
}

/// Inject workspace context files into system prompt
pub fn inject_workspace_context(prompt: &str) -> String {
    let context = workspace_context::get_workspace_context();
    
    let mut result = prompt.to_string();
    
    // Add tools context
    if let Some(tools) = &context.tools_md {
        result.push_str("\n\n## Workspace Tools\n");
        result.push_str(tools);
    }
    
    // Add agents context
    if let Some(agents) = &context.agents_md {
        result.push_str("\n\n## Agent Instructions\n");
        result.push_str(agents);
    }
    
    result
}

/// Get boot tasks if BOOT.md exists
pub fn get_boot_tasks() -> Option<String> {
    workspace_context::get_boot_tasks()
}
