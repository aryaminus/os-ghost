//! Action confirmation system
//! Manages pending actions that require user confirmation before execution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use crate::actions::action_ledger::{update_action_status, ActionLedgerStatus};
use crate::ai::ai_provider::SmartAiRouter;
use crate::mcp::browser::BrowserMcpServer;
use crate::data::timeline::{record_timeline_event, TimelineEntryType, TimelineStatus};
use tauri::State;

// ============================================================================
// Action Handler Registry
// ============================================================================

/// Context passed to action handlers containing all necessary dependencies
pub struct HandlerContext {
    pub action_id: u64,
    pub action_type: String,
    pub description: String,
    pub target: String,
    pub reason: Option<String>,
    pub args: serde_json::Value,
    pub effect_queue: std::sync::Arc<crate::core::game_state::EffectQueue>,
    pub session: std::sync::Arc<crate::memory::SessionMemory>,
    pub ai_router: std::sync::Arc<SmartAiRouter>,
    pub mcp_server: std::sync::Arc<BrowserMcpServer>,
    pub notes_store: std::sync::Arc<crate::integrations::integrations::NotesStore>,
}

/// Type alias for async action handler functions
type ActionHandlerFn = fn(
    HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>>;

/// Registry mapping action types to handler functions
struct HandlerRegistry {
    handlers: HashMap<String, ActionHandlerFn>,
}

impl HandlerRegistry {
    fn new() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };
        registry.register_builtin_handlers();
        registry
    }

    fn register(&mut self, action_type: &str, handler: ActionHandlerFn) {
        self.handlers.insert(action_type.to_string(), handler);
    }

    fn get(&self, action_type: &str) -> Option<ActionHandlerFn> {
        self.handlers.get(action_type).copied()
    }

    /// Register all built-in action handlers
    fn register_builtin_handlers(&mut self) {
        self.register("browser.navigate", handle_browser_navigate);
        self.register("browser.inject_effect", handle_browser_inject_effect);
        self.register("browser.highlight_text", handle_browser_highlight_text);
        self.register("sandbox.read_file", handle_sandbox_read_file);
        self.register("sandbox.write_file", handle_sandbox_write_file);
        self.register("sandbox.list_dir", handle_sandbox_list_dir);
        self.register("sandbox.shell", handle_sandbox_shell);
        self.register("notes.add", handle_notes_add);
        self.register("notes.update", handle_notes_update);
        self.register("notes.delete", handle_notes_delete);
        self.register("extension.tool", handle_extension_tool);
        self.register("intent.quick_ask", handle_intent_quick_ask);
        self.register("intent.summarize_page", handle_intent_summarize_page);
        self.register("intent.create_tasks", handle_intent_create_tasks);
        self.register("intent.draft_reply", handle_intent_draft_reply);
    }
}

// Global registry instance
lazy_static::lazy_static! {
    static ref HANDLER_REGISTRY: HandlerRegistry = HandlerRegistry::new();
}

/// Execute an action using the appropriate handler
async fn execute_with_handler(ctx: HandlerContext) -> Result<serde_json::Value, String> {
    let handler = HANDLER_REGISTRY.get(&ctx.action_type);
    match handler {
        Some(h) => h(ctx).await,
        None => Err(format!("Unknown action type: {}", ctx.action_type)),
    }
}

// ============================================================================
// Action Handler Implementations
// ============================================================================

/// Handler for browser.navigate action
fn handle_browser_navigate(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let url = ctx
            .args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or("Missing URL")?;
        ctx.effect_queue.push(crate::core::game_state::EffectMessage {
            action: "navigate".to_string(),
            effect: None,
            duration: None,
            text: None,
            url: Some(url.to_string()),
        });
        if let Some(rollback) = crate::actions::rollback::get_rollback_manager() {
            rollback.record_navigation(&ctx.action_id.to_string(), url, None);
        }
        Ok(serde_json::json!({ "navigated_to": url }))
    })
}

/// Handler for browser.inject_effect action
fn handle_browser_inject_effect(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let effect = ctx
            .args
            .get("effect")
            .and_then(|v| v.as_str())
            .ok_or("Missing effect")?;
        let duration = ctx
            .args
            .get("duration")
            .and_then(|v| v.as_u64())
            .map(|d| d.clamp(100, 10_000));
        ctx.effect_queue.push(crate::core::game_state::EffectMessage {
            action: "inject_effect".to_string(),
            effect: Some(effect.to_string()),
            duration,
            text: None,
            url: None,
        });
        if let Some(rollback) = crate::actions::rollback::get_rollback_manager() {
            rollback.record_effect(&ctx.action_id.to_string(), effect, duration.unwrap_or(1000));
        }
        Ok(serde_json::json!({ "effect_applied": effect }))
    })
}

/// Handler for browser.highlight_text action
fn handle_browser_highlight_text(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let text = ctx
            .args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("Missing text")?;
        ctx.effect_queue.push(crate::core::game_state::EffectMessage {
            action: "highlight_text".to_string(),
            effect: None,
            duration: None,
            text: Some(text.to_string()),
            url: None,
        });
        if let Some(rollback) = crate::actions::rollback::get_rollback_manager() {
            rollback.record_highlight(&ctx.action_id.to_string(), text);
        }
        Ok(serde_json::json!({ "highlighted": text }))
    })
}

