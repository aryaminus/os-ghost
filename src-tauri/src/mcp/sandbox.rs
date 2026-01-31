//! Sandboxed System Tools
//!
//! Implements file system and shell access with progressive trust levels
//! following Anthropic's computer-use guidelines and OpenAI Operator patterns.
//!
//! Security Model:
//! - **Path Allowlists**: Only permitted directories can be accessed
//! - **Command Categorization**: Shell commands grouped by risk level
//! - **Trust Levels**: Progressive access earned through safe behavior
//! - **Confirmation Gates**: High-risk ops always require user approval
//! - **Audit Logging**: Every action logged for accountability
//!
//! Reference:
//! - Anthropic: "Sandbox side-effects, log all actions for auditability"
//! - CUA.ai: "Progressive trust earned through demonstrated safety"

use crate::actions::ActionRiskLevel;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

const SANDBOX_CONFIG_FILE: &str = "sandbox_settings.json";

// ============================================================================
// Trust Level System
// ============================================================================

/// Progressive trust levels for system access
/// Trust must be earned through safe behavior over time
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// No system access allowed (default for new users)
    #[default]
    Untrusted,
    /// Read-only access to allowed paths
    ReadOnly,
    /// Read/write to user-designated safe directories
    Limited,
    /// Extended access with confirmation for sensitive ops
    Elevated,
    /// Full access within guardrails (requires explicit opt-in)
    Full,
}

impl TrustLevel {
    /// Minimum trust level required for file read operations
    pub fn min_for_file_read() -> Self {
        TrustLevel::ReadOnly
    }

    /// Minimum trust level required for file write operations
    pub fn min_for_file_write() -> Self {
        TrustLevel::Limited
    }

    /// Minimum trust level required for shell execution
    pub fn min_for_shell() -> Self {
        TrustLevel::Elevated
    }

    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            TrustLevel::Untrusted => "No system access",
            TrustLevel::ReadOnly => "Read-only file access",
            TrustLevel::Limited => "Read/write to safe directories",
            TrustLevel::Elevated => "Extended access with confirmations",
            TrustLevel::Full => "Full access within guardrails",
        }
    }

    /// Check if this level permits the given operation
    pub fn permits(&self, required: TrustLevel) -> bool {
        *self >= required
    }
}

// ============================================================================
// Sandbox Configuration
// ============================================================================

/// Sandbox configuration for file system access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Trust level for this session
    pub trust_level: TrustLevel,
    /// Explicitly allowed read paths (directories or files)
    pub read_allowlist: Vec<PathBuf>,
    /// Explicitly allowed write paths (directories only)
    pub write_allowlist: Vec<PathBuf>,
    /// Blocked paths (never accessible regardless of trust)
    pub blocklist: Vec<PathBuf>,
    /// Allowed shell command categories
    pub allowed_shell_categories: HashSet<ShellCategory>,
    /// Whether to require confirmation for all writes
    pub confirm_all_writes: bool,
    /// Maximum file size for read operations (bytes)
    pub max_read_size: usize,
    /// Trust score (0-100) - increases with safe actions
    pub trust_score: u32,
    /// Number of successful safe operations
    pub safe_operations_count: u64,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        
        Self {
            trust_level: TrustLevel::Untrusted,
            read_allowlist: vec![
                home.join("Documents"),
                home.join("Downloads"),
                home.join("Desktop"),
            ],
            write_allowlist: vec![],
            blocklist: vec![
                PathBuf::from("/etc"),
                PathBuf::from("/System"),
                PathBuf::from("/usr"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
                PathBuf::from("/var"),
                PathBuf::from("/private"),
                home.join(".ssh"),
                home.join(".gnupg"),
                home.join(".aws"),
                home.join(".config/gcloud"),
                home.join("Library/Keychains"),
                home.join(".password-store"),
                home.join(".netrc"),
            ],
            allowed_shell_categories: HashSet::new(),
            confirm_all_writes: true,
            max_read_size: 1024 * 1024,
            trust_score: 0,
            safe_operations_count: 0,
        }
    }
}

impl SandboxConfig {
    /// Get the path to the settings file
    fn settings_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("os-ghost");
        path.push(SANDBOX_CONFIG_FILE);
        path
    }

    /// Load sandbox config from disk, or return default
    pub fn load() -> Self {
        let path = Self::settings_path();
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str(&contents) {
                    return config;
                }
            }
        }
        Self::default()
    }

    /// Save sandbox config to disk
    pub fn save(&self) -> Result<(), String> {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let contents = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, contents).map_err(|e| e.to_string())?;
        tracing::debug!("Saved sandbox config to {:?}", path);
        Ok(())
    }
}

