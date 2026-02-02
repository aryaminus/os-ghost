//! The OS Ghost - Tauri Application Library
//! A screen-aware meta-game where an AI entity lives in your desktop

// Core modules
pub mod action_preview;
pub mod action_ledger;
pub mod actions;
pub mod ai_provider;
pub mod bridge;
pub mod capture;
pub mod change_detection;
pub mod email;
pub mod game_state;
pub mod gemini_client;
pub mod history;
pub mod integrations;
pub mod ipc;
pub mod monitor;
pub mod monitoring;
pub mod events_bus;
pub mod permissions;
pub mod intent;
pub mod intent_autorun;
pub mod skills;
pub mod workflows;
pub mod extensions;
pub mod persona;
pub mod perf;
pub mod notifications;
pub mod ollama_client;
pub mod pairing;
pub mod privacy;
pub mod system_status;
pub mod system_settings;
pub mod scheduler;
pub mod timeline;
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
use game_state::{EffectMessage, EffectQueue};
use gemini_client::GeminiClient;
use ipc::Puzzle;
use memory::LongTermMemory;
use ollama_client::OllamaClient;
use std::sync::{Arc, Mutex};
use std::sync::RwLock;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};
use std::str::FromStr;
use window::GhostWindow;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_updater::UpdaterExt;

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
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            crate::rollback::init_rollback_manager();
            crate::rollback::set_undo_executor(crate::rollback::default_undo_executor());
            // App menu + accessibility-friendly standard items
            let app_settings_item = MenuItem::with_id(
                app,
                "system_settings",
                "System Settings…",
                true,
                Some("CmdOrCtrl+,"),
            )
            .map_err(|e| e.to_string())?;

            let settings_open = MenuItem::with_id(
                app,
                "settings_open",
                "Open Settings",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;

            let settings_general = MenuItem::with_id(app, "settings_general", "General", true, None::<&str>)
                .map_err(|e| e.to_string())?;
            let settings_privacy = MenuItem::with_id(
                app,
                "settings_privacy",
                "Privacy & Security",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let settings_extensions = MenuItem::with_id(
                app,
                "settings_extensions",
                "Extensions",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let settings_keys = MenuItem::with_id(
                app,
                "settings_keys",
                "Keys and Models",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let settings_autonomy = MenuItem::with_id(
                app,
                "settings_autonomy",
                "Autonomy",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let settings_sandbox = MenuItem::with_id(
                app,
                "settings_sandbox",
                "Sandbox",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let settings_submenu = Submenu::with_items(
                app,
                "Settings",
                true,
                &[
                    &settings_open,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                    &settings_general,
                    &settings_privacy,
                    &settings_extensions,
                    &settings_keys,
                    &settings_autonomy,
                    &settings_sandbox,
                ],
            )
            .map_err(|e| e.to_string())?;

            let ghost_toggle = MenuItem::with_id(
                app,
                "ghost_toggle",
                "Toggle Ghost Window",
                true,
                Some("CmdOrCtrl+Shift+G"),
            )
            .map_err(|e| e.to_string())?;
            let ghost_show = MenuItem::with_id(app, "ghost_show", "Show Ghost", true, None::<&str>)
                .map_err(|e| e.to_string())?;
            let ghost_hide = MenuItem::with_id(app, "ghost_hide", "Hide Ghost", true, None::<&str>)
                .map_err(|e| e.to_string())?;
            let ghost_focus = MenuItem::with_id(app, "ghost_focus", "Focus Ghost", true, None::<&str>)
                .map_err(|e| e.to_string())?;
            let ghost_reset = MenuItem::with_id(app, "ghost_reset", "Reset Game", true, None::<&str>)
                .map_err(|e| e.to_string())?;
            let ghost_ping_extension = MenuItem::with_id(
                app,
                "ghost_ping_extension",
                "Ping Extension",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let ghost_toggle_monitoring = MenuItem::with_id(
                app,
                "ghost_toggle_monitoring",
                "Toggle Monitoring",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let ghost_submenu = Submenu::with_items(
                app,
                "Ghost",
                true,
                &[
                    &ghost_toggle,
                    &ghost_show,
                    &ghost_hide,
                    &ghost_focus,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                    &ghost_reset,
                    &ghost_ping_extension,
                    &ghost_toggle_monitoring,
                ],
            )
            .map_err(|e| e.to_string())?;

            let mode_companion = MenuItem::with_id(app, "mode_companion", "Companion Mode", true, None::<&str>)
                .map_err(|e| e.to_string())?;
            let mode_game = MenuItem::with_id(app, "mode_game", "Game Mode", true, None::<&str>)
                .map_err(|e| e.to_string())?;
            let mode_submenu = Submenu::with_items(
                app,
                "Mode",
                true,
                &[&mode_companion, &mode_game],
            )
            .map_err(|e| e.to_string())?;

            let autonomy_toggle = MenuItem::with_id(
                app,
                "autonomy_toggle",
                "Toggle Auto Puzzle",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let autonomy_settings = MenuItem::with_id(
                app,
                "autonomy_settings",
                "Open Autonomy Settings",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let autonomy_submenu = Submenu::with_items(
                app,
                "Autonomy",
                true,
                &[&autonomy_toggle, &autonomy_settings],
            )
            .map_err(|e| e.to_string())?;

            let privacy_toggle = MenuItem::with_id(
                app,
                "privacy_toggle",
                "Toggle Read-Only Mode",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let privacy_settings = MenuItem::with_id(
                app,
                "privacy_settings",
                "Open Privacy Settings",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let privacy_submenu = Submenu::with_items(
                app,
                "Privacy",
                true,
                &[&privacy_toggle, &privacy_settings],
            )
            .map_err(|e| e.to_string())?;

            let status_health = MenuItem::with_id(
                app,
                "status_health",
                "Run Health Check",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let status_extensions = MenuItem::with_id(
                app,
                "status_extensions",
                "Open Extensions",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let status_privacy = MenuItem::with_id(
                app,
                "status_privacy",
                "Open Privacy",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let status_submenu = Submenu::with_items(
                app,
                "Status",
                true,
                &[&status_health, &status_extensions, &status_privacy],
            )
            .map_err(|e| e.to_string())?;

            let edit_submenu = Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::redo(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::cut(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::copy(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::paste(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::select_all(app, None).map_err(|e| e.to_string())?,
                ],
            )
            .map_err(|e| e.to_string())?;

            let window_submenu = Submenu::with_items(
                app,
                "Window",
                true,
                &[
                    &PredefinedMenuItem::minimize(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::maximize(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::close_window(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::fullscreen(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                ],
            )
            .map_err(|e| e.to_string())?;

            let check_updates = MenuItem::with_id(
                app,
                "check_updates",
                "Check for Updates…",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
            let app_submenu = Submenu::with_items(
                app,
                "OS Ghost",
                true,
                &[
                    &PredefinedMenuItem::about(app, None, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                    &check_updates,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                    &app_settings_item,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::services(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::hide(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::hide_others(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::show_all(app, None).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                    &PredefinedMenuItem::quit(app, None).map_err(|e| e.to_string())?,
                ],
            )
            .map_err(|e| e.to_string())?;

            let menu = Menu::with_items(
                app,
                &[
                    &app_submenu,
                    &settings_submenu,
                    &ghost_submenu,
                    &mode_submenu,
                    &autonomy_submenu,
                    &privacy_submenu,
                    &status_submenu,
                    &edit_submenu,
                    &window_submenu,
                ],
            )
            .map_err(|e| e.to_string())?;
            app.set_menu(menu).map_err(|e| e.to_string())?;

            app.on_menu_event(|app, event| {
                let emit_settings_update = |app: &tauri::AppHandle| {
                    let _ = app.emit("settings:updated", serde_json::json!({ "source": "menu" }));
                };

                let emit_status_update = |app: &tauri::AppHandle| {
                    let session = app.state::<Arc<memory::SessionMemory>>();
                    let status = crate::ipc::detect_system_status(Some(session.as_ref()));
                    let _ = app.emit("system_status_update", status);
                };

                match event.id().as_ref() {
                    "system_settings" | "settings_open" | "settings_general" => {
                        let _ = crate::ipc::open_settings(Some("general".to_string()), app.clone());
                    }
                    "settings_privacy" | "privacy_settings" => {
                        let _ = crate::ipc::open_settings(Some("privacy".to_string()), app.clone());
                    }
                    "settings_extensions" => {
                        let _ = crate::ipc::open_settings(Some("extensions".to_string()), app.clone());
                    }
                    "settings_keys" => {
                        let _ = crate::ipc::open_settings(Some("keys".to_string()), app.clone());
                    }
                    "settings_autonomy" | "autonomy_settings" => {
                        let _ = crate::ipc::open_settings(Some("autonomy".to_string()), app.clone());
                    }
                    "settings_sandbox" => {
                        let _ = crate::ipc::open_settings(Some("sandbox".to_string()), app.clone());
                    }
                    "check_updates" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            run_update_check(handle, true).await;
                        });
                    }
                    "ghost_toggle" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let visible = window.is_visible().unwrap_or(true);
                            if visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    "ghost_show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "ghost_hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                    "ghost_focus" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.set_focus();
                        }
                    }
                    "ghost_reset" => {
                        tauri::async_runtime::spawn(async move {
                            let _ = crate::game_state::reset_game().await;
                        });
                    }
                    "ghost_ping_extension" => {
                        let effect_queue = app.state::<Arc<EffectQueue>>();
                        effect_queue.push(EffectMessage {
                            action: "ping".to_string(),
                            effect: None,
                            duration: None,
                            text: None,
                            url: None,
                        });
                    }
                    "ghost_toggle_monitoring" => {
                        let mut settings = system_settings::SystemSettings::load();
                        settings.monitor_enabled = !settings.monitor_enabled;
                        let _ = settings.save();
                        emit_settings_update(app);
                    }
                    "mode_companion" => {
                        let session = app.state::<Arc<memory::SessionMemory>>();
                        let _ = session.set_preferred_mode(memory::AppMode::Companion);
                        let _ = session.set_mode(memory::AppMode::Companion);
                        emit_status_update(app);
                        emit_settings_update(app);
                    }
                    "mode_game" => {
                        let session = app.state::<Arc<memory::SessionMemory>>();
                        let _ = session.set_preferred_mode(memory::AppMode::Game);
                        let _ = session.set_mode(memory::AppMode::Game);
                        emit_status_update(app);
                        emit_settings_update(app);
                    }
                    "autonomy_toggle" => {
                        let session = app.state::<Arc<memory::SessionMemory>>();
                        if let Ok(current) = session.get_auto_puzzle_from_companion() {
                            let _ = session.set_auto_puzzle_from_companion(!current);
                            emit_status_update(app);
                            emit_settings_update(app);
                        }
                    }
                    "privacy_toggle" => {
                        let settings = crate::privacy::PrivacySettings::load();
                        let _ = crate::privacy::update_privacy_settings(
                            settings.capture_consent,
                            settings.ai_analysis_consent,
                            settings.privacy_notice_acknowledged,
                            !settings.read_only_mode,
                            None,
                            None,
                            None,
                            None,
                        );
                        emit_settings_update(app);
                    }
                    "status_health" => {
                        let _ = crate::ipc::open_settings(Some("general".to_string()), app.clone());
                    }
                    "status_extensions" => {
                        let _ = crate::ipc::open_settings(Some("extensions".to_string()), app.clone());
                    }
                    "status_privacy" => {
                        let _ = crate::ipc::open_settings(Some("privacy".to_string()), app.clone());
                    }
                    _ => {}
                }
            });

            if !cfg!(debug_assertions) {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    run_update_check(handle, false).await;
                });
            }

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

            // Initialize system status store
            let status_store = Arc::new(RwLock::new(system_status::SystemStatusStore::default()));
            system_status::init_system_status_store(status_store.clone());
            app.manage(status_store);

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
            let notes_store = Arc::new(integrations::NotesStore::new(store.clone()));

            // Register session memory as managed state for IPC commands
            // Create a separate Arc for SessionMemory to be used directly by bridge
            let session_for_ipc = Arc::new(memory::SessionMemory::new(store.clone()));
            app.manage(session_for_ipc.clone());

            // Register LongTermMemory as managed state for IPC commands (HITL feedback)
            let ltm_for_ipc = Arc::new(memory::LongTermMemory::new(store));
            app.manage(ltm_for_ipc);
            app.manage(notes_store);

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

            // Initialize scheduler state
            let scheduler_state = Arc::new(RwLock::new(scheduler::SchedulerState::default()));
            app.manage(scheduler_state.clone());

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
            let status_router = ai_router.clone();
            tauri::async_runtime::spawn(async move {
                // Check every 30 seconds (less frequent than hints)
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

                use tauri::Manager;

                // Initial check immediate
                let session = status_handle.state::<Arc<memory::SessionMemory>>();
                if let Some(store) = crate::system_status::get_system_status_store() {
                    if let Ok(mut guard) = store.write() {
                        guard.active_provider = Some(status_router.active_provider().to_string());
                    }
                }
                let status = ipc::detect_system_status(Some(session.as_ref()));
                let _ = status_handle.emit("system_status_update", status);

                loop {
                    interval.tick().await;
                    let now = crate::utils::current_timestamp();
                    if let Some(store) = crate::system_status::get_system_status_store() {
                        if let Ok(mut guard) = store.write() {
                            if let Some(last) = guard.last_extension_heartbeat {
                                let timeout = crate::system_status::HEARTBEAT_TIMEOUT_SECS;
                                guard.extension_operational = now.saturating_sub(last) <= timeout;
                            }
                            guard.active_provider = Some(status_router.active_provider().to_string());
                        }
                    }
                    let session = status_handle.state::<Arc<memory::SessionMemory>>();
                    let status = ipc::detect_system_status(Some(session.as_ref()));
                    // Always emit for now so UI is always in sync
                    if let Err(e) = status_handle.emit("system_status_update", status) {
                        tracing::error!("Failed to emit status event: {}", e);
                    }
                }
            });

            // Start scheduler loop
            scheduler::start_scheduler_loop(app.handle().clone(), scheduler_state);

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
            ipc::request_extension_ping,
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
            ipc::health_check,
            reset_bridge_registration,
            rebuild_native_bridge,
            system_settings::get_system_settings,
            system_settings::update_system_settings,
            system_settings::set_monitor_enabled,
            system_settings::set_global_shortcut_enabled,
            system_settings::set_global_shortcut,
            system_settings::get_change_detection_settings,
            system_settings::set_change_detection_settings,
            // Adaptive behavior commands
            ipc::generate_adaptive_puzzle,
            ipc::generate_contextual_dialogue,
            ipc::quick_ask,
            // Integrations: calendar + notes + email
            integrations::get_calendar_settings,
            integrations::update_calendar_settings,
            integrations::get_upcoming_events,
            integrations::list_notes,
            integrations::get_files_settings,
            integrations::update_files_settings,
            integrations::list_recent_files,
            integrations::get_email_settings,
            integrations::update_email_settings,
            integrations::email_oauth_status,
            integrations::email_begin_oauth,
            integrations::email_disconnect,
            integrations::list_email_inbox,
            integrations::triage_email_inbox,
            integrations::apply_email_triage,
            integrations::add_note,
            integrations::update_note,
            integrations::delete_note,
            // Scheduler commands
            scheduler::get_scheduler_settings,
            scheduler::update_scheduler_settings,
            // Pairing commands
            pairing::get_pairing_status,
            pairing::create_pairing_code,
            pairing::approve_pairing,
            pairing::clear_pairing_code,
            pairing::reject_pairing,
            // Permission diagnostics
            permissions::get_permission_diagnostics_command,
            // Timeline commands
            timeline::get_timeline,
            timeline::clear_timeline,
            events_bus::get_recent_events,
            intent::get_intents,
            intent::dismiss_intent,
            intent::create_intent_action,
            intent::get_intent_actions,
            intent::auto_create_top_intent,
            skills::list_skills,
            skills::create_skill,
            skills::increment_skill_usage,
            skills::execute_skill,
            extensions::runtime::list_extensions,
            extensions::runtime::reload_extensions,
            extensions::runtime::execute_extension,
            extensions::runtime::list_extension_tools,
            extensions::runtime::execute_extension_tool,
            extensions::runtime::request_extension_tool_action,
            persona::get_persona,
            persona::set_persona,
            perf::get_perf_snapshot,
            notifications::push_notification,
            notifications::list_notifications,
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
            action_ledger::get_action_ledger,
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
            mcp::sandbox::apply_sandbox_baseline,
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

async fn run_update_check(app: tauri::AppHandle, install: bool) {
    let _ = app.emit("updater:checking", serde_json::json!({ "install": install }));

    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(err) => {
            let _ = app.emit(
                "updater:error",
                serde_json::json!({ "message": err.to_string() }),
            );
            return;
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            let _ = app.emit(
                "updater:available",
                serde_json::json!({
                    "version": update.version,
                    "currentVersion": update.current_version,
                    "notes": update.body,
                    "pubDate": update.date.map(|d| d.to_string())
                }),
            );

            if install {
                let _ = app.emit(
                    "updater:installing",
                    serde_json::json!({ "version": update.version }),
                );
                let result = update
                    .download_and_install(
                        |chunk, total| {
                            let _ = app.emit(
                                "updater:download-progress",
                                serde_json::json!({ "chunk": chunk, "total": total }),
                            );
                        },
                        || {
                            let _ = app.emit("updater:downloaded", serde_json::json!({}));
                        },
                    )
                    .await;

                match result {
                    Ok(()) => {
                        let _ = app.emit(
                            "updater:installed",
                            serde_json::json!({ "version": update.version }),
                        );
                        app.restart();
                    }
                    Err(err) => {
                        let _ = app.emit(
                            "updater:error",
                            serde_json::json!({ "message": err.to_string() }),
                        );
                    }
                }
            }
        }
        Ok(None) => {
            let _ = app.emit("updater:not-available", serde_json::json!({}));
        }
        Err(err) => {
            let _ = app.emit(
                "updater:error",
                serde_json::json!({ "message": err.to_string() }),
            );
        }
    }
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

#[tauri::command]
fn reset_bridge_registration() -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("No parent dir for exe")?;

    #[cfg(windows)]
    let binary_name = "native_bridge.exe";
    #[cfg(not(windows))]
    let binary_name = "native_bridge";

    let mut candidate_paths = Vec::new();
    candidate_paths.push(exe_dir.join(binary_name));

    if let Some(parent) = exe_dir.parent() {
        candidate_paths.push(parent.join(binary_name));
        candidate_paths.push(parent.join("debug").join(binary_name));
        candidate_paths.push(parent.join("release").join(binary_name));
    }

    let binary_path = candidate_paths
        .into_iter()
        .find(|path| path.exists())
        .ok_or("Native bridge binary not found. Build it and retry.")?;

    #[cfg(target_os = "macos")]
    {
        register_manifest_for_dir(
            &binary_path,
            &dirs::home_dir()
                .ok_or("No home dir")?
                .join("Library/Application Support/Google/Chrome/NativeMessagingHosts"),
        )?;
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
        register_manifest_for_dir(
            &binary_path,
            &home.join(".config/google-chrome/NativeMessagingHosts"),
        )?;
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

#[tauri::command]
fn rebuild_native_bridge() -> Result<String, String> {
    #[cfg(not(debug_assertions))]
    {
        return Err("Rebuild is only available in development builds".to_string());
    }

    #[cfg(debug_assertions)]
    {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut src_tauri_dir = None;

    if let Some(parent) = exe_path.parent() {
        if let Some(target_dir) = parent.parent() {
            if let Some(candidate) = target_dir.parent() {
                let cargo = candidate.join("Cargo.toml");
                if cargo.exists() {
                    src_tauri_dir = Some(candidate.to_path_buf());
                }
            }
        }
    }

    let src_tauri_dir = src_tauri_dir.ok_or("Unable to locate src-tauri directory")?;

    let output = std::process::Command::new("cargo")
        .arg("build")
        .arg("--bin")
        .arg("native_bridge")
        .current_dir(&src_tauri_dir)
        .output()
        .map_err(|e| format!("Failed to spawn cargo: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Bridge build failed: {}", stderr.trim()));
    }

    Ok("native_bridge rebuilt".to_string())
    }
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
