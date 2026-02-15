//! Vision analysis for screenshot understanding
//! Routes between Gemini Vision API and local VLM (Ollama)
//!
//! Provides element detection, interaction analysis, and visual verification
//! for browser automation and puzzle solving.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Types of UI elements that can be detected
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    Button,
    Input,
    Link,
    Text,
    Image,
    Dropdown,
    Checkbox,
    Radio,
    TextArea,
    Label,
    Navigation,
    Search,
    Submit,
    Other,
}

impl std::fmt::Display for ElementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElementType::Button => write!(f, "button"),
            ElementType::Input => write!(f, "input field"),
            ElementType::Link => write!(f, "link"),
            ElementType::Text => write!(f, "text"),
            ElementType::Image => write!(f, "image"),
            ElementType::Dropdown => write!(f, "dropdown"),
            ElementType::Checkbox => write!(f, "checkbox"),
            ElementType::Radio => write!(f, "radio button"),
            ElementType::TextArea => write!(f, "text area"),
            ElementType::Label => write!(f, "label"),
            ElementType::Navigation => write!(f, "navigation"),
            ElementType::Search => write!(f, "search"),
            ElementType::Submit => write!(f, "submit button"),
            ElementType::Other => write!(f, "element"),
        }
    }
}

/// Normalized coordinates (0.0 - 1.0 relative to screenshot dimensions)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct NormalizedCoords {
    pub x: f32, // 0.0 = left, 1.0 = right
    pub y: f32, // 0.0 = top, 1.0 = bottom
}

impl NormalizedCoords {
    /// Convert to screen coordinates
    pub fn to_screen(&self, screen_width: u32, screen_height: u32) -> (u32, u32) {
        (
            (self.x * screen_width as f32) as u32,
            (self.y * screen_height as f32) as u32,
        )
    }

    /// Check if coordinates are within bounds
    pub fn is_valid(&self) -> bool {
        self.x >= 0.0 && self.x <= 1.0 && self.y >= 0.0 && self.y <= 1.0
    }
}

/// A detected UI element with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualElement {
    /// Type of element
    pub element_type: ElementType,
    /// Human-readable description (e.g., "Search button", "Email input")
    pub description: String,
    /// Normalized coordinates (center of element)
    pub coordinates: NormalizedCoords,
    /// Visible text content (if any)
    pub text_content: Option<String>,
    /// Detection confidence (0.0 - 1.0)
    pub confidence: f32,
    /// Whether element appears interactive
    pub is_interactive: bool,
    /// Element attributes (placeholder, aria-label, etc.)
    pub attributes: std::collections::HashMap<String, String>,
}

/// Result of screenshot analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionAnalysis {
    /// All detected elements
    pub elements: Vec<VisualElement>,
    /// Page state description
    pub page_description: String,
    /// Timestamp of analysis
    pub timestamp: u64,
    /// Provider used for analysis
    pub provider: VisionProvider,
}

/// Vision provider types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisionProvider {
    Gemini,
    Ollama,
}

impl std::fmt::Display for VisionProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VisionProvider::Gemini => write!(f, "Gemini Vision"),
            VisionProvider::Ollama => write!(f, "Ollama VLM"),
        }
    }
}

/// Vision analyzer with provider fallback
pub struct VisionAnalyzer {
    gemini_client: Option<Arc<crate::ai::gemini_client::GeminiClient>>,
    ollama_client: Option<Arc<crate::ai::ollama_client::OllamaClient>>,
    /// Track if Gemini is currently experiencing issues
    gemini_failing: AtomicBool,
    /// Timestamp when Gemini started failing
    gemini_fail_time: AtomicU64,
    /// Cache recent analyses to avoid repeated calls
    analysis_cache: std::sync::Mutex<std::collections::HashMap<String, VisionAnalysis>>,
}

/// Cache entry expiration (5 minutes)
const CACHE_EXPIRY_SECS: u64 = 300;