/// Handler for sandbox.read_file action
fn handle_sandbox_read_file(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let path = ctx
            .args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing path")?
            .to_string();
        let result = crate::mcp::sandbox::sandbox_read_file_internal(path, false);
        if result.success {
            Ok(serde_json::to_value(result).unwrap_or_else(|_| serde_json::json!({"success": true})))
        } else {
            Err(result.error.unwrap_or_else(|| "Sandbox read failed".to_string()))
        }
    })
}

/// Handler for sandbox.write_file action
fn handle_sandbox_write_file(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let action_id = ctx.action_id;
        let plan = crate::actions::workflows::plan_for_action("sandbox.write_file", &ctx.args);
        let sim = crate::actions::workflows::simulate_plan(&plan);
        if !sim.success {
            return Err(format!(
                "guardrail: {}",
                sim.error.unwrap_or_else(|| "Workflow simulation failed".to_string())
            ));
        }

        let step_result = crate::actions::workflows::execute_plan(&plan, |step| {
            let args = step.arguments.clone();
            let step_action_type = step.action_type.clone();
            async move {
                match step_action_type.as_str() {
                    "sandbox.write_file" => {
                        let path = args
                            .get("path")
                            .and_then(|v| v.as_str())
                            .ok_or("Missing path")?
                            .to_string();
                        let content = args
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let create_dirs = args.get("create_dirs").and_then(|v| v.as_bool());
                        let result = crate::mcp::sandbox::sandbox_write_file_internal(
                            path,
                            content,
                            create_dirs,
                            false,
                            Some(action_id),
                        );
                        if result.success {
                            let output = serde_json::to_value(result)
                                .unwrap_or_else(|_| serde_json::json!({"success": true}));
                            Ok(crate::actions::workflows::WorkflowStepResult {
                                output_text: None,
                                output_json: Some(output.clone()),
                                action_id: Some(action_id),
                                rollback_action_type: Some("sandbox.write_file".to_string()),
                            })
                        } else {
                            Err(result.error.unwrap_or_else(|| "Sandbox write failed".to_string()))
                        }
                    }
                    _ => Ok(crate::actions::workflows::WorkflowStepResult {
                        output_text: None,
                        output_json: None,
                        action_id: None,
                        rollback_action_type: None,
                    }),
                }
            }
        })
        .await;

        if !step_result.success {
            let err = step_result
                .error
                .unwrap_or_else(|| "Workflow execution failed".to_string());
            record_workflow_rollback(action_id, "sandbox.write_file", &err);
            return Err(err);
        }

        Ok(step_result.output.unwrap_or_else(|| serde_json::json!({"success": true})))
    })
}

/// Handler for sandbox.list_dir action
fn handle_sandbox_list_dir(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let action_id = ctx.action_id;
        let plan = crate::actions::workflows::plan_for_action("sandbox.list_dir", &ctx.args);
        let sim = crate::actions::workflows::simulate_plan(&plan);
        if !sim.success {
            return Err(format!(
                "guardrail: {}",
                sim.error.unwrap_or_else(|| "Workflow simulation failed".to_string())
            ));
        }

        let workflow_result = crate::actions::workflows::execute_plan(&plan, |step| {
            let args = step.arguments.clone();
            let step_action_type = step.action_type.clone();
            async move {
                match step_action_type.as_str() {
                    "sandbox.list_dir" => {
                        let path = args
                            .get("path")
                            .and_then(|v| v.as_str())
                            .ok_or("Missing path")?
                            .to_string();
                        let include_hidden = args.get("include_hidden").and_then(|v| v.as_bool());
                        let result = crate::mcp::sandbox::sandbox_list_dir_internal(
                            path,
                            include_hidden,
                            false,
                        );
                        if result.success {
                            let output = serde_json::to_value(result)
                                .unwrap_or_else(|_| serde_json::json!({"success": true}));
                            Ok(crate::actions::workflows::WorkflowStepResult {
                                output_text: None,
                                output_json: Some(output.clone()),
                                action_id: Some(action_id),
                                rollback_action_type: Some("sandbox.list_dir".to_string()),
                            })
                        } else {
                            Err(result.error.unwrap_or_else(|| "Sandbox list failed".to_string()))
                        }
                    }
                    _ => Ok(crate::actions::workflows::WorkflowStepResult {
                        output_text: None,
                        output_json: None,
                        action_id: None,
                        rollback_action_type: None,
                    }),
                }
            }
        })
        .await;

        if !workflow_result.success {
            let err = workflow_result
                .error
                .unwrap_or_else(|| "Workflow execution failed".to_string());
            record_workflow_rollback(action_id, "sandbox.list_dir", &err);
            return Err(err);
        }

        Ok(workflow_result.output.unwrap_or_else(|| serde_json::json!({"success": true})))
    })
}

