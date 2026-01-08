//! AI Provider abstraction and intelligent routing
//! Provides a unified interface for Gemini (cloud) and Ollama (local) LLMs
//!
//! ## Routing Strategy
//!
//! The SmartAiRouter implements intelligent routing with these priorities:
//!
//! 1. **Cost Optimization**: When both providers are available, prefer Ollama for
//!    lightweight tasks (dialogue, similarity) to reduce API costs
//! 2. **Quality Optimization**: Prefer Gemini for complex tasks (puzzle generation,
//!    image analysis) that benefit from more powerful models
//! 3. **Availability**: Automatic fallback when primary provider fails
//! 4. **Circuit Breaker**: Track failures with time-based recovery to prevent
//!    hammering failing services

use crate::gemini_client::{
    ActivityContext, AdaptivePuzzle, DynamicPuzzle, GeminiClient, VerificationResult,
};
use crate::ollama_client::OllamaClient;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Provider type for logging and status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    Gemini,
    Ollama,
    None,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Gemini => write!(f, "Gemini"),
            ProviderType::Ollama => write!(f, "Ollama"),
            ProviderType::None => write!(f, "None"),
        }
    }
}

/// Task complexity for routing decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Medium and Heavy are intentionally reserved for future routing
enum TaskComplexity {
    /// Simple tasks - dialogue, similarity (prefer local when both available)
    Light,
    /// Medium tasks - text generation, verification (prefer cloud)
    Medium,
    /// Complex tasks - puzzle generation, image analysis (prefer cloud)
    Heavy,
}

/// Circuit breaker recovery time (30 seconds)
const CIRCUIT_BREAKER_RECOVERY_SECS: u64 = 30;

/// Smart AI Router that intelligently routes requests between providers
///
/// ## Routing Strategy
///
/// | Task Type        | Primary   | Fallback  |
/// |------------------|-----------|-----------|
/// | Dialogue         | Ollama*   | Gemini    |
/// | URL Similarity   | Ollama*   | Gemini    |
/// | Image Analysis   | Gemini    | Ollama    |
/// | Puzzle Gen       | Gemini    | Ollama    |
/// | Verification     | Gemini    | Ollama    |
///
/// *When both available; otherwise uses what's available
pub struct SmartAiRouter {
    gemini: Option<Arc<GeminiClient>>,
    ollama: Arc<OllamaClient>,
    /// Track if Gemini is currently experiencing issues (circuit breaker)
    gemini_failing: AtomicBool,
    /// Timestamp when Gemini started failing (for recovery)
    gemini_fail_time: AtomicU64,
    /// Track if Ollama was confirmed available
    ollama_available: AtomicBool,
    /// Timestamp of last Ollama check
    ollama_last_check: AtomicU64,
}

impl SmartAiRouter {
    /// Create a new router with optional Gemini client and Ollama client
    pub fn new(gemini: Option<Arc<GeminiClient>>, ollama: Arc<OllamaClient>) -> Self {
        Self {
            gemini,
            ollama,
            gemini_failing: AtomicBool::new(false),
            gemini_fail_time: AtomicU64::new(0),
            ollama_available: AtomicBool::new(false),
            ollama_last_check: AtomicU64::new(0),
        }
    }

    /// Get current timestamp in seconds
    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Initialize and check provider availability
    pub async fn initialize(&self) {
        // Check Ollama availability
        let ollama_ok = self.ollama.is_available().await;
        self.ollama_available.store(ollama_ok, Ordering::SeqCst);
        self.ollama_last_check.store(Self::now_secs(), Ordering::SeqCst);

        if ollama_ok {
            tracing::info!("Ollama server detected and available");

            // Check for vision model
            if self.ollama.has_vision_model().await {
                tracing::info!("Ollama vision model available");
            } else {
                tracing::warn!(
                    "Ollama running but vision model not found. Run: ollama pull llama3.2-vision"
                );
            }
        } else {
            tracing::debug!("Ollama server not detected");
        }

        // Log Gemini status
        if self.gemini.is_some() {
            tracing::info!("Gemini API key configured - will be preferred for complex tasks");
        } else {
            tracing::info!("No Gemini API key - will use Ollama exclusively");
        }
    }