impl VisionAnalyzer {
    /// Create a new vision analyzer with optional providers
    pub fn new(
        gemini: Option<Arc<crate::ai::gemini_client::GeminiClient>>,
        ollama: Option<Arc<crate::ai::ollama_client::OllamaClient>>,
    ) -> Self {
        Self {
            gemini_client: gemini,
            ollama_client: ollama,
            gemini_failing: AtomicBool::new(false),
            gemini_fail_time: AtomicU64::new(0),
            analysis_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Analyze screenshot and return detected elements
    /// Uses Gemini if available, falls back to Ollama
    pub async fn analyze_screenshot(&self, image_bytes: &[u8]) -> Result<VisionAnalysis> {
        // Check cache first
        let cache_key = self.compute_image_hash(image_bytes);
        {
            let cache = self.analysis_cache.lock().unwrap();
            if let Some(cached) = cache.get(&cache_key) {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if now - cached.timestamp < CACHE_EXPIRY_SECS {
                    tracing::debug!("Using cached vision analysis");
                    return Ok(cached.clone());
                }
            }
        }

        // Try Gemini first if available and not failing
        if let Some(gemini) = &self.gemini_client {
            if !self.gemini_failing.load(Ordering::Relaxed) {
                match self.analyze_with_gemini(gemini, image_bytes).await {
                    Ok(analysis) => {
                        self.cache_analysis(&cache_key, &analysis);
                        return Ok(analysis);
                    }
                    Err(e) => {
                        tracing::warn!("Gemini vision failed, marking as failing: {}", e);
                        self.gemini_failing.store(true, Ordering::Relaxed);
                        self.gemini_fail_time.store(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                            Ordering::Relaxed,
                        );
                    }
                }
            }
        }

        // Fallback to Ollama
        if let Some(ollama) = &self.ollama_client {
            match self.analyze_with_ollama(ollama, image_bytes).await {
                Ok(analysis) => {
                    self.cache_analysis(&cache_key, &analysis);
                    return Ok(analysis);
                }
                Err(e) => {
                    tracing::error!("Ollama vision also failed: {}", e);
                }
            }
        }

        Err(anyhow!("No vision provider available"))
    }

    /// Find specific element by description
    pub async fn find_element(
        &self,
        image_bytes: &[u8],
        description: &str,
    ) -> Option<VisualElement> {
        match self.analyze_screenshot(image_bytes).await {
            Ok(analysis) => {
                // Find best matching element
                analysis
                    .elements
                    .into_iter()
                    .filter(|e| {
                        e.description
                            .to_lowercase()
                            .contains(&description.to_lowercase())
                            || e.text_content
                                .as_ref()
                                .map(|t| t.to_lowercase().contains(&description.to_lowercase()))
                                .unwrap_or(false)
                    })
                    .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            }
            Err(e) => {
                tracing::error!("Failed to find element: {}", e);
                None
            }
        }
    }

    /// Check if element is interactive (clickable, editable)
    pub fn is_interactive(element: &VisualElement) -> bool {
        matches!(
            element.element_type,
            ElementType::Button
                | ElementType::Input
                | ElementType::Link
                | ElementType::Dropdown
                | ElementType::Checkbox
                | ElementType::Radio
                | ElementType::TextArea
                | ElementType::Submit
                | ElementType::Search
        ) && element.is_interactive
    }

    /// Analyze with Gemini Vision API
    async fn analyze_with_gemini(
        &self,
        client: &Arc<crate::ai::gemini_client::GeminiClient>,
        image_bytes: &[u8],
    ) -> Result<VisionAnalysis> {
        // Convert image to base64
        let base64_image = base64_encode(image_bytes);

        // Build prompt for element detection
        let prompt = r#"Analyze this browser screenshot and identify all interactive UI elements.

Return a JSON object with:
1. "page_description": Brief description of the page (1-2 sentences)
2. "elements": Array of detected elements, each with:
   - "element_type": One of [button, input, link, text, image, dropdown, checkbox, radio, text_area, label, navigation, search, submit, other]
   - "description": Human-readable description (e.g., "Search button", "Email input field")
   - "coordinates": {"x": 0.0-1.0, "y": 0.0-1.0} (normalized center of element)
   - "text_content": Visible text (if any)
   - "confidence": 0.0-1.0 (detection confidence)
   - "is_interactive": true/false

Focus on elements users would want to click or interact with. Be precise with coordinates."#;

        // Call Gemini API
        let response = client
            .analyze_image(&base64_image, prompt)
            .await
            .map_err(|e| anyhow!("Gemini vision API error: {}", e))?;

        // Parse response
        self.parse_vision_response(&response, VisionProvider::Gemini)
    }

    /// Analyze with Ollama VLM
    async fn analyze_with_ollama(
        &self,
        client: &Arc<crate::ai::ollama_client::OllamaClient>,
        image_bytes: &[u8],
    ) -> Result<VisionAnalysis> {
        // Convert image to base64
        let base64_image = base64_encode(image_bytes);

        // Build prompt
        let prompt = r#"Analyze this screenshot and identify interactive UI elements.

Respond with JSON:
{
  "page_description": "brief page description",
  "elements": [
    {
      "element_type": "button|input|link|etc",
      "description": "human readable description",
      "coordinates": {"x": 0.0-1.0, "y": 0.0-1.0},
      "text_content": "visible text or null",
      "confidence": 0.0-1.0,
      "is_interactive": true|false
    }
  ]
}"#;

        // Call Ollama
        let response = client
            .analyze_image(&base64_image, prompt)
            .await
            .map_err(|e| anyhow!("Ollama vision error: {}", e))?;

        // Parse response
        self.parse_vision_response(&response, VisionProvider::Ollama)
    }

    /// Parse vision model response
    fn parse_vision_response(
        &self,
        response: &str,
        provider: VisionProvider,
    ) -> Result<VisionAnalysis> {
        // Extract JSON from response (models sometimes wrap in markdown)
        let json_str = self.extract_json(response)?;

        #[derive(Deserialize)]
        struct VisionResponse {
            page_description: String,
            elements: Vec<ElementResponse>,
        }

        #[derive(Deserialize)]
        struct ElementResponse {
            element_type: String,
            description: String,
            coordinates: CoordsResponse,
            #[serde(default)]
            text_content: Option<String>,
            #[serde(default)]
            confidence: f32,
            #[serde(default)]
            is_interactive: bool,
        }

        #[derive(Deserialize)]
        struct CoordsResponse {
            x: f32,
            y: f32,
        }

        let parsed: VisionResponse = serde_json::from_str(&json_str)
            .map_err(|e| anyhow!("Failed to parse vision response: {}", e))?;

        let elements: Vec<VisualElement> = parsed
            .elements
            .into_iter()
            .map(|e| VisualElement {
                element_type: self.parse_element_type(&e.element_type),
                description: e.description,
                coordinates: NormalizedCoords {
                    x: e.coordinates.x.clamp(0.0, 1.0),
                    y: e.coordinates.y.clamp(0.0, 1.0),
                },
                text_content: e.text_content,
                confidence: e.confidence.clamp(0.0, 1.0),
                is_interactive: e.is_interactive,
                attributes: std::collections::HashMap::new(),
            })
            .filter(|e| e.coordinates.is_valid())
            .collect();

        Ok(VisionAnalysis {
            elements,
            page_description: parsed.page_description,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            provider,
        })
    }

    /// Extract JSON from response text
    fn extract_json(&self, text: &str) -> Result<String> {
        // Try to find JSON in markdown code blocks
        if let Some(start) = text.find("```json") {
            if let Some(end) = text[start..].find("```") {
                return Ok(text[start + 7..start + end].trim().to_string());
            }
        }

        // Try to find JSON object directly
        if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                return Ok(text[start..=end].to_string());
            }
        }

        Err(anyhow!("Could not extract JSON from response"))
    }

