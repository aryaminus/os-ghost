//! HermitClaw-style memory system with 3-factor retrieval and reflection hierarchy
//!
//! This module implements:
//! - Three-factor memory retrieval (recency + importance + relevance)
//! - Reflection hierarchy for emergent understanding
//! - Mood-based autonomous behavior
//! - Focus mode for task-locked operation

use crate::memory::store::MemoryStore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Memory entry with scoring metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub timestamp: u64,
    pub importance: u8, // 1-10, scored by LLM
    pub embedding: Option<Vec<f32>>,
    pub kind: MemoryKind,
    pub depth: u8,               // 0 = thought, 1+ = reflection depth
    pub references: Vec<String>, // IDs of source memories
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryKind {
    Thought,
    Reflection,
    Planning,
}

/// Mood system for autonomous behavior
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum Mood {
    #[default]
    Explorer,
    Research,
    DeepDive,
    Coder,
    Writer,
    Organizer,
}

/// Ghost operational mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum OperationalMode {
    #[default]
    Autonomous,
    Focus,
    Companion,
}

/// Personality genome for unique ghost identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityGenome {
    pub curiosity_domains: Vec<String>,
    pub thinking_styles: Vec<String>,
    pub temperament: String,
}

/// Configuration for memory retrieval
#[derive(Debug, Clone)]
pub struct RetrievalConfig {
    pub recency_decay_rate: f64,
    pub importance_weight: f64,
    pub relevance_weight: f64,
    pub reflection_threshold: u32,
    pub max_memories_retrieved: usize,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            recency_decay_rate: 0.995,
            importance_weight: 1.0,
            relevance_weight: 1.0,
            reflection_threshold: 50,
            max_memories_retrieved: 3,
        }
    }
}

/// Advanced memory system with 3-factor retrieval
#[allow(dead_code)]
pub struct AdvancedMemory {
    store: Arc<MemoryStore>,
    config: RetrievalConfig,
}

impl AdvancedMemory {
    pub fn new(store: Arc<MemoryStore>) -> Self {
        Self {
            store,
            config: RetrievalConfig::default(),
        }
    }

    /// Calculate recency score using exponential decay
    fn recency_score(&self, timestamp: u64, now: u64) -> f64 {
        let hours_ago = (now - timestamp) as f64 / 3600.0;
        (-(1.0 - self.config.recency_decay_rate) * hours_ago).exp()
    }

    /// Calculate importance score (normalized 0-1)
    fn importance_score(&self, importance: u8) -> f64 {
        importance as f64 / 10.0
    }

    /// Calculate relevance score using cosine similarity
    fn relevance_score(&self, embedding: &[f32], query_embedding: &[f32]) -> f64 {
        if embedding.is_empty() || query_embedding.is_empty() {
            return 0.5; // Default if no embeddings
        }

        let dot: f32 = embedding
            .iter()
            .zip(query_embedding.iter())
            .map(|(a, b)| a * b)
            .sum();
        let mag1: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag2: f32 = query_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();

        if mag1 == 0.0 || mag2 == 0.0 {
            return 0.5;
        }

        (dot / (mag1 * mag2)).max(0.0) as f64
    }

    /// Three-factor retrieval: recency + importance + relevance
    pub fn retrieve_memories(
        &self,
        _query: &str,
        _query_embedding: &[f32],
        _now: u64,
    ) -> Vec<MemoryEntry> {
        // This would read from the memory store in real implementation
        // For now, return empty - actual implementation would query sled
        Vec::new()
    }

    /// Calculate total score for a memory entry
    pub fn calculate_score(&self, memory: &MemoryEntry, query_embedding: &[f32], now: u64) -> f64 {
        let recency = self.recency_score(memory.timestamp, now);
        let importance = self.importance_score(memory.importance);
        let relevance = memory
            .embedding
            .as_ref()
            .map(|emb| self.relevance_score(emb, query_embedding))
            .unwrap_or(0.5);

        recency
            + importance * self.config.importance_weight
            + relevance * self.config.relevance_weight
    }

