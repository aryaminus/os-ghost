//! Persona profiles for companion behavior

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const PERSONA_FILE: &str = "persona.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tone: String,
    pub hint_density: f32,
    pub action_aggressiveness: f32,
    pub allow_auto_intents: bool,
}

impl Default for PersonaProfile {
    fn default() -> Self {
        Self {
            id: "quiet".to_string(),
            name: "Quiet".to_string(),
            description: "Minimal interruptions, subtle suggestions.".to_string(),
            tone: "calm and minimal".to_string(),
            hint_density: 0.3,
            action_aggressiveness: 0.2,
            allow_auto_intents: false,
        }
    }
}

fn persona_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("os-ghost");
    path.push(PERSONA_FILE);
    path
}

#[tauri::command]
pub fn get_persona() -> PersonaProfile {
    let path = persona_path();
    if path.exists() {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(profile) = serde_json::from_str::<PersonaProfile>(&contents) {
                return profile;
            }
        }
    }
    PersonaProfile::default()
}

#[tauri::command]
pub fn set_persona(profile: PersonaProfile) -> Result<PersonaProfile, String> {
    let path = persona_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let contents = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    fs::write(&path, contents).map_err(|e| e.to_string())?;
    Ok(profile)
}