/// Handler for sandbox.shell action
fn handle_sandbox_shell(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let action_id = ctx.action_id;
        let plan = crate::actions::workflows::plan_for_action("sandbox.shell", &ctx.args);
        let sim = crate::actions::workflows::simulate_plan(&plan);
        if !sim.success {
            return Err(format!(
                "guardrail: {}",
                sim.error.unwrap_or_else(|| "Workflow simulation failed".to_string())
            ));
        }

        let workflow_result = crate::actions::workflows::execute_plan(&plan, |step| {
            let args = step.arguments.clone();
            let step_action_type = step.action_type.clone();
            async move {
                match step_action_type.as_str() {
                    "sandbox.shell" => {
                        let command = args
                            .get("command")
                            .and_then(|v| v.as_str())
                            .ok_or("Missing command")?
                            .to_string();
                        let working_dir =
                            args.get("working_dir").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let result = crate::mcp::sandbox::sandbox_execute_shell_internal(
                            command,
                            working_dir,
                            false,
                        )
                        .await;
                        if result.success {
                            let output = serde_json::to_value(result)
                                .unwrap_or_else(|_| serde_json::json!({"success": true}));
                            Ok(crate::actions::workflows::WorkflowStepResult {
                                output_text: None,
                                output_json: Some(output.clone()),
                                action_id: Some(action_id),
                                rollback_action_type: Some("sandbox.shell".to_string()),
                            })
                        } else {
                            Err(result.error.unwrap_or_else(|| "Sandbox shell failed".to_string()))
                        }
                    }
                    _ => Ok(crate::actions::workflows::WorkflowStepResult {
                        output_text: None,
                        output_json: None,
                        action_id: None,
                        rollback_action_type: None,
                    }),
                }
            }
        })
        .await;

        if !workflow_result.success {
            let err = workflow_result
                .error
                .unwrap_or_else(|| "Workflow execution failed".to_string());
            record_workflow_rollback(action_id, "sandbox.shell", &err);
            return Err(err);
        }

        Ok(workflow_result.output.unwrap_or_else(|| serde_json::json!({"success": true})))
    })
}

/// Handler for notes.add action
fn handle_notes_add(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let title = ctx
            .args
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let body = ctx
            .args
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let note = ctx.notes_store
            .add_note(title, body)
            .map_err(|e| e.to_string())?;
        Ok(serde_json::to_value(note).unwrap_or_else(|_| serde_json::json!({"success": true})))
    })
}

/// Handler for notes.update action
fn handle_notes_update(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let note = ctx
            .args
            .get("note")
            .cloned()
            .ok_or("Missing note")?;
        let parsed: crate::integrations::integrations::Note =
            serde_json::from_value(note).map_err(|e| e.to_string())?;
        let updated = ctx.notes_store
            .update_note(parsed)
            .map_err(|e| e.to_string())?;
        Ok(serde_json::to_value(updated).unwrap_or_else(|_| serde_json::json!({"success": true})))
    })
}

/// Handler for notes.delete action
fn handle_notes_delete(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let id = ctx
            .args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing id")?
            .to_string();
        ctx.notes_store
            .delete_note(&id)
            .map_err(|e| e.to_string())?;
        Ok(serde_json::json!({"success": true}))
    })
}

/// Handler for extension.tool action
fn handle_extension_tool(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let extension_id = ctx
            .args
            .get("extension_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing extension_id")?
            .to_string();
        let tool_name = ctx
            .args
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or("Missing tool_name")?
            .to_string();

        let mut args_list = if let Some(text) = ctx.args.get("args_text").and_then(|v| v.as_str()) {
            text.split_whitespace().map(|s| s.to_string()).collect::<Vec<_>>()
        } else if let Some(array) = ctx.args.get("args").and_then(|v| v.as_array()) {
            array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        for (key, value) in ctx.args.as_object().into_iter().flat_map(|o| o.iter()) {
            if !key.starts_with("arg.") {
                continue;
            }
            let name = key.trim_start_matches("arg.");
            let value_str = if value.is_string() {
                value.as_str().unwrap_or("").to_string()
            } else {
                value.to_string()
            };
            if !name.is_empty() {
                args_list.push(format!("--{}={}", name, value_str));
            }
        }

        let result = crate::extensions::runtime::execute_extension_tool_internal(
            extension_id,
            tool_name,
            Some(args_list),
        )
        .await;

        match result {
            Ok(output) => Ok(serde_json::to_value(output)
                .unwrap_or_else(|_| serde_json::json!({"success": true}))),
            Err(err) => Err(err),
        }
    })
}

/// Handler for intent.quick_ask action
fn handle_intent_quick_ask(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        ensure_ai_consent()?;
        validate_intent_context("intent.quick_ask", &ctx.session, &ctx.mcp_server).await?;
        let prompt = ctx
            .args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or("Missing prompt")?
            .to_string();
        let state = ctx.session.load().unwrap_or_default();
        let redacted_url = crate::config::privacy::redact_with_settings(&state.current_url);
        let redacted_title = crate::config::privacy::redact_with_settings(&state.current_title);
        let full_prompt = format!(
            "You are a fast desktop assistant. Answer succinctly (1-4 sentences).\n\nUser question: {}\n\nContext (if relevant):\n- Current URL: {}\n- Page title: {}",
            prompt, redacted_url, redacted_title
        );
        let response = ctx.ai_router
            .generate_text(&full_prompt)
            .await
            .map_err(|e| e.to_string())?;
        Ok(serde_json::json!({ "response": response }))
    })
}

