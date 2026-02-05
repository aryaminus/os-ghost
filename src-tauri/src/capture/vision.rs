//! Vision-based screenshot analysis and element detection
//!
//! Provides high-level interface for analyzing browser screenshots
//! and detecting interactive elements for automation.

use crate::ai::{VisionAnalysis, VisionAnalyzer, VisualElement};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Analyzed screenshot with element information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedScreenshot {
    /// Raw screenshot bytes
    pub image_bytes: Vec<u8>,
    /// Vision analysis results
    pub analysis: VisionAnalysis,
    /// Screenshot dimensions
    pub width: u32,
    pub height: u32,
    /// Timestamp when captured
    pub captured_at: u64,
}

/// Result of finding an element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementMatch {
    /// The matched element
    pub element: VisualElement,
    /// Match score (0.0 - 1.0)
    pub match_score: f32,
    /// Reason for match
    pub match_reason: String,
}

/// Vision capture manager
pub struct VisionCapture {
    analyzer: Option<Arc<VisionAnalyzer>>,
}

impl VisionCapture {
    /// Create a new vision capture manager
    pub fn new(analyzer: Option<Arc<VisionAnalyzer>>) -> Self {
        Self { analyzer }
    }

    /// Check if vision capabilities are available
    pub fn is_available(&self) -> bool {
        self.analyzer.is_some()
    }

    /// Capture and analyze a browser tab
    pub async fn capture_and_analyze(&self, image_bytes: Vec<u8>) -> Result<AnalyzedScreenshot> {
        let analyzer = self
            .analyzer
            .as_ref()
            .ok_or_else(|| anyhow!("Vision analyzer not available"))?;

        let analysis = analyzer.analyze_screenshot(&image_bytes).await?;
        let (width, height) = self.estimate_image_dimensions(&image_bytes);

        Ok(AnalyzedScreenshot {
            image_bytes,
            analysis,
            width,
            height,
            captured_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Find element by description
    pub fn find_element_by_description(
        &self,
        screenshot: &AnalyzedScreenshot,
        description: &str,
    ) -> Option<ElementMatch> {
        let description_lower = description.to_lowercase();

        screenshot
            .analysis
            .elements
            .iter()
            .filter(|e| {
                let desc_match = e.description.to_lowercase().contains(&description_lower);
                let text_match = e
                    .text_content
                    .as_ref()
                    .map(|t| t.to_lowercase().contains(&description_lower))
                    .unwrap_or(false);
                desc_match || text_match
            })
            .max_by(|a, b| {
                let score_a = self.calculate_match_score(a, &description_lower);
                let score_b = self.calculate_match_score(b, &description_lower);
                score_a.partial_cmp(&score_b).unwrap()
            })
            .map(|element| {
                let score = self.calculate_match_score(element, &description_lower);
                ElementMatch {
                    element: element.clone(),
                    match_score: score,
                    match_reason: format!("Description match: {}", element.description),
                }
            })
    }

    /// Find interactive elements
    pub fn find_interactive_elements<'a>(
        &self,
        screenshot: &'a AnalyzedScreenshot,
    ) -> Vec<&'a VisualElement> {
        screenshot
            .analysis
            .elements
            .iter()
            .filter(|e| VisionAnalyzer::is_interactive(e))
            .collect()
    }

    /// Get click coordinates for an element
    pub fn get_click_coordinates(
        &self,
        screenshot: &AnalyzedScreenshot,
        element: &VisualElement,
    ) -> (u32, u32) {
        element.coordinates.to_screen(screenshot.width, screenshot.height)
    }

    /// Helper: Calculate match score
    fn calculate_match_score(&self, element: &VisualElement, query: &str) -> f32 {
        let mut score = element.confidence;
        
        // Boost score if text content matches exactly
        if let Some(text) = &element.text_content {
            if text.to_lowercase() == query {
                score += 0.2;
            } else if text.to_lowercase().contains(query) {
                score += 0.1;
            }
        }

        // Prefer interactive elements
        if VisionAnalyzer::is_interactive(element) {
            score += 0.05;
        }

        score.min(1.0)
    }

    /// Helper: Estimate image dimensions from bytes
    fn estimate_image_dimensions(&self, _image_bytes: &[u8]) -> (u32, u32) {
        // In production, use image crate to read actual dimensions
        // For now, assume standard 1920x1080
        (1920, 1080)
    }
}