impl SandboxConfig {
    /// Check if a path is allowed for reading
    pub fn can_read(&self, path: &Path) -> Result<(), SandboxError> {
        if !self.trust_level.permits(TrustLevel::min_for_file_read()) {
            return Err(SandboxError::InsufficientTrust {
                required: TrustLevel::ReadOnly,
                current: self.trust_level,
            });
        }

        let canonical = path.canonicalize().map_err(|_| SandboxError::PathNotFound)?;

        for blocked in &self.blocklist {
            if let Ok(blocked_canonical) = blocked.canonicalize() {
                if canonical.starts_with(&blocked_canonical) {
                    return Err(SandboxError::PathBlocked(path.to_path_buf()));
                }
            }
            if canonical.starts_with(blocked) {
                return Err(SandboxError::PathBlocked(path.to_path_buf()));
            }
        }

        for allowed in &self.read_allowlist {
            if let Ok(allowed_canonical) = allowed.canonicalize() {
                if canonical.starts_with(&allowed_canonical) {
                    return Ok(());
                }
            }
        }

        Err(SandboxError::PathNotAllowed(path.to_path_buf()))
    }

    /// Check if a path is allowed for writing
    pub fn can_write(&self, path: &Path) -> Result<(), SandboxError> {
        if !self.trust_level.permits(TrustLevel::min_for_file_write()) {
            return Err(SandboxError::InsufficientTrust {
                required: TrustLevel::Limited,
                current: self.trust_level,
            });
        }

        let parent = path.parent().ok_or(SandboxError::InvalidPath)?;
        let canonical_parent = if parent.exists() {
            parent.canonicalize().map_err(|_| SandboxError::PathNotFound)?
        } else {
            parent.to_path_buf()
        };

        for blocked in &self.blocklist {
            if let Ok(blocked_canonical) = blocked.canonicalize() {
                if canonical_parent.starts_with(&blocked_canonical) {
                    return Err(SandboxError::PathBlocked(path.to_path_buf()));
                }
            }
        }

        for allowed in &self.write_allowlist {
            if let Ok(allowed_canonical) = allowed.canonicalize() {
                if canonical_parent.starts_with(&allowed_canonical) {
                    return Ok(());
                }
            }
        }

        Err(SandboxError::PathNotAllowed(path.to_path_buf()))
    }

    pub fn record_safe_operation(&mut self) {
        self.safe_operations_count += 1;
        if self.trust_score < 100 {
            self.trust_score = (self.trust_score + 1).min(100);
        }
    }

    pub fn record_denied_operation(&mut self) {
        self.trust_score = self.trust_score.saturating_sub(5);
    }
}

/// Sandbox-specific errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SandboxError {
    PathBlocked(PathBuf),
    PathNotAllowed(PathBuf),
    PathNotFound,
    InvalidPath,
    InsufficientTrust { required: TrustLevel, current: TrustLevel },
    FileTooLarge { size: usize, max: usize },
    ShellCategoryNotAllowed(ShellCategory),
    CommandBlocked(String),
    IoError(String),
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxError::PathBlocked(p) => write!(f, "Path is blocked: {}", p.display()),
            SandboxError::PathNotAllowed(p) => write!(f, "Path not in allowlist: {}", p.display()),
            SandboxError::PathNotFound => write!(f, "Path not found"),
            SandboxError::InvalidPath => write!(f, "Invalid path format"),
            SandboxError::InsufficientTrust { required, current } => {
                write!(f, "Insufficient trust: requires {:?}, have {:?}", required, current)
            }
            SandboxError::FileTooLarge { size, max } => {
                write!(f, "File too large: {} bytes (max {})", size, max)
            }
            SandboxError::ShellCategoryNotAllowed(cat) => {
                write!(f, "Shell category not allowed: {:?}", cat)
            }
            SandboxError::CommandBlocked(cmd) => write!(f, "Command blocked: {}", cmd),
            SandboxError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for SandboxError {}

// ============================================================================
// Shell Command Categorization
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ShellCategory {
    ReadInfo,
    Search,
    PackageInfo,
    GitRead,
    GitWrite,
    FileManipulation,
    FileDeletion,
    Network,
    ProcessManagement,
    SystemAdmin,
    Arbitrary,
}