/// Handler for intent.summarize_page action
fn handle_intent_summarize_page(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let action_id = ctx.action_id;
        ensure_ai_consent()?;
        validate_intent_context("intent.summarize_page", &ctx.session, &ctx.mcp_server).await?;
        let plan = crate::actions::workflows::plan_for_intent("intent.summarize_page", &ctx.args);
        let sim = crate::actions::workflows::simulate_plan(&plan);
        if !sim.success {
            return Err(format!("guardrail: {}", sim.error.unwrap_or_else(|| "Workflow simulation failed".to_string())));
        }
        let tone = ctx
            .args
            .get("tone")
            .and_then(|v| v.as_str())
            .unwrap_or("concise");
        let (content, title) = get_current_page_context(&ctx.session, &ctx.mcp_server).await;
        if content.is_empty() {
            return Err("No page content available for summary".to_string());
        }
        let redacted = crate::config::privacy::redact_with_settings(&content);
        let summary_prompt = format!(
            "Summarize this page content in a {} style. Keep it under 8 sentences.\n\nTitle: {}\n\nContent:\n{}",
            tone, title, redacted
        );
        let response = ctx.ai_router
            .generate_text(&summary_prompt)
            .await
            .map_err(|e| e.to_string())?;
        verify_intent_output("intent.summarize_page", &response)?;

        let workflow_result = crate::actions::workflows::execute_plan(&plan, |_step| {
            let response = response.clone();
            async move {
                Ok(crate::actions::workflows::WorkflowStepResult {
                    output_text: Some(response.clone()),
                    output_json: Some(serde_json::json!({ "summary": response })),
                    action_id: None,
                    rollback_action_type: None,
                })
            }
        })
        .await;

        if !workflow_result.success {
            let err = workflow_result
                .error
                .unwrap_or_else(|| "Workflow verification failed".to_string());
            record_workflow_rollback(action_id, &ctx.action_type, &err);
            return Err(err);
        }

        Ok(workflow_result.output.unwrap_or_else(|| serde_json::json!({ "summary": response })))
    })
}

/// Handler for intent.create_tasks action
fn handle_intent_create_tasks(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let action_id = ctx.action_id;
        ensure_ai_consent()?;
        validate_intent_context("intent.create_tasks", &ctx.session, &ctx.mcp_server).await?;
        let plan = crate::actions::workflows::plan_for_intent("intent.create_tasks", &ctx.args);
        let sim = crate::actions::workflows::simulate_plan(&plan);
        if !sim.success {
            return Err(format!("guardrail: {}", sim.error.unwrap_or_else(|| "Workflow simulation failed".to_string())));
        }
        let format = ctx
            .args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("checklist");
        let (content, title) = get_current_page_context(&ctx.session, &ctx.mcp_server).await;
        let redacted = crate::config::privacy::redact_with_settings(&content);
        let prompt = if !redacted.is_empty() {
            format!(
                "Create a short task list from this context. Output in {} format.\n\nTitle: {}\n\nContext:\n{}",
                format, title, redacted
            )
        } else {
            let recent = ctx.session
                .get_recent_activity(5)
                .unwrap_or_default()
                .iter()
                .map(|a| a.description.clone())
                .collect::<Vec<_>>()
                .join("; ");
            format!(
                "Create a short task list from this recent activity. Output in {} format.\n\nActivity:\n{}",
                format, recent
            )
        };
        let response = ctx.ai_router
            .generate_text(&prompt)
            .await
            .map_err(|e| e.to_string())?;
        verify_intent_output("intent.create_tasks", &response)?;

        let workflow_result = crate::actions::workflows::execute_plan(&plan, |_step| {
            let response = response.clone();
            async move {
                Ok(crate::actions::workflows::WorkflowStepResult {
                    output_text: Some(response.clone()),
                    output_json: Some(serde_json::json!({ "tasks": response })),
                    action_id: None,
                    rollback_action_type: None,
                })
            }
        })
        .await;

        if !workflow_result.success {
            let err = workflow_result
                .error
                .unwrap_or_else(|| "Workflow verification failed".to_string());
            record_workflow_rollback(action_id, &ctx.action_type, &err);
            return Err(err);
        }

        Ok(workflow_result.output.unwrap_or_else(|| serde_json::json!({ "tasks": response })))
    })
}

/// Handler for intent.draft_reply action
fn handle_intent_draft_reply(
    ctx: HandlerContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>> {
    Box::pin(async move {
        let action_id = ctx.action_id;
        ensure_ai_consent()?;
        validate_intent_context("intent.draft_reply", &ctx.session, &ctx.mcp_server).await?;
        let plan = crate::actions::workflows::plan_for_intent("intent.draft_reply", &ctx.args);
        let sim = crate::actions::workflows::simulate_plan(&plan);
        if !sim.success {
            return Err(format!("guardrail: {}", sim.error.unwrap_or_else(|| "Workflow simulation failed".to_string())));
        }
        let style = ctx
            .args
            .get("style")
            .and_then(|v| v.as_str())
            .unwrap_or("friendly");
        let (content, title) = get_current_page_context(&ctx.session, &ctx.mcp_server).await;
        let redacted = crate::config::privacy::redact_with_settings(&content);
        if redacted.is_empty() {
            return Err("No page content available for draft".to_string());
        }
        let prompt = format!(
            "Draft a {} reply based on this context. Keep it short.\n\nTitle: {}\n\nContext:\n{}",
            style, title, redacted
        );
        let response = ctx.ai_router
            .generate_text(&prompt)
            .await
            .map_err(|e| e.to_string())?;
        verify_intent_output("intent.draft_reply", &response)?;

        let workflow_result = crate::actions::workflows::execute_plan(&plan, |_step| {
            let response = response.clone();
            async move {
                Ok(crate::actions::workflows::WorkflowStepResult {
                    output_text: Some(response.clone()),
                    output_json: Some(serde_json::json!({ "draft": response })),
                    action_id: None,
                    rollback_action_type: None,
                })
            }
        })
        .await;

        if !workflow_result.success {
            let err = workflow_result
                .error
                .unwrap_or_else(|| "Workflow verification failed".to_string());
            record_workflow_rollback(action_id, &ctx.action_type, &err);
            return Err(err);
        }

        Ok(workflow_result.output.unwrap_or_else(|| serde_json::json!({ "draft": response })))
    })
}

// ============================================================================

/// Unique action ID counter
static ACTION_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Risk level for actions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionRiskLevel {
    /// Low risk: visual effects, highlights
    Low,
    /// Medium risk: navigation within known domains
    Medium,
    /// High risk: navigation to external sites, form submissions
    High,
}

impl ActionRiskLevel {
    /// Determine if this is considered high-risk for confirmation purposes
    pub fn is_high_risk(&self) -> bool {
        matches!(self, ActionRiskLevel::High)
    }
}

/// Status of a pending action
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    /// Waiting for user confirmation
    Pending,
    /// User approved the action
    Approved,
    /// User denied the action
    Denied,
    /// Action expired (timed out)
    Expired,
    /// Action was executed successfully
    Executed,
    /// Action failed during execution
    Failed,
}

