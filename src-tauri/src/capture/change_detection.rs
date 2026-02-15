//! Screen change detection module for efficient screenshot capture
//! Compares pixel differences to avoid redundant captures

use anyhow::Result;
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for screen change detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDetectionConfig {
    /// Pixel difference threshold (0-255) - higher = less sensitive
    pub pixel_threshold: u8,
    /// Maximum number of changed pixels allowed (as percentage of total pixels)
    pub max_changed_percentage: f32,
    /// Minimum changed pixels to trigger capture (as percentage of total pixels)
    pub min_changed_percentage: f32,
}

impl Default for ChangeDetectionConfig {
    fn default() -> Self {
        Self {
            pixel_threshold: 30,
            max_changed_percentage: 0.95,
            min_changed_percentage: 0.01,
        }
    }
}

/// Change detection result
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeResult {
    /// No significant changes detected
    NoChange,
    /// Minor changes - may or may not capture depending on mode
    MinorChange(f32),
    /// Significant changes - should capture
    SignificantChange(f32),
    /// Complete screen change - should definitely capture
    ScreenSwitch(f32),
}

impl ChangeResult {
    /// Get the percentage of changed pixels
    pub fn changed_percentage(&self) -> f32 {
        match self {
            ChangeResult::NoChange => 0.0,
            ChangeResult::MinorChange(pct) => *pct,
            ChangeResult::SignificantChange(pct) => *pct,
            ChangeResult::ScreenSwitch(pct) => *pct,
        }
    }

    /// Check if we should capture based on the result
    pub fn should_capture(&self, config: &ChangeDetectionConfig) -> bool {
        match self {
            ChangeResult::NoChange => false,
            ChangeResult::MinorChange(pct) => *pct >= config.min_changed_percentage,
            ChangeResult::SignificantChange(_) => true,
            ChangeResult::ScreenSwitch(_) => true,
        }
    }
}

/// Screen change detector
pub struct ChangeDetector {
    config: ChangeDetectionConfig,
    last_image: Option<Arc<Vec<u8>>>,
    last_dimensions: Option<(u32, u32)>,
}

impl ChangeDetector {
    /// Create a new change detector with default config
    pub fn new() -> Self {
        Self {
            config: ChangeDetectionConfig::default(),
            last_image: None,
            last_dimensions: None,
        }
    }

    /// Create a new change detector with custom config
    pub fn with_config(config: ChangeDetectionConfig) -> Self {
        Self {
            config,
            last_image: None,
            last_dimensions: None,
        }
    }

    /// Get the current config
    pub fn config(&self) -> &ChangeDetectionConfig {
        &self.config
    }

    /// Update the config
    pub fn set_config(&mut self, config: ChangeDetectionConfig) {
        self.config = config;
    }

    /// Get primary screen dimensions
    fn get_primary_screen() -> Result<Screen> {
        let screens = Screen::all()?;
        screens
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No screens found"))
    }

