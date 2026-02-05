//! AI module - AI providers and clients

pub mod ai_provider;
pub mod gemini_client;
pub mod ollama_client;

// Re-export commonly used types
pub use ai_provider::{SmartAiRouter, RoutingDecision};
pub use gemini_client::GeminiClient;
pub use ollama_client::OllamaClient;