impl ShellCategory {
    pub fn risk_level(&self) -> ActionRiskLevel {
        match self {
            ShellCategory::ReadInfo | ShellCategory::Search | 
            ShellCategory::PackageInfo | ShellCategory::GitRead => ActionRiskLevel::Low,
            ShellCategory::GitWrite | ShellCategory::FileManipulation | 
            ShellCategory::Network => ActionRiskLevel::Medium,
            _ => ActionRiskLevel::High,
        }
    }

    pub fn min_trust_level(&self) -> TrustLevel {
        match self {
            ShellCategory::ReadInfo | ShellCategory::Search |
            ShellCategory::PackageInfo | ShellCategory::GitRead => TrustLevel::ReadOnly,
            ShellCategory::GitWrite | ShellCategory::FileManipulation |
            ShellCategory::Network => TrustLevel::Limited,
            ShellCategory::FileDeletion | ShellCategory::ProcessManagement => TrustLevel::Elevated,
            ShellCategory::SystemAdmin | ShellCategory::Arbitrary => TrustLevel::Full,
        }
    }

    pub fn always_confirm(&self) -> bool {
        matches!(self, ShellCategory::FileDeletion | ShellCategory::ProcessManagement |
                       ShellCategory::SystemAdmin | ShellCategory::Arbitrary)
    }
}

lazy_static! {
    static ref COMMAND_PATTERNS: Vec<(Regex, ShellCategory)> = vec![
        (Regex::new(r"^(ls|dir|pwd|cat|head|tail|wc|file|stat|which|whereis|type|echo|printf)\b").unwrap(), ShellCategory::ReadInfo),
        (Regex::new(r"^(find|grep|rg|ag|locate|mdfind)\b").unwrap(), ShellCategory::Search),
        (Regex::new(r"^(brew (list|info|search)|npm (list|ls)|pip (list|show)|cargo (tree|metadata))\b").unwrap(), ShellCategory::PackageInfo),
        (Regex::new(r"^git\s+(status|log|diff|show|branch|remote|tag|stash list)\b").unwrap(), ShellCategory::GitRead),
        (Regex::new(r"^git\s+(add|commit|push|pull|merge|rebase|checkout|reset|stash)\b").unwrap(), ShellCategory::GitWrite),
        (Regex::new(r"^(cp|mv|mkdir|touch|ln)\b").unwrap(), ShellCategory::FileManipulation),
        (Regex::new(r"^(rm|rmdir)\b").unwrap(), ShellCategory::FileDeletion),
        (Regex::new(r"^(curl|wget|ping|nc|ssh|scp|rsync)\b").unwrap(), ShellCategory::Network),
        (Regex::new(r"^(ps|kill|killall|pkill|top|htop)\b").unwrap(), ShellCategory::ProcessManagement),
        (Regex::new(r"^(sudo|chmod|chown|chgrp|mount|umount)\b").unwrap(), ShellCategory::SystemAdmin),
    ];
    
    static ref BLOCKED_COMMANDS: Vec<Regex> = vec![
        Regex::new(r"(?i)(mkfs|fdisk|dd\s+if=.*of=/dev|diskutil\s+erase)").unwrap(),
        Regex::new(r":\(\)\s*\{\s*:\|:&\s*\}").unwrap(),
        Regex::new(r"rm\s+(-rf?|--recursive)\s+/\s*$").unwrap(),
        Regex::new(r"dd\s+.*of=/dev/(sda|hda|disk0)").unwrap(),
        Regex::new(r"csrutil\s+disable|spctl\s+--master-disable").unwrap(),
        Regex::new(r"security\s+(find|export|dump)-").unwrap(),
        Regex::new(r"(cat|less|more|head|tail)\s+/etc/(passwd|shadow)").unwrap(),
    ];
    
    static ref SANDBOX_CONFIG: RwLock<SandboxConfig> = RwLock::new(SandboxConfig::load());
}

pub fn categorize_command(command: &str) -> ShellCategory {
    let trimmed = command.trim();
    for (pattern, category) in COMMAND_PATTERNS.iter() {
        if pattern.is_match(trimmed) {
            return *category;
        }
    }
    ShellCategory::Arbitrary
}

pub fn is_command_blocked(command: &str) -> bool {
    BLOCKED_COMMANDS.iter().any(|pattern| pattern.is_match(command))
}

pub fn get_sandbox_config() -> SandboxConfig {
    SANDBOX_CONFIG.read().unwrap().clone()
}