    /// Parse element type string
    fn parse_element_type(&self, type_str: &str) -> ElementType {
        match type_str.to_lowercase().as_str() {
            "button" => ElementType::Button,
            "input" | "input_field" | "input field" => ElementType::Input,
            "link" | "a" => ElementType::Link,
            "text" => ElementType::Text,
            "image" | "img" => ElementType::Image,
            "dropdown" | "select" => ElementType::Dropdown,
            "checkbox" => ElementType::Checkbox,
            "radio" | "radio_button" | "radio button" => ElementType::Radio,
            "textarea" | "text_area" | "text area" => ElementType::TextArea,
            "label" => ElementType::Label,
            "navigation" | "nav" => ElementType::Navigation,
            "search" => ElementType::Search,
            "submit" | "submit_button" | "submit button" => ElementType::Submit,
            _ => ElementType::Other,
        }
    }

    /// Compute simple hash for image caching
    fn compute_image_hash(&self, image_bytes: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        image_bytes.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Cache analysis result
    fn cache_analysis(&self, key: &str, analysis: &VisionAnalysis) {
        let mut cache = self.analysis_cache.lock().unwrap();
        cache.insert(key.to_string(), analysis.clone());

        // Clean old entries if cache too large
        if cache.len() > 100 {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            cache.retain(|_, v| now - v.timestamp < CACHE_EXPIRY_SECS);
        }
    }

    /// Reset Gemini failure state (call periodically to retry)
    pub fn reset_gemini_status(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let fail_time = self.gemini_fail_time.load(Ordering::Relaxed);

        // Reset after 5 minutes
        if now - fail_time > 300 {
            self.gemini_failing.store(false, Ordering::Relaxed);
            tracing::info!("Resetting Gemini vision status");
        }
    }
}

/// Helper function for base64 encoding
fn base64_encode(input: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalized_coords() {
        let coords = NormalizedCoords { x: 0.5, y: 0.5 };
        assert_eq!(coords.to_screen(1920, 1080), (960, 540));
        assert!(coords.is_valid());

        let invalid = NormalizedCoords { x: 1.5, y: 0.5 };
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_element_type_display() {
        assert_eq!(ElementType::Button.to_string(), "button");
        assert_eq!(ElementType::Input.to_string(), "input field");
    }

    #[test]
    fn test_is_interactive() {
        let button = VisualElement {
            element_type: ElementType::Button,
            description: "Test".to_string(),
            coordinates: NormalizedCoords::default(),
            text_content: None,
            confidence: 0.9,
            is_interactive: true,
            attributes: std::collections::HashMap::new(),
        };
        assert!(VisionAnalyzer::is_interactive(&button));

        let text = VisualElement {
            element_type: ElementType::Text,
            description: "Test".to_string(),
            coordinates: NormalizedCoords::default(),
            text_content: None,
            confidence: 0.9,
            is_interactive: false,
            attributes: std::collections::HashMap::new(),
        };
        assert!(!VisionAnalyzer::is_interactive(&text));
    }
}