    /// Capture screen and compare with last image (optimized for memory efficiency)
    pub fn capture_and_detect(&mut self) -> Result<(Vec<u8>, ChangeResult)> {
        let primary = Self::get_primary_screen()?;
        let image = primary.capture()?;
        let width = image.width();
        let height = image.height();

        let dimensions = (width, height);

        // Early dimension check - cheap comparison before pixel processing
        let change_result = if let Some(ref last_image) = self.last_image {
            if let Some(last_dims) = self.last_dimensions {
                if last_dims != dimensions {
                    ChangeResult::ScreenSwitch(1.0)
                } else {
                    // Only convert to buffer if we need to compare
                    let mut rgba_buffer = Vec::with_capacity((width * height * 4) as usize);
                    for y in 0..height {
                        for x in 0..width {
                            let pixel = image.get_pixel(x, y);
                            rgba_buffer.push(pixel[0]);
                            rgba_buffer.push(pixel[1]);
                            rgba_buffer.push(pixel[2]);
                            rgba_buffer.push(pixel[3]);
                        }
                    }
                    let result = self.detect_changes(&rgba_buffer, last_image, width, height);

                    // Only store if we need it for next comparison
                    if result != ChangeResult::NoChange {
                        self.last_image = Some(Arc::new(rgba_buffer));
                        self.last_dimensions = Some(dimensions);
                        return Ok((self.last_image.as_ref().unwrap().as_ref().clone(), result));
                    }

                    // No change - don't store, return empty buffer
                    return Ok((Vec::new(), result));
                }
            } else {
                ChangeResult::ScreenSwitch(1.0)
            }
        } else {
            ChangeResult::ScreenSwitch(1.0)
        };

        // First capture or dimension change - convert to buffer
        let mut rgba_buffer = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let pixel = image.get_pixel(x, y);
                rgba_buffer.push(pixel[0]);
                rgba_buffer.push(pixel[1]);
                rgba_buffer.push(pixel[2]);
                rgba_buffer.push(pixel[3]);
            }
        }

        self.last_image = Some(Arc::new(rgba_buffer));
        self.last_dimensions = Some(dimensions);

        Ok((
            self.last_image.as_ref().unwrap().as_ref().clone(),
            change_result,
        ))
    }

    /// Detect changes between two images
    fn detect_changes(
        &self,
        current: &[u8],
        previous: &[u8],
        width: u32,
        height: u32,
    ) -> ChangeResult {
        let total_pixels = (width * height) as usize;
        let mut changed_pixels = 0;

        let stride = 4;
        let sample_rate = if total_pixels > 2_000_000 { 4 } else { 2 };

        for i in (0..current.len() / stride).step_by(sample_rate) {
            let idx = i * stride;

            if idx + 3 >= current.len() || idx + 3 >= previous.len() {
                continue;
            }

            let r_diff = (current[idx] as i16 - previous[idx] as i16).abs();
            let g_diff = (current[idx + 1] as i16 - previous[idx + 1] as i16).abs();
            let b_diff = (current[idx + 2] as i16 - previous[idx + 2] as i16).abs();
            let a_diff = (current[idx + 3] as i16 - previous[idx + 3] as i16).abs();

            let max_diff = r_diff.max(g_diff).max(b_diff).max(a_diff);

            if max_diff > self.config.pixel_threshold as i16 {
                changed_pixels += 1;
            }
        }

        let adjusted_total_pixels = total_pixels / sample_rate;
        let changed_percentage = if adjusted_total_pixels > 0 {
            changed_pixels as f32 / adjusted_total_pixels as f32
        } else {
            0.0
        };

        if changed_percentage < self.config.min_changed_percentage {
            ChangeResult::NoChange
        } else if changed_percentage < 0.10 {
            ChangeResult::MinorChange(changed_percentage)
        } else if changed_percentage < self.config.max_changed_percentage {
            ChangeResult::SignificantChange(changed_percentage)
        } else {
            ChangeResult::ScreenSwitch(changed_percentage)
        }
    }

    /// Reset the detector (clear last image)
    pub fn reset(&mut self) {
        self.last_image = None;
        self.last_dimensions = None;
    }

    /// Check if we have a previous image to compare
    pub fn has_previous(&self) -> bool {
        self.last_image.is_some()
    }
}

impl Default for ChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe change detector
pub struct SharedChangeDetector(Arc<Mutex<ChangeDetector>>);

impl SharedChangeDetector {
    /// Create a new shared change detector
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ChangeDetector::new())))
    }

    /// Create a new shared change detector with custom config
    pub fn with_config(config: ChangeDetectionConfig) -> Self {
        Self(Arc::new(Mutex::new(ChangeDetector::with_config(config))))
    }

    /// Capture screen and detect changes
    pub async fn capture_and_detect(&self) -> Result<(Vec<u8>, ChangeResult)> {
        let mut detector = self.0.lock().await;
        detector.capture_and_detect()
    }

    /// Reset the detector
    pub async fn reset(&self) {
        let mut detector = self.0.lock().await;
        detector.reset();
    }

    /// Get the current config
    pub async fn get_config(&self) -> ChangeDetectionConfig {
        let detector = self.0.lock().await;
        detector.config().clone()
    }

    /// Update the config
    pub async fn set_config(&self, config: ChangeDetectionConfig) {
        let mut detector = self.0.lock().await;
        detector.set_config(config);
    }

    /// Check if we have a previous image
    pub async fn has_previous(&self) -> bool {
        let detector = self.0.lock().await;
        detector.has_previous()
    }
}

impl Default for SharedChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SharedChangeDetector {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_result_should_capture() {
        let config = ChangeDetectionConfig::default();

        assert!(!ChangeResult::NoChange.should_capture(&config));
        assert!(!ChangeResult::MinorChange(0.005).should_capture(&config));
        assert!(ChangeResult::MinorChange(0.02).should_capture(&config));
        assert!(ChangeResult::SignificantChange(0.15).should_capture(&config));
        assert!(ChangeResult::ScreenSwitch(1.0).should_capture(&config));
    }
}