    /// Get the current active provider type (for display/status)
    pub fn active_provider(&self) -> ProviderType {
        if self.gemini.is_some() && !self.is_gemini_circuit_open() {
            ProviderType::Gemini
        } else if self.ollama_available.load(Ordering::SeqCst) {
            ProviderType::Ollama
        } else if self.gemini.is_some() {
            ProviderType::Gemini // Fall back to Gemini even if circuit open
        } else {
            ProviderType::None
        }
    }

    /// Check if any AI provider is available
    pub fn is_available(&self) -> bool {
        self.gemini.is_some() || self.ollama_available.load(Ordering::SeqCst)
    }

    /// Check if Ollama is available
    pub fn has_ollama(&self) -> bool {
        self.ollama_available.load(Ordering::SeqCst)
    }

    /// Check if Gemini is configured
    pub fn has_gemini(&self) -> bool {
        self.gemini.is_some()
    }

    /// Refresh Ollama availability (call periodically if needed)
    pub async fn refresh_ollama_status(&self) {
        let available = self.ollama.is_available().await;
        self.ollama_available.store(available, Ordering::SeqCst);
        self.ollama_last_check.store(Self::now_secs(), Ordering::SeqCst);
    }

    /// Check if Gemini circuit breaker is open (should skip Gemini)
    fn is_gemini_circuit_open(&self) -> bool {
        if !self.gemini_failing.load(Ordering::SeqCst) {
            return false;
        }

        // Check if enough time has passed to try again
        let fail_time = self.gemini_fail_time.load(Ordering::SeqCst);
        let now = Self::now_secs();

        if now - fail_time > CIRCUIT_BREAKER_RECOVERY_SECS {
            // Reset circuit breaker - allow retry
            self.gemini_failing.store(false, Ordering::SeqCst);
            tracing::info!("Gemini circuit breaker reset - retrying");
            false
        } else {
            true
        }
    }

    /// Mark Gemini as recovering (after successful request)
    fn mark_gemini_ok(&self) {
        self.gemini_failing.store(false, Ordering::SeqCst);
    }

    /// Mark Gemini as failing (after error)
    fn mark_gemini_failing(&self) {
        self.gemini_failing.store(true, Ordering::SeqCst);
        self.gemini_fail_time.store(Self::now_secs(), Ordering::SeqCst);
    }

    /// Determine which provider to use for a given task complexity
    /// Returns (primary, has_fallback)
    fn choose_provider(&self, complexity: TaskComplexity) -> (ProviderType, bool) {
        let has_gemini = self.gemini.is_some() && !self.is_gemini_circuit_open();
        let has_ollama = self.ollama_available.load(Ordering::SeqCst);

        match (has_gemini, has_ollama, complexity) {
            // Both available - route based on task complexity
            (true, true, TaskComplexity::Light) => (ProviderType::Ollama, true),
            (true, true, TaskComplexity::Medium) => (ProviderType::Gemini, true),
            (true, true, TaskComplexity::Heavy) => (ProviderType::Gemini, true),

            // Only one available
            (true, false, _) => (ProviderType::Gemini, false),
            (false, true, _) => (ProviderType::Ollama, false),

            // Neither available
            (false, false, _) => (ProviderType::None, false),
        }
    }

    // ========================================================================
    // AI Methods with Fallback Logic
    // ========================================================================

    /// Analyze an image with AI vision
    pub async fn analyze_image(&self, base64_image: &str, prompt: &str) -> Result<String> {
        // Try Gemini first if available and not failing
        if let Some(ref gemini) = self.gemini {
            if !self.gemini_failing.load(Ordering::SeqCst) {
                match gemini.analyze_image(base64_image, prompt).await {
                    Ok(result) => {
                        self.mark_gemini_ok();
                        return Ok(result);
                    }
                    Err(e) => {
                        tracing::warn!("Gemini analyze_image failed, trying Ollama: {}", e);
                        self.mark_gemini_failing();
                    }
                }
            }
        }

        // Fallback to Ollama
        if self.ollama_available.load(Ordering::SeqCst) {
            return self.ollama.analyze_image(base64_image, prompt).await;
        }

        // Last resort: try Gemini even if marked as failing
        if let Some(ref gemini) = self.gemini {
            return gemini.analyze_image(base64_image, prompt).await;
        }

        Err(anyhow::anyhow!("No AI provider available"))
    }

