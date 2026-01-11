use crate::ai_provider::SmartAiRouter;
use crate::utils::current_timestamp_millis;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

/// Puzzle definition
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Puzzle {
    pub id: String,
    pub clue: String,
    pub hint: String,
    pub target_url_pattern: String,
    pub target_description: String,
    // Sponsored fields
    pub sponsor_id: Option<String>,
    pub sponsor_url: Option<String>,
    pub is_sponsored: bool,
}

/// Generated puzzle with ID for frontend
#[derive(Debug, Serialize, Clone)]
pub struct GeneratedPuzzle {
    pub id: String,
    pub clue: String,
    pub hint: String,
    pub hints: Vec<String>,
    pub target_url_pattern: String,
    pub target_description: String,
    pub is_sponsored: bool,
    pub sponsor_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryItem {
    pub url: String,
    pub title: String,
    #[serde(rename = "visitCount", default)]
    pub visit_count: Option<i32>,
    #[serde(rename = "lastVisitTime", default)]
    pub last_visit_time: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct TopSite {
    pub url: String,
    pub title: String,
}

/// Chance of generating a sponsored puzzle (0.0 - 1.0)
const SPONSORED_PUZZLE_CHANCE: f64 = 0.2;

/// Helper: Generate a unique puzzle ID
pub fn generate_puzzle_id(prefix: &str) -> String {
    format!(
        "{}_{}_{}",
        prefix,
        current_timestamp_millis(),
        rand::thread_rng().gen_range(1000..9999)
    )
}

/// Helper: Create a sponsored puzzle
fn create_sponsored_puzzle() -> (Puzzle, GeneratedPuzzle) {
    let id = generate_puzzle_id("sponsored");
    let puzzle = Puzzle {
        id: id.clone(),
        clue: "Seek the cloud where giants build their dreams. Find the console of the Titans."
            .to_string(),
        hint: "Search for 'Google Cloud Console'".to_string(),
        target_url_pattern: "console\\.cloud\\.google\\.com".to_string(),
        target_description: "Google Cloud Console".to_string(),
        sponsor_id: Some("google_cloud".to_string()),
        sponsor_url: Some("https://cloud.google.com".to_string()),
        is_sponsored: true,
    };

    let generated = GeneratedPuzzle {
        id,
        clue: puzzle.clue.clone(),
        hint: puzzle.hint.clone(),
        hints: vec!["Search for 'Google Cloud Console'".to_string()],
        target_url_pattern: puzzle.target_url_pattern.clone(),
        target_description: puzzle.target_description.clone(),
        is_sponsored: true,
        sponsor_id: puzzle.sponsor_id.clone(),
    };

    (puzzle, generated)
}

/// Helper: Register a puzzle and start the timer
async fn register_puzzle(
    puzzles: &std::sync::RwLock<Vec<Puzzle>>,
    puzzle: Puzzle,
) -> Result<(), String> {
    {
        let mut puzzles = puzzles.write().map_err(|e| format!("Lock error: {}", e))?;
        // Keep at most 5 dynamic puzzles (remove oldest first)
        while puzzles.iter().filter(|p| p.id.starts_with("dynamic_")).count() >= 5 {
            if let Some(idx) = puzzles.iter().position(|p| p.id.starts_with("dynamic_")) {
                puzzles.remove(idx);
            } else {
                break;
            }
        }

        puzzles.push(puzzle);
    }

    // Start timer for the new puzzle
    let mut state = crate::game_state::GameState::load().await;
    state.start_puzzle_timer().await;

    Ok(())
}

/// Helper: Common logic to register a dynamic puzzle
async fn register_dynamic_puzzle(
    puzzles: &std::sync::RwLock<Vec<Puzzle>>,
    dynamic: crate::gemini_client::DynamicPuzzle,
    id_prefix: &str,
) -> Result<GeneratedPuzzle, String> {
    let id = generate_puzzle_id(id_prefix);

    let puzzle = Puzzle {
        id: id.clone(),
        clue: dynamic.clue.clone(),
        hint: dynamic.hints.first().cloned().unwrap_or_default(),
        target_url_pattern: dynamic.target_url_pattern.clone(),
        target_description: dynamic.target_description.clone(),
        sponsor_id: None,
        sponsor_url: None,
        is_sponsored: false,
    };

    register_puzzle(puzzles, puzzle).await?;

    Ok(GeneratedPuzzle {
        id,
        clue: dynamic.clue,
        hint: dynamic.hints.first().cloned().unwrap_or_default(),
        hints: dynamic.hints,
        target_url_pattern: dynamic.target_url_pattern,
        target_description: dynamic.target_description,
        is_sponsored: false,
        sponsor_id: None,
    })
}

/// Start an investigation (Unified command for puzzle generation)
/// Decides best source (Content vs History) based on SessionMemory
#[tauri::command]
pub async fn start_investigation(
    _app_handle: tauri::AppHandle,
    ai_router: State<'_, Arc<SmartAiRouter>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
    session: State<'_, Arc<crate::memory::SessionMemory>>, // Inject SessionMemory
) -> Result<GeneratedPuzzle, String> {
    // 1. Load Session State
    let state = session
        .load()
        .map_err(|e| format!("Failed to load session: {}", e))?;

    let url = state.current_url.clone();
    let title = state.current_title.clone();

    // If we have content in memory, allow using it for passive verification
    // even if the regex doesn't match perfectly
    let content = state.current_content.clone().unwrap_or_default();

    // 20% chance of sponsored puzzle
    // Generic chance of sponsored puzzle
    let use_sponsored = rand::thread_rng().gen_range(0.0..1.0) < SPONSORED_PUZZLE_CHANCE;

    if use_sponsored {
        let (puzzle, generated) = create_sponsored_puzzle();

        register_puzzle(&puzzles, puzzle.clone()).await?;

        // Update session
        if let Ok(mut s) = session.load() {
            s.puzzle_id = puzzle.id.clone();
            let _ = session.save(&s);
        }

        return Ok(generated);
    }

    let redacted_url = crate::privacy::redact_pii(&url);
    let redacted_content = crate::privacy::redact_pii(&content);

    // Fetch recent history for context
    let history_context = match crate::history::get_recent_history(10).await {
        Ok(history) => history
            .into_iter()
            .map(|h| {
                format!(
                    "- {} ({})",
                    crate::privacy::redact_pii(&h.title),
                    crate::privacy::redact_pii(&h.url)
                )
            })
            .collect::<Vec<String>>()
            .join("\n"),
        Err(_) => "No recent history available.".to_string(),
    };

    tracing::info!("Starting investigation for url: {}", redacted_url);

    // Call AI
    let dynamic = ai_router
        .generate_dynamic_puzzle(&redacted_url, &title, &redacted_content, &history_context)
        .await
        .map_err(|e| format!("Failed to generate puzzle: {}", e))?;

    // Use helper to register
    let generated = register_dynamic_puzzle(&puzzles, dynamic, "dynamic").await?;

    // Update session
    if let Ok(mut s) = session.load() {
        s.puzzle_id = generated.id.clone();
        let _ = session.save(&s);
    }

    Ok(generated)
}

/// Generate a puzzle based on browsing history (for immediate puzzle without page visit)
#[tauri::command]
pub async fn generate_puzzle_from_history(
    seed_url: String,
    seed_title: String,
    recent_history: Vec<HistoryItem>,
    top_sites: Vec<TopSite>,
    ai_router: State<'_, Arc<SmartAiRouter>>,
    puzzles: State<'_, std::sync::RwLock<Vec<Puzzle>>>,
) -> Result<GeneratedPuzzle, String> {
    let redacted_url = crate::privacy::redact_pii(&seed_url);

    // Build history context from recent history
    let history_context = recent_history
        .iter()
        .take(20)
        .map(|h| {
            format!(
                "- {} ({})",
                crate::privacy::redact_pii(&h.title),
                crate::privacy::redact_pii(&h.url)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Build top sites context
    let top_sites_context = top_sites
        .iter()
        .map(|s| format!("- {} ({})", s.title, s.url))
        .collect::<Vec<_>>()
        .join("\n");

    let combined_context = format!(
        "Recent Browsing History:\n{}\n\nTop Sites:\n{}",
        history_context, top_sites_context
    );

    tracing::info!("Generating puzzle from history with seed: {}", redacted_url);

    let dynamic = ai_router
        .generate_dynamic_puzzle(&redacted_url, &seed_title, "", &combined_context)
        .await
        .map_err(|e| format!("Failed to generate puzzle: {}", e))?;

    // Generate unique ID using shared helper
    let id = generate_puzzle_id("history");

    let puzzle = Puzzle {
        id: id.clone(),
        clue: dynamic.clue.clone(),
        hint: dynamic.hints.first().cloned().unwrap_or_default(),
        target_url_pattern: dynamic.target_url_pattern.clone(),
        target_description: dynamic.target_description.clone(),
        sponsor_id: None,
        sponsor_url: None,
        is_sponsored: false,
    };

    // Register puzzle
    register_puzzle(&puzzles, puzzle).await?;

    Ok(GeneratedPuzzle {
        id,
        clue: dynamic.clue,
        hint: dynamic.hints.first().cloned().unwrap_or_default(),
        hints: dynamic.hints,
        target_url_pattern: dynamic.target_url_pattern,
        target_description: dynamic.target_description,
        is_sponsored: false,
        sponsor_id: None,
    })
}
