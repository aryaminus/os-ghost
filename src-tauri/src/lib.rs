//! The OS Ghost - Tauri Application Library
//! A screen-aware meta-game where an AI entity lives in your desktop

// Core modules
pub mod action_preview;
pub mod actions;
pub mod ai_provider;
pub mod bridge;
pub mod capture;
pub mod game_state;
pub mod gemini_client;
pub mod history;
pub mod ipc;
pub mod monitor;
pub mod monitoring;
pub mod ollama_client;
pub mod privacy;
pub mod system_settings;
pub mod rollback;
pub mod utils;
pub mod window;

// Multi-agent system
pub mod agents;
pub mod memory;
pub mod workflow;

// MCP-compatible abstractions (Chapter 10: Model Context Protocol)
pub mod mcp;

use ai_provider::SmartAiRouter;
use game_state::EffectQueue;
use gemini_client::GeminiClient;
use ipc::Puzzle;
use memory::LongTermMemory;
use ollama_client::OllamaClient;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use std::str::FromStr;
use window::GhostWindow;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

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
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("os_ghost=debug,tauri=info"));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();

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
            match std::fs::read_to_string(&config_path) {
                Ok(contents) => match serde_json::from_str::<serde_json::Value>(&contents) {
                    Ok(config) => {
                        let runtime = utils::runtime_config();

                        // Load Gemini API key if not in environment
                        if std::env::var("GEMINI_API_KEY").is_err() {
                            if let Some(key) = config.get("gemini_api_key").and_then(|k| k.as_str())
                            {
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

                        if let Some(model) =
                            config.get("ollama_vision_model").and_then(|v| v.as_str())
                        {
                            if !model.is_empty() {
                                runtime.set_ollama_vision_model(model.to_string());
                                tracing::debug!(
                                    "Loaded Ollama vision model from config: {}",
                                    model
                                );
                            }
                        }

                        if let Some(model) =
                            config.get("ollama_text_model").and_then(|v| v.as_str())
                        {
                            if !model.is_empty() {
                                runtime.set_ollama_text_model(model.to_string());
                                tracing::debug!("Loaded Ollama text model from config: {}", model);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse config file {:?}: {}", config_path, e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read config file {:?}: {}", config_path, e);
                }
            }
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Register global shortcut for quick toggle (if enabled)
            let app_handle = app.handle().clone();
            let settings = system_settings::SystemSettings::load();
            if settings.global_shortcut_enabled {
                match Shortcut::from_str(&settings.global_shortcut) {
                    Ok(shortcut) => {
                        if let Err(e) = app_handle.global_shortcut().register(shortcut) {
                            tracing::warn!("Failed to register global shortcut: {}", e);
                        }

                        let app_handle_for_shortcut = app_handle.clone();
                        if let Err(e) = app_handle.global_shortcut().on_shortcut(
                            shortcut,
                            move |_, _, _| {
                                if let Some(window) =
                                    app_handle_for_shortcut.get_webview_window("main")
                                {
                                    let visible = window.is_visible().unwrap_or(true);
                                    if visible {
                                        let _ = window.hide();
                                    } else {
                                        let _ = window.show();
                                        let _ = window.set_focus();
                                    }
                                }
                            },
                        ) {
                            tracing::warn!("Failed to set shortcut handler: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Invalid global shortcut: {}", e);
                    }
                }
            }
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
            let session_for_ipc = Arc::new(memory::SessionMemory::new(store.clone()));
            app.manage(session_for_ipc.clone());

            // Register LongTermMemory as managed state for IPC commands (HITL feedback)
            let ltm_for_ipc = Arc::new(memory::LongTermMemory::new(store));
            app.manage(ltm_for_ipc);

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

                use tauri::Manager;

                // Initial check immediate
                let session = status_handle.state::<Arc<memory::SessionMemory>>();
                let status = ipc::detect_system_status(Some(session.as_ref()));
                let _ = status_handle.emit("system_status_update", status);

                loop {
                    interval.tick().await;
                    let session = status_handle.state::<Arc<memory::SessionMemory>>();
                    let status = ipc::detect_system_status(Some(session.as_ref()));
                    // Always emit for now so UI is always in sync
                    if let Err(e) = status_handle.emit("system_status_update", status) {
                        tracing::error!("Failed to emit status event: {}", e);
                    }
                }
            });

            // Start Native Messaging bridge for Chrome extension
            let app_handle = app.handle().clone();
            bridge::start_native_messaging_server(app_handle.clone());

            // Auto-register Native Messaging Host if needed (for distributed builds)
            tauri::async_runtime::spawn(async move {
                if let Err(e) = auto_register_manifest(&app_handle) {
                    tracing::error!("Failed to auto-register manifest: {}", e);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            window::start_window_drag,
            ipc::capture_and_analyze,
            ipc::verify_screenshot_proof,
            ipc::check_api_key,
            ipc::set_api_key,
            ipc::clear_api_key,
            ipc::validate_api_key,
            ipc::puzzles::start_investigation,
            ipc::puzzles::generate_puzzle_from_history,
            ipc::process_agent_cycle,
            ipc::start_background_checks,
            ipc::enable_autonomous_mode,
            ipc::trigger_browser_effect,
            // System detection commands
            ipc::detect_chrome,
            ipc::launch_chrome,
            ipc::open_settings,
            ipc::open_external_url,
            ipc::get_app_mode,
            ipc::set_app_mode,
            ipc::get_autonomy_settings,
            ipc::set_autonomy_settings,
            ipc::get_settings_state,
            system_settings::get_system_settings,
            system_settings::update_system_settings,
            system_settings::set_global_shortcut_enabled,
            system_settings::set_global_shortcut,
            // Adaptive behavior commands
            ipc::generate_adaptive_puzzle,
            ipc::generate_contextual_dialogue,
            ipc::quick_ask,
            // Ollama configuration commands
            ipc::get_ollama_config,
            ipc::set_ollama_config,
            ipc::reset_ollama_config,
            ipc::get_ollama_status,
            // HITL Feedback commands (Chapter 13)
            ipc::submit_feedback,
            ipc::submit_escalation,
            ipc::resolve_escalation,
            ipc::get_player_stats,
            ipc::get_feedback_ratio,
            ipc::get_learning_patterns,
            // Privacy commands
            privacy::get_privacy_settings,
            privacy::update_privacy_settings,
            privacy::can_capture_screen,
            privacy::can_analyze_with_ai,
            privacy::get_privacy_notice,
            capture::get_capture_settings,
            capture::set_capture_settings,
            // Action confirmation commands
            actions::get_pending_actions,
            actions::approve_action,
            actions::deny_action,
            actions::get_action_history,
            actions::clear_pending_actions,
            actions::clear_action_history,
            actions::execute_approved_action,
            // Action preview commands
            actions::get_active_preview,
            actions::approve_preview,
            actions::deny_preview,
            actions::update_preview_param,
            // Undo/Rollback commands
            actions::get_rollback_status,
            actions::undo_action,
            actions::redo_action,
            // Sandbox commands (file system & shell access)
            mcp::sandbox::get_sandbox_settings,
            mcp::sandbox::set_sandbox_trust_level,
            mcp::sandbox::add_sandbox_read_path,
            mcp::sandbox::remove_sandbox_read_path,
            mcp::sandbox::add_sandbox_write_path,
            mcp::sandbox::remove_sandbox_write_path,
            mcp::sandbox::enable_shell_category,
            mcp::sandbox::disable_shell_category,
            mcp::sandbox::set_confirm_all_writes,
            mcp::sandbox::set_max_read_size,
            mcp::sandbox::sandbox_read_file,
            mcp::sandbox::sandbox_write_file,
            mcp::sandbox::sandbox_list_dir,
            mcp::sandbox::sandbox_execute_shell,
            // Model capabilities & token usage (P1/P2)
            ipc::get_model_capabilities,
            ipc::get_token_usage,
            ipc::reset_token_usage,
            // Unified polling (replaces multiple frontend intervals)
            ipc::poll_agent_status,
            // Intelligent mode commands
            ipc::get_intelligent_mode,
            ipc::set_intelligent_mode,
            ipc::set_reflection_mode,
            ipc::set_guardrails_mode,
            // Game state commands
            game_state::get_game_state,
            game_state::reset_game,
            game_state::check_hint_available,
            game_state::get_next_hint,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ============================================================================
// Auto-Registration Helper
// ============================================================================

/// Extension IDs for allowed origins
const EXTENSION_ID_STORE: &str = "iakaaklohlcdhoalipmmljopmjnhbcdn";
const EXTENSION_ID_UNPACKED: &str = "mmoochocmifhoanmkhkjolhjbikijjag";

/// Automatically register the Native Messaging manifest for the bundled sidecar
fn auto_register_manifest(_app: &tauri::AppHandle) -> Result<(), String> {
    // 1. Resolve the path to the bundled sidecar binary
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("No parent dir for exe")?;

    #[cfg(windows)]
    let binary_name = "native_bridge.exe";
    #[cfg(not(windows))]
    let binary_name = "native_bridge";

    let binary_path = exe_dir.join(binary_name);

    if !binary_path.exists() {
        // In dev mode, sidecars might not be present. Skip auto-registration.
        tracing::debug!("Sidecar not found at {:?}, skipping auto-reg", binary_path);
        return Ok(());
    }

    // 2. Register for all supported browsers
    #[cfg(target_os = "macos")]
    {
        // Chrome
        register_manifest_for_dir(
            &binary_path,
            &dirs::home_dir()
                .ok_or("No home dir")?
                .join("Library/Application Support/Google/Chrome/NativeMessagingHosts"),
        )?;
        // Chromium
        register_manifest_for_dir(
            &binary_path,
            &dirs::home_dir()
                .ok_or("No home dir")?
                .join("Library/Application Support/Chromium/NativeMessagingHosts"),
        )?;
    }

    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().ok_or("No home dir")?;
        // Chrome
        register_manifest_for_dir(
            &binary_path,
            &home.join(".config/google-chrome/NativeMessagingHosts"),
        )?;
        // Chromium
        register_manifest_for_dir(
            &binary_path,
            &home.join(".config/chromium/NativeMessagingHosts"),
        )?;
    }

    #[cfg(target_os = "windows")]
    {
        register_windows_manifest(&binary_path)?;
    }

    Ok(())
}

/// Register manifest in a specific directory (macOS/Linux)
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn register_manifest_for_dir(
    binary_path: &std::path::Path,
    manifest_dir: &std::path::Path,
) -> Result<(), String> {
    // Only create manifest if parent browser config exists (browser is installed)
    if let Some(parent) = manifest_dir.parent() {
        if !parent.exists() {
            tracing::debug!(
                "Browser config dir {:?} doesn't exist, skipping",
                parent
            );
            return Ok(());
        }
    }

    if !manifest_dir.exists() {
        std::fs::create_dir_all(manifest_dir).map_err(|e| e.to_string())?;
    }

    let manifest_path = manifest_dir.join("com.osghost.game.json");

    let content = serde_json::json!({
        "name": "com.osghost.game",
        "description": "OS Ghost Native Messaging Bridge",
        "path": binary_path,
        "type": "stdio",
        "allowed_origins": [
            format!("chrome-extension://{}/", EXTENSION_ID_STORE),
            format!("chrome-extension://{}/", EXTENSION_ID_UNPACKED)
        ]
    });

    let json = serde_json::to_string_pretty(&content).map_err(|e| e.to_string())?;
    std::fs::write(&manifest_path, json).map_err(|e| e.to_string())?;

    tracing::info!(
        "Auto-registered Native Messaging manifest at {:?}",
        manifest_path
    );
    Ok(())
}

/// Register Native Messaging host via Windows Registry
#[cfg(target_os = "windows")]
fn register_windows_manifest(binary_path: &std::path::Path) -> Result<(), String> {
    use std::process::Command;

    // On Windows, Native Messaging requires:
    // 1. A manifest JSON file (can be anywhere)
    // 2. A registry key pointing to that manifest

    // Put manifest in app data
    let app_data = dirs::data_local_dir().ok_or("No local app data dir")?;
    let manifest_dir = app_data.join("OSGhost");

    if !manifest_dir.exists() {
        std::fs::create_dir_all(&manifest_dir).map_err(|e| e.to_string())?;
    }

    let manifest_path = manifest_dir.join("com.osghost.game.json");

    // Windows paths in JSON need forward slashes or escaped backslashes
    let binary_path_str = binary_path.to_string_lossy().replace('\\', "\\\\");

    let content = serde_json::json!({
        "name": "com.osghost.game",
        "description": "OS Ghost Native Messaging Bridge",
        "path": binary_path_str,
        "type": "stdio",
        "allowed_origins": [
            format!("chrome-extension://{}/", EXTENSION_ID_STORE),
            format!("chrome-extension://{}/", EXTENSION_ID_UNPACKED)
        ]
    });

    let json = serde_json::to_string_pretty(&content).map_err(|e| e.to_string())?;
    std::fs::write(&manifest_path, &json).map_err(|e| e.to_string())?;

    tracing::info!("Wrote Native Messaging manifest to {:?}", manifest_path);

    // Register in Windows Registry for Chrome
    // HKEY_CURRENT_USER\Software\Google\Chrome\NativeMessagingHosts\com.osghost.game
    let manifest_path_str = manifest_path.to_string_lossy();

    let reg_result = Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Google\Chrome\NativeMessagingHosts\com.osghost.game",
            "/ve",
            "/t",
            "REG_SZ",
            "/d",
            &manifest_path_str,
            "/f",
        ])
        .output();

    match reg_result {
        Ok(output) if output.status.success() => {
            tracing::info!("Registered Chrome Native Messaging host in registry");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to register Chrome host: {}", stderr);
        }
        Err(e) => {
            tracing::warn!("Failed to run reg command for Chrome: {}", e);
        }
    }

    // Also register for Chromium
    let reg_result = Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Chromium\NativeMessagingHosts\com.osghost.game",
            "/ve",
            "/t",
            "REG_SZ",
            "/d",
            &manifest_path_str,
            "/f",
        ])
        .output();

    match reg_result {
        Ok(output) if output.status.success() => {
            tracing::info!("Registered Chromium Native Messaging host in registry");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::debug!("Chromium registry (may not be installed): {}", stderr);
        }
        Err(e) => {
            tracing::debug!("Failed to run reg command for Chromium: {}", e);
        }
    }

    Ok(())
}
