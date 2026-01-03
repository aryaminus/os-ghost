//! The OS Ghost - Tauri Application Library
//! A screen-aware meta-game where an AI entity lives in your desktop

// Core modules
pub mod ai_client;
pub mod bridge;
pub mod capture;
pub mod game_state;
pub mod history;
pub mod ipc;
pub mod privacy;
pub mod window;

// Multi-agent system
pub mod agents;
pub mod memory;
pub mod workflow;

use ai_client::GeminiClient;
use ipc::Puzzle;
use std::sync::Arc;
use tauri::Manager;
use window::GhostWindow;

/// Default puzzles for the game
fn default_puzzles() -> Vec<Puzzle> {
    vec![
        Puzzle {
            id: "puzzle_001".to_string(),
            clue: "In 1995, a manifesto appeared in newspapers. Find where it first published online.".to_string(),
            hint: "The Washington Post was one of the first to publish it digitally...".to_string(),
            target_url_pattern: r"(washingtonpost\.com|nytimes\.com).*manifesto".to_string(),
            target_description: "Unabomber manifesto newspaper publication 1995".to_string(),
            is_sponsored: false,
            sponsor_id: None,
            sponsor_url: None,
        },
        Puzzle {
            id: "puzzle_002".to_string(),
            clue: "Before computers, there were wheels within wheels. Find the machine that cracked the impossible.".to_string(),
            hint: "Bletchley Park holds many secrets...".to_string(),
            target_url_pattern: r"(enigma|bletchley|turing)".to_string(),
            target_description: "Enigma machine decryption Bletchley Park Alan Turing".to_string(),
            is_sponsored: false,
            sponsor_id: None,
            sponsor_url: None,
        },
        Puzzle {
            id: "puzzle_003".to_string(),
            clue: "The ghost once lived in a place where 140 characters ruled. Now it's 280, but the old archives remain.".to_string(),
            hint: "Internet Archive remembers everything...".to_string(),
            target_url_pattern: r"(web\.archive\.org|archive\.org).*twitter".to_string(),
            target_description: "Twitter Internet Archive Wayback Machine history".to_string(),
            is_sponsored: false,
            sponsor_id: None,
            sponsor_url: None,
        },
    ]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("os_ghost=debug".parse().unwrap())
                .add_directive("tauri=info".parse().unwrap()),
        )
        .init();

    tracing::info!("Starting The OS Ghost...");

    // Load .env file if present
    if let Err(e) = dotenvy::dotenv() {
        tracing::debug!("No .env file found or error loading: {}", e);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Load puzzles (wrapped in RwLock for dynamic puzzle registration)
            let puzzles = std::sync::RwLock::new(default_puzzles());
            tracing::info!(
                "Loaded {} puzzles",
                puzzles.read().map(|p| p.len()).unwrap_or(0)
            );
            app.manage(puzzles);

            // Initialize Gemini client
            let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_else(|_| {
                tracing::warn!("GEMINI_API_KEY not set, AI features will be disabled");
                String::new()
            });

            // Create shared Gemini client
            let gemini_client = Arc::new(GeminiClient::new(api_key));

            // Register client for direct access by IPC commands
            app.manage(gemini_client.clone());

            match crate::agents::AgentOrchestrator::new(gemini_client) {
                Ok(orchestrator) => {
                    app.manage(Arc::new(orchestrator));
                    tracing::info!("Agent Orchestrator initialized");
                }
                Err(e) => tracing::error!("Failed to initialize orchestrator: {}", e),
            }

            // Setup Ghost window
            if let Some(window) = app.get_webview_window("main") {
                let ghost_window = GhostWindow::new(window.as_ref().window().clone());
                if let Err(e) = ghost_window.setup() {
                    tracing::error!("Failed to setup ghost window: {}", e);
                } else {
                    tracing::info!("Ghost window configured");
                }
                // Position window in bottom-right corner (Clippy-style)
                if let Err(e) = ghost_window.position_bottom_right() {
                    tracing::error!("Failed to position window: {}", e);
                } else {
                    tracing::info!("Window positioned in bottom-right corner");
                }
            }

            // Start Native Messaging bridge for Chrome extension
            let app_handle = app.handle().clone();
            bridge::start_native_messaging_server(app_handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            capture::capture_screen,
            history::get_recent_history,
            window::set_window_clickable,
            window::start_window_drag,
            ipc::capture_and_analyze,
            ipc::get_browsing_history,
            ipc::validate_puzzle,
            ipc::calculate_proximity,
            ipc::generate_ghost_dialogue,
            ipc::get_puzzle,
            ipc::get_all_puzzles,
            ipc::check_api_key,
            ipc::generate_dynamic_puzzle,
            ipc::process_agent_cycle,
            ipc::start_background_checks,
            ipc::enable_autonomous_mode,
            ipc::trigger_browser_effect,
            game_state::get_game_state,
            game_state::reset_game,
            game_state::check_hint_available,
            game_state::get_next_hint,
            privacy::get_privacy_settings,
            privacy::update_privacy_settings,
            privacy::can_capture_screen,
            privacy::can_analyze_with_ai,
            privacy::get_privacy_notice,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