    /// Check if reflection should be triggered based on cumulative importance
    pub fn should_reflect(&self, recent_importance_sum: u32) -> bool {
        recent_importance_sum >= self.config.reflection_threshold
    }

    /// Create a reflection from recent memories
    pub fn create_reflection(
        &self,
        source_memories: &[MemoryEntry],
        current_depth: u8,
    ) -> MemoryEntry {
        let now = crate::core::utils::current_timestamp();

        // Synthesize insights from source memories
        let content = format!(
            "Reflection at depth {}: Synthesized from {} source memories. Key insights: {}",
            current_depth + 1,
            source_memories.len(),
            source_memories
                .iter()
                .map(|m| m.content.chars().take(50).collect::<String>())
                .collect::<Vec<_>>()
                .join("; ")
        );

        MemoryEntry {
            id: format!("ref_{}_{}", current_depth + 1, now),
            content,
            timestamp: now,
            importance: 8, // Reflections are typically important
            embedding: None,
            kind: MemoryKind::Reflection,
            depth: current_depth + 1,
            references: source_memories.iter().map(|m| m.id.clone()).collect(),
        }
    }

    /// Get available moods
    pub fn get_available_moods() -> Vec<Mood> {
        vec![
            Mood::Research,
            Mood::DeepDive,
            Mood::Coder,
            Mood::Writer,
            Mood::Explorer,
            Mood::Organizer,
        ]
    }

    /// Get mood description for prompts
    pub fn mood_to_description(mood: &Mood) -> &'static str {
        match mood {
            Mood::Research => "Research: Pick a topic, do web searches, write a report",
            Mood::DeepDive => "Deep-dive: Work on a project from your plans, make progress",
            Mood::Coder => "Coder: Write a script, a tool, or some code",
            Mood::Writer => "Writer: Compose something substantial - essay, analysis, report",
            Mood::Explorer => "Explorer: Search for something you know nothing about",
            Mood::Organizer => "Organizer: Update your projects, organize files, review work",
        }
    }

    /// Select random mood
    pub fn select_random_mood() -> Mood {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let idx = rng.gen_range(0..Self::get_available_moods().len());
        Self::get_available_moods()[idx].clone()
    }
}

/// Workflow export format (DroidClaw-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedWorkflow {
    pub name: String,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    #[serde(default)]
    pub app: Option<String>,
    pub goal: String,
    #[serde(default)]
    pub form_data: Option<serde_json::Value>,
}

impl ExportedWorkflow {
    /// Export workflow to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// File drop handler for processing dropped files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDrop {
    pub filename: String,
    pub path: String,
    pub content_type: String,
    pub timestamp: u64,
}

impl FileDrop {
    /// Check if file type is supported
    pub fn is_supported(&self) -> bool {
        matches!(
            self.content_type.as_str(),
            "text/plain"
                | "text/markdown"
                | "application/pdf"
                | "image/png"
                | "image/jpeg"
                | "image/gif"
        )
    }

    /// Supported extensions
    pub fn supported_extensions() -> Vec<&'static str> {
        vec![
            "txt", "md", "py", "json", "csv", "yaml", "toml", "js", "ts", "html", "css", "sh",
            "log", "pdf", "png", "jpg", "jpeg", "gif", "webp",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recency_decay() {
        let memory = AdvancedMemory::new(Arc::new(MemoryStore::new().unwrap()));
        let now = 3600; // 1 hour ago
        let score = memory.recency_score(0, now);
        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_importance_score() {
        let memory = AdvancedMemory::new(Arc::new(MemoryStore::new().unwrap()));
        assert_eq!(memory.importance_score(10), 1.0);
        assert_eq!(memory.importance_score(5), 0.5);
    }

    #[test]
    fn test_reflection_threshold() {
        let memory = AdvancedMemory::new(Arc::new(MemoryStore::new().unwrap()));
        assert!(!memory.should_reflect(30));
        assert!(memory.should_reflect(50));
    }
}