pub fn update_sandbox_config<F>(f: F) where F: FnOnce(&mut SandboxConfig) {
    let mut config = SANDBOX_CONFIG.write().unwrap();
    f(&mut config);
    // Persist to disk
    if let Err(e) = config.save() {
        tracing::warn!("Failed to save sandbox config: {}", e);
    }
}

// ============================================================================
// Result Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOpResult {
    pub success: bool,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_written: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellOpResult {
    pub success: bool,
    pub command: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDirResult {
    pub success: bool,
    pub path: String,
    pub entries: Vec<DirEntry>,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_sandbox_settings() -> SandboxConfig {
    get_sandbox_config()
}

#[tauri::command]
pub fn set_sandbox_trust_level(level: String) -> Result<SandboxConfig, String> {
    let trust_level = match level.as_str() {
        "untrusted" => TrustLevel::Untrusted,
        "read_only" => TrustLevel::ReadOnly,
        "limited" => TrustLevel::Limited,
        "elevated" => TrustLevel::Elevated,
        "full" => TrustLevel::Full,
        _ => return Err("Invalid trust level".to_string()),
    };
    update_sandbox_config(|c| c.trust_level = trust_level);
    Ok(get_sandbox_config())
}

#[tauri::command]
pub fn add_sandbox_read_path(path: String) -> Result<SandboxConfig, String> {
    let path = PathBuf::from(&path);
    if !path.exists() {
        return Err("Path does not exist".to_string());
    }
    update_sandbox_config(|c| {
        if !c.read_allowlist.contains(&path) {
            c.read_allowlist.push(path);
        }
    });
    Ok(get_sandbox_config())
}

#[tauri::command]
pub fn remove_sandbox_read_path(path: String) -> Result<SandboxConfig, String> {
    let path = PathBuf::from(&path);
    update_sandbox_config(|c| c.read_allowlist.retain(|p| p != &path));
    Ok(get_sandbox_config())
}

#[tauri::command]
pub fn add_sandbox_write_path(path: String) -> Result<SandboxConfig, String> {
    let path = PathBuf::from(&path);
    if !path.exists() || !path.is_dir() {
        return Err("Path must be an existing directory".to_string());
    }
    update_sandbox_config(|c| {
        if !c.write_allowlist.contains(&path) {
            c.write_allowlist.push(path);
        }
    });
    Ok(get_sandbox_config())
}

#[tauri::command]
pub fn remove_sandbox_write_path(path: String) -> Result<SandboxConfig, String> {
    let path = PathBuf::from(&path);
    update_sandbox_config(|c| c.write_allowlist.retain(|p| p != &path));
    Ok(get_sandbox_config())
}

fn parse_shell_category(category: &str) -> Result<ShellCategory, String> {
    match category {
        "read_info" => Ok(ShellCategory::ReadInfo),
        "search" => Ok(ShellCategory::Search),
        "package_info" => Ok(ShellCategory::PackageInfo),
        "git_read" => Ok(ShellCategory::GitRead),
        "git_write" => Ok(ShellCategory::GitWrite),
        "file_manipulation" => Ok(ShellCategory::FileManipulation),
        "file_deletion" => Ok(ShellCategory::FileDeletion),
        "network" => Ok(ShellCategory::Network),
        "process_management" => Ok(ShellCategory::ProcessManagement),
        "system_admin" => Ok(ShellCategory::SystemAdmin),
        "arbitrary" => Ok(ShellCategory::Arbitrary),
        _ => Err("Invalid shell category".to_string()),
    }
}

#[tauri::command]
pub fn enable_shell_category(category: String) -> Result<SandboxConfig, String> {
    let cat = parse_shell_category(&category)?;
    update_sandbox_config(|c| { c.allowed_shell_categories.insert(cat); });
    Ok(get_sandbox_config())
}

#[tauri::command]
pub fn disable_shell_category(category: String) -> Result<SandboxConfig, String> {
    let cat = parse_shell_category(&category)?;
    update_sandbox_config(|c| { c.allowed_shell_categories.remove(&cat); });
    Ok(get_sandbox_config())
}

#[tauri::command]
pub fn set_confirm_all_writes(enabled: bool) -> Result<SandboxConfig, String> {
    update_sandbox_config(|c| c.confirm_all_writes = enabled);
    Ok(get_sandbox_config())
}

#[tauri::command]
pub fn set_max_read_size(max_read_size: usize) -> Result<SandboxConfig, String> {
    let capped = max_read_size.clamp(1024, 50 * 1024 * 1024);
    update_sandbox_config(|c| c.max_read_size = capped);
    Ok(get_sandbox_config())
}

#[tauri::command]
pub fn apply_sandbox_baseline() -> Result<SandboxConfig, String> {
    update_sandbox_config(|c| {
        c.trust_level = TrustLevel::Limited;
        c.confirm_all_writes = true;
        c.allowed_shell_categories = [
            ShellCategory::ReadInfo,
            ShellCategory::Search,
            ShellCategory::PackageInfo,
            ShellCategory::GitRead,
        ]
        .into_iter()
        .collect();
    });
    Ok(get_sandbox_config())
}

#[tauri::command]
pub fn sandbox_read_file(path: String) -> FileOpResult {
    let path_buf = PathBuf::from(&path);
    let config = get_sandbox_config();
    
    if let Err(e) = config.can_read(&path_buf) {
        return FileOpResult { success: false, path, content: None, bytes_written: None, error: Some(e.to_string()), backup_path: None };
    }
    
    match std::fs::metadata(&path_buf) {
        Ok(metadata) if metadata.len() as usize > config.max_read_size => {
            return FileOpResult { success: false, path, content: None, bytes_written: None, 
                error: Some(format!("File too large: {} bytes (max {})", metadata.len(), config.max_read_size)), backup_path: None };
        }
        Err(e) => {
            return FileOpResult { success: false, path, content: None, bytes_written: None, error: Some(e.to_string()), backup_path: None };
        }
        _ => {}
    }
    
    match std::fs::read_to_string(&path_buf) {
        Ok(contents) => {
            update_sandbox_config(|c| c.record_safe_operation());
            FileOpResult { success: true, path, content: Some(contents), bytes_written: None, error: None, backup_path: None }
        }
        Err(e) => FileOpResult { success: false, path, content: None, bytes_written: None, error: Some(e.to_string()), backup_path: None }
    }
}

#[tauri::command]
pub fn sandbox_write_file(path: String, content: String, create_dirs: Option<bool>) -> FileOpResult {
    let path_buf = PathBuf::from(&path);
    let config = get_sandbox_config();
    
    if let Err(e) = config.can_write(&path_buf) {
        return FileOpResult { success: false, path, content: None, bytes_written: None, error: Some(e.to_string()), backup_path: None };
    }
    
    if create_dirs.unwrap_or(false) {
        if let Some(parent) = path_buf.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return FileOpResult { success: false, path, content: None, bytes_written: None, error: Some(e.to_string()), backup_path: None };
            }
        }
    }
    
    let backup_path = if path_buf.exists() {
        let backup = path_buf.with_extension("osghost.bak");
        std::fs::copy(&path_buf, &backup).ok().map(|_| backup.to_string_lossy().to_string())
    } else { None };
    
    match std::fs::write(&path_buf, &content) {
        Ok(_) => {
            update_sandbox_config(|c| c.record_safe_operation());
            FileOpResult { success: true, path, content: None, bytes_written: Some(content.len()), error: None, backup_path }
        }
        Err(e) => FileOpResult { success: false, path, content: None, bytes_written: None, error: Some(e.to_string()), backup_path: None }
    }
}

