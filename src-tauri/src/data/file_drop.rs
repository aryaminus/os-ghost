//! File Drop Processing System
//!
//! Watches a directory for new files and processes them when dropped in.
//! Inspired by HermitClaw - files dropped in the workspace are processed with high priority.

use crate::memory::advanced::FileDrop;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::sync::mpsc;
use tokio::time::interval;

/// File drop watcher configuration
#[derive(Debug, Clone)]
pub struct FileDropConfig {
    /// Directory to watch for new files
    pub watch_dir: PathBuf,
    /// Supported file extensions
    pub supported_extensions: Vec<String>,
    /// Check interval in milliseconds
    pub check_interval_ms: u64,
}

impl Default for FileDropConfig {
    fn default() -> Self {
        Self {
            watch_dir: PathBuf::from("."),
            supported_extensions: FileDrop::supported_extensions()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            check_interval_ms: 1000,
        }
    }
}

/// File drop event
#[derive(Debug, Clone)]
pub struct FileDropEvent {
    pub file_drop: FileDrop,
    pub content: Option<String>,
}

/// File drop watcher
pub struct FileDropWatcher {
    config: FileDropConfig,
    known_files: std::collections::HashSet<PathBuf>,
}

impl FileDropWatcher {
    pub fn new(config: FileDropConfig) -> Self {
        Self {
            config,
            known_files: std::collections::HashSet::new(),
        }
    }

    /// Initialize by scanning existing files
    pub async fn init(&mut self) -> std::io::Result<()> {
        if !self.config.watch_dir.exists() {
            fs::create_dir_all(&self.config.watch_dir).await?;
        }

        let mut dir = fs::read_dir(&self.config.watch_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                self.known_files.insert(path);
            }
        }
        Ok(())
    }

    /// Check for new files and emit events
    pub async fn check_for_new_files(&mut self) -> Vec<FileDropEvent> {
        let mut events = Vec::new();

        if let Ok(mut dir) = fs::read_dir(&self.config.watch_dir).await {
            while let Ok(Some(entry)) = dir.next_entry().await {
                let path = entry.path();
                
                // Skip if not a file or already known
                if !path.is_file() || self.known_files.contains(&path) {
                    continue;
                }

                // Check if supported extension
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if !self.config.supported_extensions.contains(&ext_str) {
                        continue;
                    }

                    // Mark as known
                    self.known_files.insert(path.clone());

                    // Read content for text files
                    let content = if is_text_extension(&ext_str) {
                        fs::read_to_string(&path).await.ok()
                    } else {
                        None
                    };

                    let file_drop = FileDrop {
                        filename: path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        path: path.to_string_lossy().to_string(),
                        content_type: get_content_type(&ext_str),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0),
                    };

                    events.push(FileDropEvent {
                        file_drop,
                        content,
                    });
                }
            }
        }

        events
    }

    /// Run the watcher as a background task
    pub async fn run(&mut self, tx: mpsc::Sender<FileDropEvent>) {
        if let Err(e) = self.init().await {
            tracing::error!("Failed to init file drop watcher: {}", e);
            return;
        }

        let mut check_interval = interval(Duration::from_millis(self.config.check_interval_ms));

        loop {
            check_interval.tick().await;
            
            let events = self.check_for_new_files().await;
            for event in events {
                if let Err(e) = tx.send(event).await {
                    tracing::error!("Failed to send file drop event: {}", e);
                    return;
                }
            }
        }
    }
}

/// Check if file extension is text
fn is_text_extension(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "md" | "py" | "json" | "csv" | "yaml" | "toml" |
        "js" | "ts" | "jsx" | "tsx" | "html" | "css" | "sh" | "log" |
        "rs" | "java" | "c" | "cpp" | "h" | "go" | "rb" | "php"
    )
}

/// Get MIME content type from extension
fn get_content_type(ext: &str) -> String {
    match ext {
        "txt" | "log" | "sh" | "py" | "rs" | "js" | "ts" | "java" | "c" | "cpp" | "h" | "go" | "rb" | "php" => "text/plain".to_string(),
        "md" => "text/markdown".to_string(),
        "json" => "application/json".to_string(),
        "csv" => "text/csv".to_string(),
        "yaml" | "yml" => "application/yaml".to_string(),
        "toml" => "application/toml".to_string(),
        "html" | "htm" => "text/html".to_string(),
        "css" => "text/css".to_string(),
        "pdf" => "application/pdf".to_string(),
        "png" => "image/png".to_string(),
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "gif" => "image/gif".to_string(),
        "webp" => "image/webp".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type() {
        assert_eq!(get_content_type("txt"), "text/plain");
        assert_eq!(get_content_type("png"), "image/png");
        assert_eq!(get_content_type("json"), "application/json");
    }

    #[test]
    fn test_is_text_extension() {
        assert!(is_text_extension("txt"));
        assert!(is_text_extension("md"));
        assert!(is_text_extension("rs"));
        assert!(!is_text_extension("png"));
    }
}
