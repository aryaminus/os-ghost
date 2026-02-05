//! Minimal workflow runner for multi-step tasks

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub action_type: String,
    pub description: String,
    pub arguments: serde_json::Value,
    #[serde(default)]
    pub rollback_action_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPlan {
    pub id: String,
    pub title: String,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub success: bool,
    pub executed_steps: usize,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepResult {
    pub output_text: Option<String>,
    pub output_json: Option<serde_json::Value>,
    pub action_id: Option<u64>,
    pub rollback_action_type: Option<String>,
}

/// Simple plan builder for intent actions
pub fn plan_for_intent(action_type: &str, args: &serde_json::Value) -> WorkflowPlan {
    let id = format!("workflow_{}", crate::core::utils::current_timestamp());
    let title = format!("Workflow for {}", action_type);
    let step = WorkflowStep {
        id: format!("step_{}", crate::core::utils::current_timestamp()),
        action_type: action_type.to_string(),
        description: format!("Execute {}", action_type),
        arguments: args.clone(),
        rollback_action_type: None,
    };
    let verify = WorkflowStep {
        id: format!("step_verify_{}", crate::core::utils::current_timestamp()),
        action_type: "verify.output".to_string(),
        description: "Verify output".to_string(),
        arguments: serde_json::json!({ "action_type": action_type }),
        rollback_action_type: None,
    };
    WorkflowPlan {
        id,
        title,
        steps: vec![step, verify],
    }
}

pub fn plan_for_action(action_type: &str, args: &serde_json::Value) -> WorkflowPlan {
    let id = format!("workflow_{}", crate::core::utils::current_timestamp());
    let title = format!("Workflow for {}", action_type);
    let mut steps = Vec::new();
    let rollback_action_type = match action_type {
        "sandbox.write_file" | "notes.add" | "notes.update" | "notes.delete" => {
            Some(action_type.to_string())
        }
        _ => None,
    };

    steps.push(WorkflowStep {
        id: format!("step_{}", crate::core::utils::current_timestamp()),
        action_type: action_type.to_string(),
        description: format!("Execute {}", action_type),
        arguments: args.clone(),
        rollback_action_type,
    });

    if action_type == "sandbox.write_file" {
        let path = args.get("path").cloned().unwrap_or(serde_json::Value::Null);
        let content = args.get("content").cloned().unwrap_or(serde_json::Value::Null);
        steps.push(WorkflowStep {
            id: format!("step_verify_{}", crate::core::utils::current_timestamp()),
            action_type: "verify.sandbox_write".to_string(),
            description: "Verify sandbox write".to_string(),
            arguments: serde_json::json!({ "path": path, "content": content }),
            rollback_action_type: None,
        });
    } else if action_type == "sandbox.list_dir" {
        let path = args.get("path").cloned().unwrap_or(serde_json::Value::Null);
        steps.push(WorkflowStep {
            id: format!("step_verify_{}", crate::core::utils::current_timestamp()),
            action_type: "verify.list_dir".to_string(),
            description: "Verify list dir".to_string(),
            arguments: serde_json::json!({ "path": path }),
            rollback_action_type: None,
        });
    } else if action_type == "sandbox.shell" {
        let command = args.get("command").cloned().unwrap_or(serde_json::Value::Null);
        steps.push(WorkflowStep {
            id: format!("step_verify_{}", crate::core::utils::current_timestamp()),
            action_type: "verify.shell".to_string(),
            description: "Verify shell result".to_string(),
            arguments: serde_json::json!({ "command": command }),
            rollback_action_type: None,
        });
    } else if action_type.starts_with("notes.") {
        let id = args.get("id").cloned().unwrap_or(serde_json::Value::Null);
        steps.push(WorkflowStep {
            id: format!("step_verify_{}", crate::core::utils::current_timestamp()),
            action_type: "verify.notes".to_string(),
            description: "Verify notes change".to_string(),
            arguments: serde_json::json!({ "action_type": action_type, "id": id }),
            rollback_action_type: None,
        });
    }

    WorkflowPlan { id, title, steps }
}

pub fn simulate_plan(plan: &WorkflowPlan) -> WorkflowResult {
    if plan.steps.is_empty() {
        return WorkflowResult {
            success: false,
            executed_steps: 0,
            error: Some("No steps in workflow".to_string()),
            output: None,
        };
    }
    WorkflowResult {
        success: true,
        executed_steps: 0,
        error: None,
        output: None,
    }
}

pub fn verify_output(action_type: &str, output: &str) -> Result<(), String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err("guardrail: Workflow output empty".to_string());
    }

    if action_type.starts_with("intent.") && trimmed.len() < 20 {
        return Err("guardrail: Workflow output too short".to_string());
    }

    Ok(())
}