/// A pending action awaiting user confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAction {
    /// Unique action ID
    pub id: u64,
    /// Type of action (e.g., "browser.navigate", "browser.inject_effect")
    pub action_type: String,
    /// Human-readable description of what the action will do
    pub description: String,
    /// The target (e.g., URL, element)
    pub target: String,
    /// Risk level
    pub risk_level: ActionRiskLevel,
    /// Current status
    pub status: ActionStatus,
    /// Timestamp when action was created (seconds since UNIX epoch)
    pub created_at: u64,
    /// Optional reason for the action (from AI)
    pub reason: Option<String>,
    /// The original arguments to pass to the tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

impl PendingAction {
    /// Create a new pending action
    pub fn new(
        action_type: String,
        description: String,
        target: String,
        risk_level: ActionRiskLevel,
        reason: Option<String>,
        arguments: Option<serde_json::Value>,
    ) -> Self {
        let id = ACTION_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            id,
            action_type,
            description,
            target,
            risk_level,
            status: ActionStatus::Pending,
            created_at,
            reason,
            arguments,
        }
    }

    /// Check if this action has expired (default: 60 seconds)
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.created_at) > 60
    }
}

/// Manages pending actions globally
#[derive(Debug, Default)]
pub struct ActionQueue {
    /// Map of action ID to pending action
    actions: RwLock<HashMap<u64, PendingAction>>,
    /// Action history (for audit log)
    history: RwLock<Vec<PendingAction>>,
}

impl ActionQueue {
    /// Create a new action queue
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new pending action
    pub fn add(&self, action: PendingAction) -> u64 {
        let id = action.id;
        if let Ok(mut actions) = self.actions.write() {
            actions.insert(id, action);
        }
        id
    }

    /// Get a pending action by ID
    pub fn get(&self, id: u64) -> Option<PendingAction> {
        self.actions.read().ok()?.get(&id).cloned()
    }

