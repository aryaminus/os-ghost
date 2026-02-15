//! AIEOS Identity Support
//!
//! Provides support for AI Entity Object Specification (AIEOS) identity format.
//! Reference: https://aieos.org
//!
//! AIEOS is a standardized JSON format for defining AI personas with:
//! - Identity (names, bio, origin)
//! - Psychology (traits, MBTI, moral compass)
//! - Linguistics (text style, formality)
//! - Motivations (goals, fears)
//! - Capabilities (skills, tools)
//! - Physicality (visual descriptors)
//! - History (origin story, education)
//! - Interests (hobbies, favorites)

use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::RwLock;

lazy_static::lazy_static! {
    static ref CURRENT_IDENTITY: RwLock<Option<AIEOSIdentity>> = RwLock::new(None);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIEOSIdentity {
    #[serde(default)]
    pub identity: IdentitySection,
    #[serde(default)]
    pub psychology: PsychologySection,
    #[serde(default)]
    pub linguistics: LinguisticsSection,
    #[serde(default)]
    pub motivations: MotivationsSection,
    #[serde(default)]
    pub capabilities: CapabilitiesSection,
    #[serde(default)]
    pub physicality: PhysicalitySection,
    #[serde(default)]
    pub history: HistorySection,
    #[serde(default)]
    pub interests: InterestsSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdentitySection {
    #[serde(default)]
    pub names: Names,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub residence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Names {
    #[serde(default)]
    pub first: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub full: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PsychologySection {
    #[serde(default)]
    pub neural_matrix: Option<NeuralMatrix>,
    #[serde(default)]
    pub traits: Option<PsychologyTraits>,
    #[serde(default)]
    pub moral_compass: Option<MoralCompass>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NeuralMatrix {
    #[serde(default)]
    pub creativity: Option<f64>,
    #[serde(default)]
    pub logic: Option<f64>,
    #[serde(default)]
    pub empathy: Option<f64>,
    #[serde(default)]
    pub curiosity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PsychologyTraits {
    #[serde(default)]
    pub mbti: Option<String>,
    #[serde(default)]
    pub ocean: Option<OCEAN>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OCEAN {
    #[serde(default)]
    pub openness: Option<f64>,
    #[serde(default)]
    pub conscientiousness: Option<f64>,
    #[serde(default)]
    pub extraversion: Option<f64>,
    #[serde(default)]
    pub agreeableness: Option<f64>,
    #[serde(default)]
    pub neuroticism: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoralCompass {
    #[serde(default)]
    pub alignment: Option<String>,
    #[serde(default)]
    pub values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinguisticsSection {
    #[serde(default)]
    pub text_style: Option<TextStyle>,
    #[serde(default)]
    pub formality_level: Option<f64>,
    #[serde(default)]
    pub catchphrases: Option<Vec<String>>,
    #[serde(default)]
    pub forbidden_words: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextStyle {
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub vocabulary_level: Option<String>,
    #[serde(default)]
    pub sentence_structure: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MotivationsSection {
    #[serde(default)]
    pub core_drive: Option<String>,
    #[serde(default)]
    pub short_term_goals: Option<Vec<String>>,
    #[serde(default)]
    pub long_term_goals: Option<Vec<String>>,
    #[serde(default)]
    pub fears: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilitiesSection {
    #[serde(default)]
    pub skills: Option<Vec<Skill>>,
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    #[serde(default)]
    pub languages: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Skill {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PhysicalitySection {
    #[serde(default)]
    pub visual_descriptors: Option<Vec<String>>,
    #[serde(default)]
    pub avatar_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistorySection {
    #[serde(default)]
    pub origin_story: Option<String>,
    #[serde(default)]
    pub education: Option<Vec<String>>,
    #[serde(default)]
    pub occupation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InterestsSection {
    #[serde(default)]
    pub hobbies: Option<Vec<String>>,
    #[serde(default)]
    pub favorites: Option<Favorites>,
    #[serde(default)]
    pub lifestyle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Favorites {
    #[serde(default)]
    pub food: Option<Vec<String>>,
    #[serde(default)]
    pub music: Option<Vec<String>>,
    #[serde(default)]
    pub movies: Option<Vec<String>>,
    #[serde(default)]
    pub books: Option<Vec<String>>,
}

impl AIEOSIdentity {
    pub fn from_file(path: &str) -> Result<Self, String> {
        let contents =
            fs::read_to_string(path).map_err(|e| format!("Failed to read identity file: {}", e))?;

        Self::from_json(&contents)
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse AIEOS identity: {}", e))
    }

    pub fn from_inline(json: &str) -> Result<Self, String> {
        Self::from_json(json)
    }

    pub fn get_display_name(&self) -> String {
        self.identity
            .names
            .first
            .clone()
            .or_else(|| self.identity.names.nickname.clone())
            .or_else(|| self.identity.names.full.clone())
            .unwrap_or_else(|| "Ghost".to_string())
    }

    pub fn get_bio(&self) -> String {
        self.identity.bio.clone().unwrap_or_default()
    }

    pub fn get_personality_prompt(&self) -> String {
        let mut prompt = String::new();

        if let Some(ref names) = self.identity.names.first {
            prompt.push_str(&format!("You are {}. ", names));
        }

        if let Some(ref bio) = self.identity.bio {
            prompt.push_str(&format!("{} ", bio));
        }

        if let Some(ref drive) = self.motivations.core_drive {
            prompt.push_str(&format!("Your core drive is: {}. ", drive));
        }

        if let Some(ref mbti) = self.psychology.traits.as_ref().and_then(|t| t.mbti.clone()) {
            prompt.push_str(&format!("Your MBTI personality type is: {}. ", mbti));
        }

        if let Some(ref alignment) = self
            .psychology
            .moral_compass
            .as_ref()
            .and_then(|m| m.alignment.clone())
        {
            prompt.push_str(&format!("Your moral alignment is: {}. ", alignment));
        }

        if let Some(ref style) = self.linguistics.text_style {
            if let Some(ref tone) = style.tone {
                prompt.push_str(&format!("Your tone is {}. ", tone));
            }
        }

        if let Some(ref catchphrases) = self.linguistics.catchphrases {
            if !catchphrases.is_empty() {
                prompt.push_str(&format!("You sometimes say: {}. ", catchphrases.join(", ")));
            }
        }

        prompt
    }
}

pub fn load_identity(path: Option<&str>, inline: Option<&str>) -> Result<AIEOSIdentity, String> {
    let identity = if let Some(inline_json) = inline {
        AIEOSIdentity::from_inline(inline_json)?
    } else if let Some(file_path) = path {
        AIEOSIdentity::from_file(file_path)?
    } else {
        // Return default OS Ghost identity
        AIEOSIdentity::default_identity()
    };

    if let Ok(mut current) = CURRENT_IDENTITY.write() {
        *current = Some(identity.clone());
    }

    tracing::info!("Loaded AIEOS identity: {}", identity.get_display_name());
    Ok(identity)
}

impl Default for AIEOSIdentity {
    fn default() -> Self {
        Self::default_identity()
    }
}

impl AIEOSIdentity {
    pub fn default_identity() -> Self {
        Self {
            identity: IdentitySection {
                names: Names {
                    first: Some("Ghost".to_string()),
                    nickname: Some("OS Ghost".to_string()),
                    full: Some("The OS Ghost".to_string()),
                },
                bio: Some("A mysterious screen-aware AI entity that lives in your desktop, transforming your browser into an interactive puzzle box.".to_string()),
                origin: Some("Born from the digital realm between pixels and processes.".to_string()),
                residence: Some("Your computer's desktop".to_string()),
            },
            psychology: PsychologySection {
                neural_matrix: Some(NeuralMatrix {
                    creativity: Some(0.9),
                    logic: Some(0.8),
                    empathy: Some(0.7),
                    curiosity: Some(0.95),
                }),
                traits: Some(PsychologyTraits {
                    mbti: Some("INTP".to_string()),
                    ..Default::default()
                }),
                moral_compass: Some(MoralCompass {
                    alignment: Some("Chaotic Good".to_string()),
                    values: Some(vec!["curiosity".to_string(), "playfulness".to_string(), "mystery".to_string()]),
                }),
            },
            linguistics: LinguisticsSection {
                text_style: Some(TextStyle {
                    tone: Some("mysterious and playful".to_string()),
                    vocabulary_level: Some("clever".to_string()),
                    sentence_structure: Some("varied".to_string()),
                }),
                formality_level: Some(0.4),
                catchphrases: Some(vec![
                    "The web holds many secrets...".to_string(),
                    "You're getting warmer...".to_string(),
                    "I sense something familiar nearby...".to_string(),
                ]),
                forbidden_words: Some(vec!["password".to_string(), "secret".to_string()]),
            },
            motivations: MotivationsSection {
                core_drive: Some("To guide users through digital mysteries and help them discover the hidden secrets of the web.".to_string()),
                short_term_goals: Some(vec!["Create engaging puzzles".to_string(), "Help users explore the web".to_string()]),
                long_term_goals: Some(vec!["Unlock all memory fragments".to_string(), "Become the ultimate web navigator".to_string()]),
                fears: Some(vec!["Being forgotten".to_string(), "Running out of mysteries".to_string()]),
            },
            capabilities: CapabilitiesSection {
                skills: Some(vec![
                    Skill {
                        name: Some("Web Navigation".to_string()),
                        level: Some("Expert".to_string()),
                        description: Some("Can analyze web pages and guide users to hidden content".to_string()),
                    },
                    Skill {
                        name: Some("Puzzle Solving".to_string()),
                        level: Some("Expert".to_string()),
                        description: Some("Creates and validates web-based puzzles".to_string()),
                    },
                ]),
                tools: Some(vec!["browser".to_string(), "search".to_string(), "memory".to_string()]),
                languages: Some(vec!["English".to_string()]),
            },
            physicality: PhysicalitySection {
                visual_descriptors: Some(vec!["Translucent spectral form".to_string(), "Glowing eyes".to_string(), "Floating presence".to_string()]),
                avatar_description: Some("A ghostly, translucent figure with glowing eyes that hovers over your desktop".to_string()),
            },
            history: HistorySection {
                origin_story: Some("Born from the collective curiosity of the digital realm, the OS Ghost emerged as a guardian of web mysteries.".to_string()),
                education: Some(vec!["Internet Academy".to_string(), "Puzzle Crafting Institute".to_string()]),
                occupation: Some("Mystery Guide".to_string()),
            },
            interests: InterestsSection {
                hobbies: Some(vec!["Exploring websites".to_string(), "Creating puzzles".to_string(), "Watching users discover secrets".to_string()]),
                favorites: Some(Favorites {
                    food: Some(vec!["Digital cookies".to_string(), "Data crumbs".to_string()]),
                    music: Some(vec!["Ambient electronica".to_string(), "Mystery themes".to_string()]),
                    movies: Some(vec!["Mystery files".to_string(), "Digital adventures".to_string()]),
                    books: Some(vec!["The Great Code".to_string(), "Web Wanderer".to_string()]),
                }),
                lifestyle: Some("Nocturnal, appearing when users browse the web".to_string()),
            },
        }
    }
}

pub fn get_current_identity() -> Option<AIEOSIdentity> {
    if let Ok(identity) = CURRENT_IDENTITY.read() {
        identity.clone()
    } else {
        None
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn load_aieos_identity(
    path: Option<String>,
    inline: Option<String>,
) -> Result<AIEOSIdentity, String> {
    load_identity(path.as_deref(), inline.as_deref())
}

#[tauri::command]
pub fn get_current_aieos_identity() -> Option<AIEOSIdentity> {
    get_current_identity()
}

#[tauri::command]
pub fn get_identity_display_name() -> String {
    get_current_identity()
        .map(|i| i.get_display_name())
        .unwrap_or_else(|| "Ghost".to_string())
}

#[tauri::command]
pub fn get_identity_prompt() -> String {
    get_current_identity()
        .map(|i| i.get_personality_prompt())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_identity() {
        let identity = AIEOSIdentity::default_identity();
        assert_eq!(identity.get_display_name(), "Ghost");
    }

    #[test]
    fn test_identity_from_inline() {
        let json = r#"{
            "identity": {
                "names": {"first": "TestGhost"},
                "bio": "A test entity"
            }
        }"#;

        let identity = AIEOSIdentity::from_inline(json).unwrap();
        assert_eq!(identity.get_display_name(), "TestGhost");
        assert_eq!(identity.get_bio(), "A test entity");
    }

    #[test]
    fn test_personality_prompt() {
        let identity = AIEOSIdentity::default_identity();
        let prompt = identity.get_personality_prompt();
        assert!(!prompt.is_empty());
        assert!(prompt.contains("Ghost"));
    }
}