pub fn rollback_plan(_plan: &WorkflowPlan) -> WorkflowResult {
    WorkflowResult {
        success: true,
        executed_steps: 0,
        error: None,
        output: None,
    }
}

pub async fn execute_plan<F, Fut>(plan: &WorkflowPlan, mut exec: F) -> WorkflowResult
where
    F: FnMut(&WorkflowStep) -> Fut,
    Fut: std::future::Future<Output = Result<WorkflowStepResult, String>>,
{
    let mut executed_steps = 0;
    let mut rollback_stack: Vec<(String, u64)> = Vec::new();
    let mut last_output_text: Option<String> = None;
    let mut last_output_json: Option<serde_json::Value> = None;
    let mut last_step_action: Option<String> = None;

    for step in &plan.steps {
        if step.action_type.starts_with("verify.") {
            if let Err(err) = verify_step(
                step,
                last_output_text.as_deref(),
                last_output_json.as_ref(),
                last_step_action.as_deref(),
            ) {
                rollback_stack.reverse();
                for (action_type, action_id) in rollback_stack {
                    let _ = rollback_action(&action_type, action_id);
                }
                return WorkflowResult {
                    success: false,
                    executed_steps,
                    error: Some(err),
                    output: last_output_json,
                };
            }
            continue;
        }

        match exec(step).await {
            Ok(step_result) => {
                executed_steps += 1;
                if let Some(output) = step_result.output_text {
                    last_output_text = Some(output);
                }
                if let Some(output) = step_result.output_json {
                    last_output_json = Some(output);
                }
                last_step_action = Some(step.action_type.clone());
                if let (Some(action_type), Some(action_id)) =
                    (step_result.rollback_action_type, step_result.action_id)
                {
                    rollback_stack.push((action_type, action_id));
                }
            }
            Err(err) => {
                rollback_stack.reverse();
                for (action_type, action_id) in rollback_stack {
                    let _ = rollback_action(&action_type, action_id);
                }
                return WorkflowResult {
                    success: false,
                    executed_steps,
                    error: Some(err),
                    output: last_output_json,
                };
            }
        }
    }

    WorkflowResult {
        success: true,
        executed_steps,
        error: None,
        output: last_output_json,
    }
}