    /// Get all pending actions
    pub fn get_pending(&self) -> Vec<PendingAction> {
        self.actions
            .read()
            .ok()
            .map(|actions| {
                actions
                    .values()
                    .filter(|a| a.status == ActionStatus::Pending && !a.is_expired())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Approve an action
    pub fn approve(&self, id: u64) -> Option<PendingAction> {
        self.update_status_in_queue(id, ActionStatus::Approved)
    }

    /// Deny an action
    pub fn deny(&self, id: u64) -> Option<PendingAction> {
        self.remove_and_archive(id, ActionStatus::Denied)
    }

    /// Mark action as executed
    pub fn mark_executed(&self, id: u64) -> Option<PendingAction> {
        self.remove_and_archive(id, ActionStatus::Executed)
    }

    /// Mark action as failed
    pub fn mark_failed(&self, id: u64) -> Option<PendingAction> {
        self.remove_and_archive(id, ActionStatus::Failed)
    }

    /// Update action status in queue (keeps action available for execution)
    fn update_status_in_queue(&self, id: u64, status: ActionStatus) -> Option<PendingAction> {
        let mut actions = self.actions.write().ok()?;
        let action = actions.get_mut(&id)?;
        action.status = status;
        Some(action.clone())
    }

    /// Update action arguments in queue
    pub fn update_arguments(&self, id: u64, arguments: Option<serde_json::Value>) -> Option<PendingAction> {
        let mut actions = self.actions.write().ok()?;
        let action = actions.get_mut(&id)?;
        action.arguments = arguments;
        Some(action.clone())
    }

    /// Remove action from queue and move to history
    fn remove_and_archive(&self, id: u64, status: ActionStatus) -> Option<PendingAction> {
        let mut action = {
            let mut actions = self.actions.write().ok()?;
            actions.remove(&id)?
        };
        action.status = status;

        // Add to history
        if let Ok(mut history) = self.history.write() {
            history.push(action.clone());
            // Keep only last 100 actions in history
            if history.len() > 100 {
                history.remove(0);
            }
        }

        Some(action)
    }

    /// Get action history
    pub fn get_history(&self, limit: usize) -> Vec<PendingAction> {
        self.history
            .read()
            .ok()
            .map(|history| history.iter().rev().take(limit).cloned().collect())
            .unwrap_or_default()
    }

    /// Clean up expired actions
    pub fn cleanup_expired(&self) {
        if let Ok(mut actions) = self.actions.write() {
            let expired_ids: Vec<u64> = actions
                .iter()
                .filter(|(_, a)| a.is_expired())
                .map(|(id, _)| *id)
                .collect();

            for id in expired_ids {
                if let Some(mut action) = actions.remove(&id) {
                    action.status = ActionStatus::Expired;
                    if let Ok(mut history) = self.history.write() {
                        history.push(action);
                        if history.len() > 100 {
                            history.remove(0);
                        }
                    }
                }
            }
        }
    }
}

// Global action queue instance
lazy_static::lazy_static! {
    pub static ref ACTION_QUEUE: ActionQueue = ActionQueue::new();
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get all pending actions awaiting confirmation
#[tauri::command]
pub fn get_pending_actions() -> Vec<PendingAction> {
    ACTION_QUEUE.cleanup_expired();
    ACTION_QUEUE.get_pending()
}

/// Approve a pending action
#[tauri::command]
pub fn approve_action(action_id: u64) -> Result<PendingAction, String> {
    let action = ACTION_QUEUE
        .approve(action_id)
        .ok_or_else(|| format!("Action {} not found or already processed", action_id))?;

    update_action_status(
        action_id,
        ActionLedgerStatus::Approved,
        None,
        None,
    );

    record_timeline_event(
        &format!("Action approved: {}", action.action_type),
        action.reason.clone(),
        TimelineEntryType::Action,
        TimelineStatus::Approved,
    );

    Ok(action)
}

/// Deny a pending action
#[tauri::command]
pub fn deny_action(action_id: u64) -> Result<PendingAction, String> {
    let action = ACTION_QUEUE
        .deny(action_id)
        .ok_or_else(|| format!("Action {} not found or already processed", action_id))?;

    update_action_status(
        action_id,
        ActionLedgerStatus::Denied,
        None,
        None,
    );

    record_timeline_event(
        &format!("Action denied: {}", action.action_type),
        action.reason.clone(),
        TimelineEntryType::Action,
        TimelineStatus::Denied,
    );

    Ok(action)
}

/// Get action history (audit log)
#[tauri::command]
pub fn get_action_history(limit: Option<usize>) -> Vec<PendingAction> {
    ACTION_QUEUE.get_history(limit.unwrap_or(50))
}

/// Clear all pending actions (deny all)
#[tauri::command]
pub fn clear_pending_actions() -> usize {
    let pending = ACTION_QUEUE.get_pending();
    let count = pending.len();
    for action in pending {
        ACTION_QUEUE.deny(action.id);
    }
    count
}

/// Clear action history
#[tauri::command]
pub fn clear_action_history() -> usize {
    let history = ACTION_QUEUE.get_history(1000);
    let count = history.len();
    if let Ok(mut history_guard) = ACTION_QUEUE.history.write() {
        history_guard.clear();
    }
    count
}

/// Execute an approved action by ID
/// Returns the action data on success, or an error message
#[tauri::command]
pub async fn execute_approved_action(
    action_id: u64,
    effect_queue: tauri::State<'_, std::sync::Arc<crate::core::game_state::EffectQueue>>,
    session: State<'_, std::sync::Arc<crate::memory::SessionMemory>>,
    ai_router: State<'_, std::sync::Arc<SmartAiRouter>>,
    mcp_server: State<'_, std::sync::Arc<BrowserMcpServer>>,
    notes_store: State<'_, std::sync::Arc<crate::integrations::integrations::NotesStore>>,
) -> Result<serde_json::Value, String> {
    // Get the action and verify it's approved
    let action = ACTION_QUEUE
        .get(action_id)
        .ok_or_else(|| format!("Action {} not found", action_id))?;

    if action.status != ActionStatus::Approved {
        return Err(format!(
            "Action {} is not approved (status: {:?})",
            action_id, action.status
        ));
    }

    // Get the arguments
    let args = action
        .arguments
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));

    // Execute based on action type using handler registry
    let ctx = HandlerContext {
        action_id,
        action_type: action.action_type.clone(),
        description: action.description.clone(),
        target: action.target.clone(),
        reason: action.reason.clone(),
        args,
        effect_queue: effect_queue.inner().clone(),
        session: session.inner().clone(),
        ai_router: ai_router.inner().clone(),
        mcp_server: mcp_server.inner().clone(),
        notes_store: notes_store.inner().clone(),
    };

    let result = execute_with_handler(ctx).await;

    let result = match result {
        Ok(value) => value,
        Err(err) => {
            if err.starts_with("guardrail:") {
                ACTION_QUEUE.deny(action_id);
                update_action_status(
                    action_id,
                    ActionLedgerStatus::Denied,
                    None,
                    Some(err.clone()),
                );
                record_timeline_event(
                    &format!("Action denied: {}", action.action_type),
                    Some(err.clone()),
                    TimelineEntryType::Action,
                    TimelineStatus::Denied,
                );
                crate::data::events_bus::record_event(
                    crate::data::events_bus::EventKind::Guardrail,
                    format!("Guardrail denied action: {}", action.action_type),
                    Some(err.clone()),
                    std::collections::HashMap::new(),
                    crate::data::events_bus::EventPriority::Normal,
                    Some(format!("guardrail:{}", action_id)),
                    Some(600),
                    Some("guardrail".to_string()),
                );
                return Err(err);
            }
            ACTION_QUEUE.mark_failed(action_id);
            update_action_status(
                action_id,
                ActionLedgerStatus::Failed,
                None,
                Some(err.clone()),
            );
            record_timeline_event(
                &format!("Action failed: {}", action.action_type),
                Some(err.clone()),
                TimelineEntryType::Action,
                TimelineStatus::Failed,
            );
            crate::data::events_bus::record_event(
                crate::data::events_bus::EventKind::Action,
                format!("Action failed: {}", action.action_type),
                Some(err.clone()),
                std::collections::HashMap::new(),
                crate::data::events_bus::EventPriority::Normal,
                Some(format!("action_failed:{}", action_id)),
                Some(600),
                Some("actions".to_string()),
            );
            return Err(err);
        }
    };

    // Mark as executed
    ACTION_QUEUE.mark_executed(action_id);
    update_action_status(
        action_id,
        ActionLedgerStatus::Executed,
        Some(result.clone()),
        None,
    );
    record_timeline_event(
        &format!("Action executed: {}", action.action_type),
        action.reason.clone(),
        TimelineEntryType::Action,
        TimelineStatus::Executed,
    );
    maybe_create_skill(&action);
    crate::data::events_bus::record_event(
        crate::data::events_bus::EventKind::Action,
        format!("Action executed: {}", action.action_type),
        action.reason.clone(),
        std::collections::HashMap::new(),
        crate::data::events_bus::EventPriority::Low,
        Some(format!("action_executed:{}", action_id)),
        Some(600),
        Some("actions".to_string()),
    );

    Ok(result)
}

fn ensure_ai_consent() -> Result<(), String> {
    let privacy = crate::config::privacy::PrivacySettings::load();
    if !privacy.ai_analysis_consent {
        return Err("AI analysis consent required".to_string());
    }
    Ok(())
}

fn guardrail_deny(reason: &str) -> Result<(), String> {
    Err(format!("guardrail: {}", reason))
}

fn record_workflow_rollback(action_id: u64, action_type: &str, reason: &str) {
    record_timeline_event(
        &format!("Workflow rollback: {}", action_type),
        Some(reason.to_string()),
        TimelineEntryType::Action,
        TimelineStatus::Failed,
    );
    crate::data::events_bus::record_event(
        crate::data::events_bus::EventKind::Action,
        format!("Workflow rollback: {}", action_type),
        Some(reason.to_string()),
        std::collections::HashMap::new(),
        crate::data::events_bus::EventPriority::Normal,
        Some(format!("workflow_rollback:{}", action_id)),
        Some(600),
        Some("workflow".to_string()),
    );

    let rollback = crate::actions::workflows::rollback_action(action_type, action_id);
    if !rollback.success {
        crate::data::events_bus::record_event(
            crate::data::events_bus::EventKind::Guardrail,
            format!("Rollback failed: {}", action_type),
            rollback.error.clone(),
            std::collections::HashMap::new(),
            crate::data::events_bus::EventPriority::Normal,
            Some(format!("workflow_rollback_failed:{}", action_id)),
            Some(600),
            Some("workflow".to_string()),
        );
    }
}

fn verify_intent_output(intent_type: &str, output: &str) -> Result<(), String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return guardrail_deny("Empty response");
    }

