//! Minimal skill registry for repeated tasks

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const SKILLS_FILE: &str = "skills.json";

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    pub trigger: String,
    pub action_type: String,
    pub arguments: serde_json::Value,
    pub created_at: u64,
    pub usage_count: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillRegistry {
    pub skills: Vec<SkillEntry>,
}

fn skills_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("os-ghost");
    path.push(SKILLS_FILE);
    path
}

fn load_registry() -> SkillRegistry {
    let path = skills_path();
    if path.exists() {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(registry) = serde_json::from_str::<SkillRegistry>(&contents) {
                return registry;
            }
        }
    }
    SkillRegistry::default()
}

fn save_registry(registry: &SkillRegistry) -> Result<(), String> {
    let path = skills_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let contents = serde_json::to_string_pretty(registry).map_err(|e| e.to_string())?;
    fs::write(&path, contents).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_skills() -> Vec<SkillEntry> {
    load_registry().skills
}

#[tauri::command]
pub fn create_skill(
    title: String,
    description: String,
    trigger: String,
    action_type: String,
    arguments: serde_json::Value,
) -> Result<SkillEntry, String> {
    create_skill_internal(title, description, trigger, action_type, arguments)
}

#[tauri::command]
pub fn increment_skill_usage(skill_id: String) -> Result<(), String> {
    let mut registry = load_registry();
    if let Some(skill) = registry.skills.iter_mut().find(|s| s.id == skill_id) {
        if !skill.enabled {
            return Err("Skill disabled".to_string());
        }
        skill.usage_count = skill.usage_count.saturating_add(1);
        save_registry(&registry)?;
        return Ok(());
    }
    Err("Skill not found".to_string())
}

#[tauri::command]
pub fn set_skill_enabled(skill_id: String, enabled: bool) -> Result<SkillEntry, String> {
    let mut registry = load_registry();
    if let Some(skill) = registry.skills.iter_mut().find(|s| s.id == skill_id) {
        skill.enabled = enabled;
    } else {
        return Err("Skill not found".to_string());
    }
    save_registry(&registry)?;
    let updated = registry
        .skills
        .iter()
        .find(|s| s.id == skill_id)
        .cloned()
        .ok_or_else(|| "Skill not found".to_string())?;
    Ok(updated)
}

#[tauri::command]
pub fn delete_skill(skill_id: String) -> Result<(), String> {
    let mut registry = load_registry();
    let initial = registry.skills.len();
    registry.skills.retain(|s| s.id != skill_id);
    if registry.skills.len() == initial {
        return Err("Skill not found".to_string());
    }
    save_registry(&registry)?;
    Ok(())
}

#[tauri::command]
pub fn update_skill(
    skill_id: String,
    title: String,
    description: String,
    trigger: String,
) -> Result<SkillEntry, String> {
    let mut registry = load_registry();
    if let Some(skill) = registry.skills.iter_mut().find(|s| s.id == skill_id) {
        skill.title = title;
        skill.description = description;
        skill.trigger = trigger;
    } else {
        return Err("Skill not found".to_string());
    }
    save_registry(&registry)?;
    let updated = registry
        .skills
        .iter()
        .find(|s| s.id == skill_id)
        .cloned()
        .ok_or_else(|| "Skill not found".to_string())?;
    Ok(updated)
}

#[tauri::command]
pub fn execute_skill(skill_id: String) -> Result<u64, String> {
    let registry = load_registry();
    let skill = registry
        .skills
        .into_iter()
        .find(|s| s.id == skill_id)
        .ok_or_else(|| "Skill not found".to_string())?;

    if !skill.enabled {
        return Err("Skill disabled".to_string());
    }

    let pending = crate::actions::PendingAction::new(
        skill.action_type.clone(),
        format!("Skill: {}", skill.title),
        skill.trigger.clone(),
        crate::actions::ActionRiskLevel::Low,
        Some(skill.description.clone()),
        Some(skill.arguments.clone()),
    );

    let action_id = crate::actions::ACTION_QUEUE.add(pending.clone());
    crate::actions::action_ledger::record_action_created(
        action_id,
        pending.action_type,
        pending.description,
        pending.target,
        "low".to_string(),
        pending.reason,
        pending.arguments,
        Some("skill".to_string()),
    );

    Ok(action_id)
}

pub fn has_skill(action_type: &str, trigger: &str) -> bool {
    let registry = load_registry();
    registry
        .skills
        .iter()
        .any(|skill| skill.action_type == action_type && skill.trigger == trigger)
}

pub fn increment_usage_for(action_type: &str, trigger: &str) -> bool {
    let mut registry = load_registry();
    if let Some(skill) = registry
        .skills
        .iter_mut()
        .find(|s| s.action_type == action_type && s.trigger == trigger)
    {
        if !skill.enabled {
            return false;
        }
        skill.usage_count = skill.usage_count.saturating_add(1);
        let _ = save_registry(&registry);
        return true;
    }
    false
}

pub fn create_skill_internal(
    title: String,
    description: String,
    trigger: String,
    action_type: String,
    arguments: serde_json::Value,
) -> Result<SkillEntry, String> {
    let mut registry = load_registry();
    let created_at = crate::core::utils::current_timestamp();
    let id = format!("skill_{}_{}", created_at, registry.skills.len());
    let entry = SkillEntry {
        id,
        title,
        description,
        trigger,
        action_type,
        arguments,
        created_at,
        usage_count: 0,
        enabled: true,
    };
    registry.skills.push(entry.clone());
    save_registry(&registry)?;
    Ok(entry)
}