fn verify_step(
    step: &WorkflowStep,
    output_text: Option<&str>,
    output_json: Option<&serde_json::Value>,
    last_action_type: Option<&str>,
) -> Result<(), String> {
    match step.action_type.as_str() {
        "verify.output" => {
            let action_type = step
                .arguments
                .get("action_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let output = output_text.unwrap_or("");
            verify_output(action_type, output)
        }
        "verify.sandbox_write" => {
            let path = step
                .arguments
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Missing path for verification")?;
            let content = step
                .arguments
                .get("content")
                .and_then(|v| v.as_str());
            let path_buf = std::path::PathBuf::from(path);
            if !path_buf.exists() {
                return Err("guardrail: File write verification failed".to_string());
            }
            if let Some(expected) = content {
                let actual = std::fs::read_to_string(&path_buf)
                    .map_err(|_| "guardrail: Unable to read file for verification".to_string())?;
                if actual != expected {
                    return Err("guardrail: File content mismatch".to_string());
                }
            }
            Ok(())
        }
        "verify.list_dir" => {
            let path = step
                .arguments
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("Missing path for verification")?;
            let path_buf = std::path::PathBuf::from(path);
            if !path_buf.exists() {
                return Err("guardrail: Directory missing".to_string());
            }
            if !path_buf.is_dir() {
                return Err("guardrail: Path is not a directory".to_string());
            }
            let config = crate::mcp::sandbox::get_sandbox_config();
            if let Err(err) = config.can_read(&path_buf) {
                return Err(format!("guardrail: Directory not allowed: {:?}", err));
            }
            let json = output_json.ok_or("guardrail: Missing list output")?;
            let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            if !success {
                return Err("guardrail: List directory failed".to_string());
            }
            let output_path = json
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if output_path != path {
                return Err("guardrail: List path mismatch".to_string());
            }
            let entries_len = json
                .get("entries")
                .and_then(|v| v.as_array())
                .map(|v| v.len())
                .unwrap_or(0);
            let count = json.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if entries_len != count {
                return Err("guardrail: List count mismatch".to_string());
            }
            Ok(())
        }
        "verify.shell" => {
            let json = output_json.ok_or("guardrail: Missing shell output")?;
            let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            if !success {
                return Err("guardrail: Shell command failed".to_string());
            }
            let expected_command = step
                .arguments
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let actual_command = json.get("command").and_then(|v| v.as_str()).unwrap_or("");
            if !expected_command.is_empty() && actual_command != expected_command {
                return Err("guardrail: Shell command mismatch".to_string());
            }
            let category = json.get("category").and_then(|v| v.as_str()).unwrap_or("");
            if category == "blocked" || category.is_empty() {
                return Err("guardrail: Shell category blocked".to_string());
            }
            let exit_code = json.get("exit_code").and_then(|v| v.as_i64());
            if let Some(code) = exit_code {
                if code != 0 {
                    return Err("guardrail: Shell exit code nonzero".to_string());
                }
            }
            if let Some(action_type) = last_action_type {
                if action_type != "sandbox.shell" {
                    return Err("guardrail: Shell verify mismatch".to_string());
                }
            }
            Ok(())
        }
        "verify.notes" => {
            let action_type = step
                .arguments
                .get("action_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let json = output_json.ok_or("guardrail: Missing notes output")?;
            match action_type {
                "notes.add" | "notes.update" => {
                    let id = json.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if id.is_empty() {
                        return Err("guardrail: Note id missing".to_string());
                    }
                    let store = crate::memory::MemoryStore::new()
                        .map_err(|_| "guardrail: Notes store unavailable".to_string())?;
                    let note: Option<crate::integrations::integrations::Note> = store
                        .get("notes", id)
                        .map_err(|_| "guardrail: Note lookup failed".to_string())?;
                    let note = note.ok_or("guardrail: Note missing after write".to_string())?;
                    let expected_title = json
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let expected_body = json
                        .get("body")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if note.title != expected_title || note.body != expected_body {
                        return Err("guardrail: Note content mismatch".to_string());
                    }
                }
                "notes.delete" => {
                    let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                    if !success {
                        return Err("guardrail: Note delete failed".to_string());
                    }
                    if let Some(id) = step.arguments.get("id").and_then(|v| v.as_str()) {
                        let store = crate::memory::MemoryStore::new()
                            .map_err(|_| "guardrail: Notes store unavailable".to_string())?;
                        let note: Option<crate::integrations::integrations::Note> = store
                            .get("notes", id)
                            .map_err(|_| "guardrail: Note lookup failed".to_string())?;
                        if note.is_some() {
                            return Err("guardrail: Note still present after delete".to_string());
                        }
                    }
                }
                _ => {}
            }
            if let Some(action_type) = last_action_type {
                if action_type != "notes.add"
                    && action_type != "notes.update"
                    && action_type != "notes.delete"
                {
                    return Err("guardrail: Notes verify mismatch".to_string());
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub fn rollback_action(action_type: &str, action_id: u64) -> WorkflowResult {
    if let Some(rollback) = crate::actions::rollback::get_rollback_manager() {
        let action_id = action_id.to_string();
        match action_type {
            "browser.navigate" | "browser.highlight_text" | "browser.inject_effect" => {
                let result = rollback.undo();
                if result.success {
                    return WorkflowResult {
                        success: true,
                        executed_steps: 1,
                        error: None,
                        output: None,
                    };
                }
                return WorkflowResult {
                    success: false,
                    executed_steps: 0,
                    error: result.error,
                    output: None,
                };
            }
            "sandbox.write_file" | "notes.add" | "notes.update" | "notes.delete" => {
                let result = rollback.undo();
                if result.success {
                    return WorkflowResult {
                        success: true,
                        executed_steps: 1,
                        error: None,
                        output: None,
                    };
                }
                return WorkflowResult {
                    success: false,
                    executed_steps: 0,
                    error: result.error,
                    output: None,
                };
            }
            "sandbox.shell" | "sandbox.list_dir" => {
                rollback.undo_stack.block_undo(&action_id, "Rollback not supported for sandbox read/shell");
            }
            _ => {
                rollback.undo_stack.block_undo(&action_id, "No rollback handler for action type");
            }
        }
    }

    WorkflowResult {
        success: false,
        executed_steps: 0,
        error: Some("Rollback manager unavailable".to_string()),
        output: None,
    }
}