    match intent_type {
        "intent.summarize_page" => {
            if trimmed.len() < 60 {
                return guardrail_deny("Summary too short");
            }
        }
        "intent.create_tasks" => {
            let lines = trimmed.lines().count();
            if lines < 2 {
                return guardrail_deny("Task list too short");
            }
        }
        "intent.draft_reply" => {
            if trimmed.len() < 40 {
                return guardrail_deny("Draft reply too short");
            }
        }
        _ => {}
    }

    Ok(())
}

async fn validate_intent_context(
    intent_type: &str,
    session: &std::sync::Arc<crate::memory::SessionMemory>,
    mcp_server: &std::sync::Arc<BrowserMcpServer>,
) -> Result<(), String> {
    let now = crate::core::utils::current_timestamp() as i64;
    let browser_state = mcp_server.state();
    let page = browser_state.current_page.read().await;
    let age_secs = now.saturating_sub(page.timestamp).max(0) as u64;
    let stale = page.timestamp > 0 && age_secs > 1800;
    let page_body_empty = page.body_text.is_empty();
    drop(page);

    let state = session.load().unwrap_or_default();
    let content_empty = state.current_content.as_deref().unwrap_or("").is_empty() && page_body_empty;

    match intent_type {
        "intent.summarize_page" | "intent.draft_reply" => {
            if content_empty {
                return guardrail_deny("No page content available");
            }
            if stale {
                return guardrail_deny("Page context is stale");
            }
        }
        "intent.create_tasks" => {
            if content_empty {
                let recent = session.get_recent_activity(5).unwrap_or_default();
                if recent.is_empty() {
                    return guardrail_deny("No recent activity available");
                }
            }
        }
        _ => {}
    }

    Ok(())
}

async fn get_current_page_context(
    session: &std::sync::Arc<crate::memory::SessionMemory>,
    mcp_server: &std::sync::Arc<BrowserMcpServer>,
) -> (String, String) {
    let state = session.load().unwrap_or_default();
    let mut content = state.current_content.unwrap_or_default();
    let mut title = state.current_title.clone();

    if content.is_empty() || title.is_empty() {
        let browser_state = mcp_server.state();
        let page = browser_state.current_page.read().await;
        if content.is_empty() {
            content = page.body_text.clone();
        }
        if title.is_empty() {
            title = page.title.clone();
        }
    }

    (content, title)
}