#[tauri::command]
pub fn sandbox_list_dir(path: String, include_hidden: Option<bool>) -> ListDirResult {
    let path_buf = PathBuf::from(&path);
    let include_hidden = include_hidden.unwrap_or(false);
    let config = get_sandbox_config();
    
    if let Err(e) = config.can_read(&path_buf) {
        return ListDirResult { success: false, path, entries: vec![], count: 0, error: Some(e.to_string()) };
    }
    
    match std::fs::read_dir(&path_buf) {
        Ok(dir_entries) => {
            let entries: Vec<DirEntry> = dir_entries
                .filter_map(|e| e.ok())
                .filter(|e| include_hidden || !e.file_name().to_string_lossy().starts_with('.'))
                .map(|e| {
                    let meta = e.metadata().ok();
                    DirEntry {
                        name: e.file_name().to_string_lossy().to_string(),
                        path: e.path().to_string_lossy().to_string(),
                        is_dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                        is_file: meta.as_ref().map(|m| m.is_file()).unwrap_or(false),
                        size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    }
                })
                .collect();
            update_sandbox_config(|c| c.record_safe_operation());
            let count = entries.len();
            ListDirResult { success: true, path, entries, count, error: None }
        }
        Err(e) => ListDirResult { success: false, path, entries: vec![], count: 0, error: Some(e.to_string()) }
    }
}

