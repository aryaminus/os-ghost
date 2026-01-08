//! The OS Ghost - Tauri Application Library
//! A screen-aware meta-game where an AI entity lives in your desktop

// Core modules
pub mod gemini_client;
pub mod ai_provider;
pub mod bridge;
pub mod capture;
pub mod game_state;
pub mod history;
pub mod ipc;
pub mod monitor;
pub mod ollama_client;
pub mod privacy;
pub mod utils;
pub mod window;

// Multi-agent system
pub mod agents;
pub mod memory;
pub mod workflow;

use gemini_client::GeminiClient;
use ai_provider::SmartAiRouter;
use game_state::EffectQueue;
use ipc::Puzzle;
use memory::LongTermMemory;
use ollama_client::OllamaClient;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
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

    // Load configuration from config file if not in environment
    // This allows production builds to use user-provided keys
    // Uses thread-safe runtime config instead of env::set_var
    if let Some(config_dir) = dirs::config_dir() {
        let config_path = config_dir.join("os-ghost").join("config.json");
        if config_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&contents) {
                    let runtime = utils::runtime_config();

                    // Load Gemini API key if not in environment
                    if std::env::var("GEMINI_API_KEY").is_err() {
                        if let Some(key) = config.get("gemini_api_key").and_then(|k| k.as_str()) {
                            if !key.is_empty() {
                                runtime.set_api_key(key.to_string());
                                tracing::info!("Loaded Gemini API key from config file");
                            }
                        }
                    }

                    // Load Ollama configuration (always from config, env is fallback)
                    if let Some(url) = config.get("ollama_url").and_then(|v| v.as_str()) {
                        if !url.is_empty() {
                            runtime.set_ollama_url(url.to_string());
                            tracing::debug!("Loaded Ollama URL from config: {}", url);
                        }
                    }
                    if let Some(model) = config.get("ollama_vision_model").and_then(|v| v.as_str())
                    {
                        if !model.is_empty() {
                            runtime.set_ollama_vision_model(model.to_string());
                            tracing::debug!("Loaded Ollama vision model from config: {}", model);
                        }
                    }
                    if let Some(model) = config.get("ollama_text_model").and_then(|v| v.as_str()) {
                        if !model.is_empty() {
                            runtime.set_ollama_text_model(model.to_string());
                            tracing::debug!("Loaded Ollama text model from config: {}", model);
                        }
                    }
                }
            }
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Load puzzles (wrapped in RwLock for dynamic puzzle registration)
            let puzzles = std::sync::RwLock::new(default_puzzles());
            tracing::info!(
                "Loaded {} puzzles",
                puzzles.read().map(|p| p.len()).unwrap_or(0)
            );
            app.manage(puzzles);

            // Initialize AI Providers (Gemini + Ollama with smart routing)
            // Get API key from thread-safe runtime config or environment
            let api_key = utils::runtime_config().get_api_key();

            // Create Gemini client only if API key exists
            let gemini_client = if let Some(ref key) = api_key {
                if !key.is_empty() {
                    tracing::info!("Gemini API key configured");
                    Some(Arc::new(GeminiClient::new(key.clone())))
                } else {
                    tracing::warn!("GEMINI_API_KEY empty - will use Ollama if available");
                    None
                }
            } else {
                tracing::warn!("GEMINI_API_KEY not set - will use Ollama if available");
                None
            };

            // Create Ollama client (always available, will check server at runtime)
            let ollama_client = Arc::new(OllamaClient::new());

            // Create SmartAiRouter with both providers
            let ai_router = Arc::new(SmartAiRouter::new(gemini_client.clone(), ollama_client));

            // Initialize router (check Ollama availability)
            let router_init = ai_router.clone();
            tauri::async_runtime::spawn(async move {
                router_init.initialize().await;
            });

            // Register router for IPC commands
            app.manage(ai_router.clone());

            // Create shared memory instances (used by both Orchestrator and Monitor)
            // Note: We use std::sync::Mutex here because:
            // 1. The underlying sled database is already thread-safe
            // 2. Lock durations are short (just for state reads/writes)
            // 3. The mutex protects higher-level state coordination, not sled access
            let store = memory::MemoryStore::new().map_err(|e| {
                tracing::error!("Failed to create memory store: {}", e);
                e
            })?;

            let shared_ltm = Arc::new(Mutex::new(LongTermMemory::new(store.clone())));
            let shared_session = Arc::new(Mutex::new(memory::SessionMemory::new(store.clone())));

            // Register session memory as managed state for IPC commands
            // Create a separate Arc for SessionMemory to be used directly by bridge
            let session_for_ipc = Arc::new(memory::SessionMemory::new(store));
            app.manage(session_for_ipc);

            // Create Orchestrator with shared memory (uses AI router)
            match crate::agents::AgentOrchestrator::new(
                ai_router.clone(),
                shared_ltm.clone(),
                shared_session.clone(),
            ) {
                Ok(orchestrator) => {
                    app.manage(Arc::new(orchestrator));
                    tracing::info!("Agent Orchestrator initialized with shared memory");
                }
                Err(e) => tracing::error!("Failed to initialize orchestrator: {}", e),
            }

            // Initialize Autonomous Task State (for controlling background loops)
            app.manage(ipc::AutonomousTask(tokio::sync::Mutex::new(None)));

            // Initialize EffectQueue for browser visual effects
            app.manage(Arc::new(EffectQueue::default()));

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

            // Start Background Monitor with AI router
            let monitor_router = ai_router.clone();
            let monitor_handle = app.handle().clone();
            let monitor_ltm = shared_ltm.clone();
            let monitor_session = shared_session.clone();

            tauri::async_runtime::spawn(async move {
                monitor::start_monitor_loop(
                    monitor_handle,
                    monitor_router,
                    monitor_ltm,
                    monitor_session,
                )
                .await;
            });

            // Start Hint Checker Loop (Background Task)
            let hint_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
                let mut last_availability = false;

                loop {
                    interval.tick().await;
                    let state = game_state::GameState::load().await;
                    let available = state.should_reveal_hint();

                    // Only emit if state changed to true, or periodically to ensure UI is in sync
                    // For now, simple edge detection + periodic refresh every minute
                    if available && !last_availability {
                        if let Err(e) = hint_handle.emit("hint_available", true) {
                            tracing::error!("Failed to emit hint event: {}", e);
                        }
                    }

                    last_availability = available;
                }
            });

            // Start System Status Checker Loop (Background Task)
            let status_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Check every 30 seconds (less frequent than hints)
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

                // Initial check immediate
                let status = ipc::detect_chrome();
                let _ = status_handle.emit("system_status_update", status);

                loop {
                    interval.tick().await;
                    let status = ipc::detect_chrome();
                    // Always emit for now so UI is always in sync
                    if let Err(e) = status_handle.emit("system_status_update", status) {
                        tracing::error!("Failed to emit status event: {}", e);
                    }
                }
            });

            // Start Native Messaging bridge for Chrome extension
            let app_handle = app.handle().clone();
            bridge::start_native_messaging_server(app_handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            window::start_window_drag,
            ipc::capture_and_analyze,
            ipc::verify_screenshot_proof,
            ipc::check_api_key,
            ipc::set_api_key,
            ipc::validate_api_key,
            ipc::start_investigation,
            ipc::generate_puzzle_from_history,
            ipc::process_agent_cycle,
            ipc::start_background_checks,
            ipc::enable_autonomous_mode,
            ipc::trigger_browser_effect,
            // System detection commands
            ipc::detect_chrome,
            ipc::launch_chrome,
            // Adaptive behavior commands
            ipc::generate_adaptive_puzzle,
            ipc::generate_contextual_dialogue,
            // Ollama configuration commands
            ipc::get_ollama_config,
            ipc::set_ollama_config,
            ipc::reset_ollama_config,
            ipc::get_ollama_status,
            game_state::get_game_state,
            game_state::reset_game,
            game_state::check_hint_available,
            game_state::get_next_hint,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