fn maybe_create_skill(action: &PendingAction) {
    if !action.action_type.starts_with("intent.") {
        return;
    }

    let trigger = action.description.clone();
    if crate::data::skills::increment_usage_for(&action.action_type, &trigger) {
        return;
    }
    if crate::data::skills::has_skill(&action.action_type, &trigger) {
        return;
    }

    // Require repeat before creating a skill
    let history = ACTION_QUEUE.get_history(50);
    let repeat_count = history
        .iter()
        .filter(|item| {
            item.action_type == action.action_type && item.description == action.description
        })
        .count();
    if repeat_count < 2 {
        return;
    }

    let title = format!("{} (Skill)", action.description);
    let description = action
        .reason
        .clone()
        .unwrap_or_else(|| "Auto-created from repeated intent action".to_string());
    let args = action.arguments.clone().unwrap_or_else(|| serde_json::json!({}));

    let _ = crate::data::skills::create_skill_internal(
        title,
        description,
        trigger,
        action.action_type.clone(),
        args,
    );
}

// ============================================================================
// Action Preview Tauri Commands
// ============================================================================

/// Get the currently active action preview
#[tauri::command]
pub fn get_active_preview() -> Option<crate::actions::action_preview::ActionPreview> {
    crate::actions::action_preview::get_preview_manager()
        .and_then(|m| m.get_active_preview())
}

/// Approve a preview and execute it
#[tauri::command]
pub async fn approve_preview(
    preview_id: String,
    effect_queue: tauri::State<'_, std::sync::Arc<crate::core::game_state::EffectQueue>>,
    session: State<'_, std::sync::Arc<crate::memory::SessionMemory>>,
    ai_router: State<'_, std::sync::Arc<SmartAiRouter>>,
    mcp_server: State<'_, std::sync::Arc<BrowserMcpServer>>,
    notes_store: State<'_, std::sync::Arc<crate::integrations::integrations::NotesStore>>,
) -> Result<(), String> {
    let action_id = approve_preview_internal(&preview_id)?;

    execute_approved_action(action_id, effect_queue, session, ai_router, mcp_server, notes_store).await?;
    Ok(())
}

fn approve_preview_internal(preview_id: &str) -> Result<u64, String> {
    let manager = crate::actions::action_preview::get_preview_manager_mut()
        .ok_or("Preview manager not initialized")?;
    let preview = manager
        .get_active_preview()
        .ok_or("No active preview".to_string())?;

    if preview.id != preview_id {
        return Err("Preview ID mismatch".to_string());
    }

    let updated_args = preview.updated_arguments();
    let action_id = preview.action.id;

    if updated_args.is_some() {
        ACTION_QUEUE.update_arguments(action_id, updated_args);
    }

    manager.approve_preview(preview_id)?;

    ACTION_QUEUE
        .approve(action_id)
        .ok_or_else(|| format!("Action {} not found or already processed", action_id))?;

    Ok(action_id)
}

/// Deny a preview
#[tauri::command]
pub fn deny_preview(preview_id: String, reason: Option<String>) -> Result<(), String> {
    let action_id = {
        let manager = crate::actions::action_preview::get_preview_manager_mut()
            .ok_or("Preview manager not initialized")?;
        let preview = manager
            .get_active_preview()
            .ok_or("No active preview".to_string())?;

        if preview.id != preview_id {
            return Err("Preview ID mismatch".to_string());
        }

        let action_id = preview.action.id;
        manager.deny_preview(&preview_id, reason)?;
        action_id
    };

    ACTION_QUEUE.deny(action_id);
    Ok(())
}

/// Update a preview parameter
#[tauri::command]
pub fn update_preview_param(
    preview_id: String,
    param_name: String,
    value: serde_json::Value,
) -> Result<crate::actions::action_preview::ActionPreview, String> {
    let manager = crate::actions::action_preview::get_preview_manager_mut()
        .ok_or("Preview manager not initialized")?;
    manager.update_param(&preview_id, &param_name, value)?;
    manager.get_active_preview().ok_or("No active preview".to_string())
}

// ============================================================================
// Rollback/Undo Tauri Commands
// ============================================================================

/// Get the current rollback status
#[tauri::command]
pub fn get_rollback_status() -> crate::actions::rollback::RollbackStatus {
    crate::actions::rollback::get_rollback_manager()
        .map(|m| m.get_status())
        .unwrap_or_else(|| crate::actions::rollback::RollbackStatus {
            can_undo: false,
            can_redo: false,
            undo_description: None,
            redo_description: None,
            stack_size: 0,
            recent_actions: vec![],
        })
}

/// Undo the last action
#[tauri::command]
pub fn undo_action() -> crate::actions::rollback::UndoResult {
    crate::actions::rollback::get_rollback_manager()
        .map(|m| m.undo())
        .unwrap_or_else(|| crate::actions::rollback::UndoResult {
            success: false,
            action: None,
            error: Some("Rollback manager not initialized".to_string()),
            restored_state: None,
        })
}

/// Redo the last undone action
#[tauri::command]
pub fn redo_action() -> crate::actions::rollback::UndoResult {
    crate::actions::rollback::get_rollback_manager()
        .map(|m| m.redo())
        .unwrap_or_else(|| crate::actions::rollback::UndoResult {
            success: false,
            action: None,
            error: Some("Rollback manager not initialized".to_string()),
            restored_state: None,
        })
}
