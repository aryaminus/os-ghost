//! AI module - AI providers and clients

pub mod ai_provider;
pub mod gemini_client;
pub mod ollama_client;
pub mod providers;
pub mod vision;

// Re-export commonly used types
pub use ai_provider::{ProviderType, SmartAiRouter};
pub use gemini_client::GeminiClient;
pub use ollama_client::OllamaClient;
pub use providers::{
    get_provider, list_providers, register_provider, CompletionOptions, Provider, ProviderError,
    ProviderFactory, ProviderInfo, ProviderKind,
};
pub use vision::{ElementType, VisionAnalysis, VisionAnalyzer, VisionProvider, VisualElement};