    /// Generate text from a prompt
    pub async fn generate_text(&self, prompt: &str) -> Result<String> {
        // Try Gemini first
        if let Some(ref gemini) = self.gemini {
            if !self.gemini_failing.load(Ordering::SeqCst) {
                match gemini.generate_text(prompt).await {
                    Ok(result) => {
                        self.mark_gemini_ok();
                        return Ok(result);
                    }
                    Err(e) => {
                        tracing::warn!("Gemini generate_text failed, trying Ollama: {}", e);
                        self.mark_gemini_failing();
                    }
                }
            }
        }

        // Fallback to Ollama
        if self.ollama_available.load(Ordering::SeqCst) {
            return self.ollama.generate_text(prompt).await;
        }

        // Last resort
        if let Some(ref gemini) = self.gemini {
            return gemini.generate_text(prompt).await;
        }

        Err(anyhow::anyhow!("No AI provider available"))
    }

    /// Calculate URL similarity
    /// Light task - prefers Ollama when both available (cost optimization)
    pub async fn calculate_url_similarity(&self, url1: &str, url2: &str) -> Result<f32> {
        let (primary, has_fallback) = self.choose_provider(TaskComplexity::Light);

        match primary {
            ProviderType::Ollama => {
                match self.ollama.calculate_url_similarity(url1, url2).await {
                    Ok(result) => return Ok(result),
                    Err(e) if has_fallback => {
                        tracing::warn!("Ollama similarity failed, trying Gemini: {}", e);
                    }
                    Err(e) => return Err(e),
                }

                // Fallback to Gemini
                if let Some(ref gemini) = self.gemini {
                    return gemini.calculate_url_similarity(url1, url2).await;
                }
            }
            ProviderType::Gemini => {
                if let Some(ref gemini) = self.gemini {
                    match gemini.calculate_url_similarity(url1, url2).await {
                        Ok(result) => {
                            self.mark_gemini_ok();
                            return Ok(result);
                        }
                        Err(e) if has_fallback => {
                            tracing::warn!("Gemini similarity failed, trying Ollama: {}", e);
                            self.mark_gemini_failing();
                        }
                        Err(e) => {
                            self.mark_gemini_failing();
                            return Err(e);
                        }
                    }
                }

                // Fallback to Ollama
                if self.ollama_available.load(Ordering::SeqCst) {
                    return self.ollama.calculate_url_similarity(url1, url2).await;
                }
            }
            ProviderType::None => {}
        }

        Ok(0.0) // Return no similarity if no provider
    }

    /// Generate dialogue
    /// Light task - prefers Ollama when both available (cost optimization)
    pub async fn generate_dialogue(&self, context: &str, personality: &str) -> Result<String> {
        let (primary, has_fallback) = self.choose_provider(TaskComplexity::Light);

        match primary {
            ProviderType::Ollama => {
                match self.ollama.generate_dialogue(context, personality).await {
                    Ok(result) => return Ok(result),
                    Err(e) if has_fallback => {
                        tracing::warn!("Ollama dialogue failed, trying Gemini: {}", e);
                    }
                    Err(e) => return Err(e),
                }

                // Fallback to Gemini
                if let Some(ref gemini) = self.gemini {
                    return gemini.generate_dialogue(context, personality).await;
                }
            }
            ProviderType::Gemini => {
                if let Some(ref gemini) = self.gemini {
                    match gemini.generate_dialogue(context, personality).await {
                        Ok(result) => {
                            self.mark_gemini_ok();
                            return Ok(result);
                        }
                        Err(e) if has_fallback => {
                            tracing::warn!("Gemini dialogue failed, trying Ollama: {}", e);
                            self.mark_gemini_failing();
                        }
                        Err(e) => {
                            self.mark_gemini_failing();
                            return Err(e);
                        }
                    }
                }

                // Fallback to Ollama
                if self.ollama_available.load(Ordering::SeqCst) {
                    return self.ollama.generate_dialogue(context, personality).await;
                }
            }
            ProviderType::None => {}
        }

        Ok("...".to_string()) // Silent fallback
    }