#[tauri::command]
pub async fn sandbox_execute_shell(command: String, working_dir: Option<String>) -> ShellOpResult {
    let config = get_sandbox_config();
    
    if !config.trust_level.permits(TrustLevel::min_for_shell()) {
        return ShellOpResult { success: false, command, category: "blocked".to_string(), exit_code: None, stdout: None, stderr: None,
            error: Some(format!("Shell requires {:?} trust, current: {:?}", TrustLevel::Elevated, config.trust_level)) };
    }
    
    if is_command_blocked(&command) {
        update_sandbox_config(|c| c.record_denied_operation());
        return ShellOpResult { success: false, command, category: "blocked".to_string(), exit_code: None, stdout: None, stderr: None,
            error: Some("Command blocked by security policy".to_string()) };
    }
    
    let category = categorize_command(&command);
    if !config.allowed_shell_categories.contains(&category) {
        return ShellOpResult { success: false, command, category: format!("{:?}", category), exit_code: None, stdout: None, stderr: None,
            error: Some(format!("Category {:?} not allowed", category)) };
    }
    
    if !config.trust_level.permits(category.min_trust_level()) {
        return ShellOpResult { success: false, command, category: format!("{:?}", category), exit_code: None, stdout: None, stderr: None,
            error: Some(format!("Category {:?} requires {:?}", category, category.min_trust_level())) };
    }
    
    let mut cmd = if cfg!(target_os = "windows") {
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.arg("/C").arg(&command);
        cmd
    } else {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(&command);
        cmd
    };
    if let Some(dir) = working_dir { cmd.current_dir(dir); }
    
    match tokio::time::timeout(std::time::Duration::from_secs(30), cmd.output()).await {
        Ok(Ok(output)) => {
            update_sandbox_config(|c| c.record_safe_operation());
            ShellOpResult {
                success: output.status.success(), command, category: format!("{:?}", category),
                exit_code: output.status.code(),
                stdout: Some(String::from_utf8_lossy(&output.stdout).to_string()),
                stderr: Some(String::from_utf8_lossy(&output.stderr).to_string()),
                error: None
            }
        }
        Ok(Err(e)) => ShellOpResult { success: false, command, category: format!("{:?}", category), exit_code: None, stdout: None, stderr: None, error: Some(e.to_string()) },
        Err(_) => ShellOpResult { success: false, command, category: format!("{:?}", category), exit_code: None, stdout: None, stderr: None, error: Some("Timeout after 30s".to_string()) }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_level_ordering() {
        assert!(TrustLevel::Full > TrustLevel::Elevated);
        assert!(TrustLevel::Elevated > TrustLevel::Limited);
        assert!(TrustLevel::Limited > TrustLevel::ReadOnly);
        assert!(TrustLevel::ReadOnly > TrustLevel::Untrusted);
    }

    #[test]
    fn test_command_categorization() {
        assert_eq!(categorize_command("ls -la"), ShellCategory::ReadInfo);
        assert_eq!(categorize_command("grep -r pattern ."), ShellCategory::Search);
        assert_eq!(categorize_command("git status"), ShellCategory::GitRead);
        assert_eq!(categorize_command("git push origin main"), ShellCategory::GitWrite);
        assert_eq!(categorize_command("rm -rf node_modules"), ShellCategory::FileDeletion);
        assert_eq!(categorize_command("some_random_command"), ShellCategory::Arbitrary);
    }

    #[test]
    fn test_blocked_commands() {
        assert!(is_command_blocked("rm -rf /"));
        assert!(is_command_blocked("dd if=/dev/zero of=/dev/sda"));
        assert!(is_command_blocked("csrutil disable"));
        assert!(!is_command_blocked("ls -la"));
        assert!(!is_command_blocked("git status"));
    }

    #[test]
    fn test_trust_score() {
        let mut config = SandboxConfig::default();
        assert_eq!(config.trust_score, 0);
        config.record_safe_operation();
        assert_eq!(config.trust_score, 1);
        config.record_denied_operation();
        assert_eq!(config.trust_score, 0);
    }
    
    #[test]
    fn test_shell_category_parsing() {
        assert_eq!(parse_shell_category("read_info").unwrap(), ShellCategory::ReadInfo);
        assert!(parse_shell_category("invalid").is_err());
    }
}