    /// Generate dynamic puzzle
    pub async fn generate_dynamic_puzzle(
        &self,
        url: &str,
        page_title: &str,
        page_content: &str,
        history_context: &str,
    ) -> Result<DynamicPuzzle> {
        // Try Gemini first (has Google Search grounding)
        if let Some(ref gemini) = self.gemini {
            if !self.gemini_failing.load(Ordering::SeqCst) {
                match gemini
                    .generate_dynamic_puzzle(url, page_title, page_content, history_context)
                    .await
                {
                    Ok(result) => {
                        self.mark_gemini_ok();
                        return Ok(result);
                    }
                    Err(e) => {
                        tracing::warn!("Gemini puzzle generation failed, trying Ollama: {}", e);
                        self.mark_gemini_failing();
                    }
                }
            }
        }

        // Fallback to Ollama
        if self.ollama_available.load(Ordering::SeqCst) {
            return self
                .ollama
                .generate_dynamic_puzzle(url, page_title, page_content, history_context)
                .await;
        }

        // Last resort
        if let Some(ref gemini) = self.gemini {
            return gemini
                .generate_dynamic_puzzle(url, page_title, page_content, history_context)
                .await;
        }

        Err(anyhow::anyhow!(
            "No AI provider available for puzzle generation"
        ))
    }

    /// Verify screenshot clue
    pub async fn verify_screenshot_clue(
        &self,
        base64_image: &str,
        clue_description: &str,
    ) -> Result<VerificationResult> {
        // Try Gemini first
        if let Some(ref gemini) = self.gemini {
            if !self.gemini_failing.load(Ordering::SeqCst) {
                match gemini
                    .verify_screenshot_clue(base64_image, clue_description)
                    .await
                {
                    Ok(result) => {
                        self.mark_gemini_ok();
                        return Ok(result);
                    }
                    Err(e) => {
                        tracing::warn!("Gemini verification failed, trying Ollama: {}", e);
                        self.mark_gemini_failing();
                    }
                }
            }
        }

        // Fallback to Ollama
        if self.ollama_available.load(Ordering::SeqCst) {
            return self
                .ollama
                .verify_screenshot_clue(base64_image, clue_description)
                .await;
        }

        // Last resort
        if let Some(ref gemini) = self.gemini {
            return gemini
                .verify_screenshot_clue(base64_image, clue_description)
                .await;
        }

        Err(anyhow::anyhow!("No AI provider available for verification"))
    }

    /// Generate adaptive puzzle based on activity
    pub async fn generate_adaptive_puzzle(
        &self,
        activities: &[ActivityContext],
        current_app: Option<&str>,
        current_content: Option<&str>,
    ) -> Result<AdaptivePuzzle> {
        // Try Gemini first
        if let Some(ref gemini) = self.gemini {
            if !self.gemini_failing.load(Ordering::SeqCst) {
                match gemini
                    .generate_adaptive_puzzle(activities, current_app, current_content)
                    .await
                {
                    Ok(result) => {
                        self.mark_gemini_ok();
                        return Ok(result);
                    }
                    Err(e) => {
                        tracing::warn!("Gemini adaptive puzzle failed, trying Ollama: {}", e);
                        self.mark_gemini_failing();
                    }
                }
            }
        }

        // Fallback to Ollama
        if self.ollama_available.load(Ordering::SeqCst) {
            return self
                .ollama
                .generate_adaptive_puzzle(activities, current_app, current_content)
                .await;
        }

        // Last resort
        if let Some(ref gemini) = self.gemini {
            return gemini
                .generate_adaptive_puzzle(activities, current_app, current_content)
                .await;
        }

        Err(anyhow::anyhow!(
            "No AI provider available for adaptive puzzle generation"
        ))
    }

    /// Generate contextual dialogue
    pub async fn generate_contextual_dialogue(
        &self,
        recent_activities: &[ActivityContext],
        current_context: &str,
        ghost_mood: &str,
    ) -> Result<String> {
        // Try Gemini first
        if let Some(ref gemini) = self.gemini {
            if !self.gemini_failing.load(Ordering::SeqCst) {
                match gemini
                    .generate_contextual_dialogue(recent_activities, current_context, ghost_mood)
                    .await
                {
                    Ok(result) => {
                        self.mark_gemini_ok();
                        return Ok(result);
                    }
                    Err(e) => {
                        tracing::warn!("Gemini contextual dialogue failed, trying Ollama: {}", e);
                        self.mark_gemini_failing();
                    }
                }
            }
        }

        // Fallback to Ollama with activity context
        if self.ollama_available.load(Ordering::SeqCst) {
            return self
                .ollama
                .generate_contextual_dialogue(recent_activities, current_context, ghost_mood)
                .await;
        }

        // Last resort
        if let Some(ref gemini) = self.gemini {
            return gemini
                .generate_contextual_dialogue(recent_activities, current_context, ghost_mood)
                .await;
        }

        Ok("...".to_string())
    }
}
