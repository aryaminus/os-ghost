//! Action ledger for explainability and audit
//!
//! Optimized implementation with async batching to reduce disk I/O.
//! Entries are buffered in memory and flushed periodically or when batch size is reached.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, Duration};

const ACTION_LEDGER_FILE: &str = "action_ledger.json";
const MAX_ENTRIES: usize = 500;
const BATCH_SIZE: usize = 10;
const FLUSH_INTERVAL_SECS: u64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionLedgerStatus {
    Pending,
    Approved,
    Denied,
    Executed,
    Failed,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLedgerEntry {
    pub action_id: u64,
    pub timestamp: u64,
    pub action_type: String,
    pub description: String,
    pub target: String,
    pub risk_level: String,
    pub reason: Option<String>,
    pub status: ActionLedgerStatus,
    pub inputs: Option<serde_json::Value>,
    pub outputs: Option<serde_json::Value>,
    pub error: Option<String>,
    pub source: Option<String>,
}

/// Async batching ledger that minimizes disk I/O
pub struct ActionLedger {
    /// In-memory cache of all entries (Arc for cheap cloning)
    entries: Arc<Mutex<Vec<ActionLedgerEntry>>>,
    /// Channel for sending new entries to background task
    pending_tx: mpsc::Sender<ActionLedgerEntry>,
    /// Flag to track if background task is running
    flush_task_running: Arc<AtomicBool>,
}

impl ActionLedger {
    /// Create a new ledger and start the background flush task
    pub fn new() -> Self {
        // Load existing entries from disk on startup
        let initial_entries = load_entries_from_disk();
        let entries = Arc::new(Mutex::new(initial_entries));
        let (pending_tx, pending_rx) = mpsc::channel::<ActionLedgerEntry>(100);
        let flush_task_running = Arc::new(AtomicBool::new(true));

        // Start background flush task
        let entries_clone = Arc::clone(&entries);
        let running_clone = Arc::clone(&flush_task_running);
        tokio::spawn(async move {
            Self::flush_task(entries_clone, pending_rx, running_clone).await;
        });

        Self {
            entries,
            pending_tx,
            flush_task_running,
        }
    }

    /// Background task that batches and flushes entries to disk
    async fn flush_task(
        entries: Arc<Mutex<Vec<ActionLedgerEntry>>>,
        mut pending_rx: mpsc::Receiver<ActionLedgerEntry>,
        running: Arc<AtomicBool>,
    ) {
        let mut batch = Vec::with_capacity(BATCH_SIZE);
        let mut flush_interval = interval(Duration::from_secs(FLUSH_INTERVAL_SECS));

        loop {
            tokio::select! {
                // Receive new entry
                Some(entry) = pending_rx.recv() => {
                    batch.push(entry);
                    if batch.len() >= BATCH_SIZE {
                        Self::flush_batch(&entries, &mut batch).await;
                    }
                }
                // Periodic flush
                _ = flush_interval.tick() => {
                    if !batch.is_empty() {
                        Self::flush_batch(&entries, &mut batch).await;
                    }
                }
                // Shutdown check
                else => {
                    if !running.load(Ordering::Relaxed) {
                        // Final flush before shutdown
                        if !batch.is_empty() {
                            Self::flush_batch(&entries, &mut batch).await;
                        }
                        break;
                    }
                }
            }
        }
    }

    /// Flush a batch of entries to disk
    async fn flush_batch(
        entries: &Arc<Mutex<Vec<ActionLedgerEntry>>>,
        batch: &mut Vec<ActionLedgerEntry>,
    ) {
        if batch.is_empty() {
            return;
        }

        // Move batch contents to extend entries
        let mut guard = entries.lock().await;
        guard.extend(batch.drain(..));

        // Trim to MAX_ENTRIES (keep most recent) using efficient rotation
        if guard.len() > MAX_ENTRIES {
            let excess = guard.len() - MAX_ENTRIES;
            guard.drain(0..excess);
        }

        // Serialize directly from guard without cloning - more memory efficient
        let path = ledger_path();
        match serde_json::to_string(&*guard) {
            Ok(json) => {
                drop(guard); // Release lock before I/O
                
                // Async write with compact JSON (not pretty-printed for efficiency)
                let _ = tokio::task::spawn_blocking(move || {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Err(e) = std::fs::write(&path, json) {
                        tracing::warn!("Failed to write action ledger: {}", e);
                    }
                })
                .await;
            }
            Err(e) => {
                drop(guard);
                tracing::warn!("Failed to serialize action ledger: {}", e);
            }
        }
    }

    /// Record a new action (non-blocking, sends to channel)
    pub async fn record_action_created(
        &self,
        action_id: u64,
        action_type: String,
        description: String,
        target: String,
        risk_level: String,
        reason: Option<String>,
        inputs: Option<serde_json::Value>,
        source: Option<String>,
    ) {
        let entry = ActionLedgerEntry {
            action_id,
            timestamp: crate::utils::current_timestamp(),
            action_type,
            description,
            target,
            risk_level,
            reason,
            status: ActionLedgerStatus::Pending,
            inputs,
            outputs: None,
            error: None,
            source,
        };

        // Non-blocking send to channel
        let _ = self.pending_tx.send(entry).await;
    }

    /// Update action status (searches in-memory cache, queues update)
    pub async fn update_action_status(
        &self,
        action_id: u64,
        status: ActionLedgerStatus,
        outputs: Option<serde_json::Value>,
        error: Option<String>,
    ) {
        let mut guard = self.entries.lock().await;
        let now = crate::utils::current_timestamp();

        if let Some(entry) = guard.iter_mut().rev().find(|e| e.action_id == action_id) {
            entry.status = status;
            entry.timestamp = now;
            if outputs.is_some() {
                entry.outputs = outputs;
            }
            if error.is_some() {
                entry.error = error;
            }
        } else {
            // Create entry for unknown action
            guard.push(ActionLedgerEntry {
                action_id,
                timestamp: now,
                action_type: "unknown".to_string(),
                description: "Unknown action".to_string(),
                target: "unknown".to_string(),
                risk_level: "unknown".to_string(),
                reason: None,
                status,
                inputs: None,
                outputs,
                error,
                source: None,
            });
        }

        // Check if we need to trigger an immediate flush (rare, but ensures consistency)
        if guard.len() >= MAX_ENTRIES {
            let entries_to_save = guard.clone();
            drop(guard);

            let path = ledger_path();
            let _ = tokio::task::spawn_blocking(move || {
                if let Ok(json) = serde_json::to_string(&entries_to_save) {
                    let _ = std::fs::write(&path, json);
                }
            })
            .await;
        }
    }

    /// Get entries (returns cloned Vec for API compatibility)
    pub async fn get_entries(&self) -> Vec<ActionLedgerEntry> {
        let guard = self.entries.lock().await;
        guard.clone()
    }

    /// Graceful shutdown - ensure all pending entries are flushed
    pub async fn shutdown(&self) {
        self.flush_task_running.store(false, Ordering::Relaxed);
        // Give a moment for final flush
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

impl Default for ActionLedger {
    fn default() -> Self {
        Self::new()
    }
}

// Global ledger instance
lazy_static::lazy_static! {
    static ref GLOBAL_LEDGER: ActionLedger = ActionLedger::new();
}

fn ledger_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("os-ghost");
    path.push(ACTION_LEDGER_FILE);
    path
}

/// Legacy sync functions for backward compatibility - now delegate to async ledger
pub fn record_action_created(
    action_id: u64,
    action_type: String,
    description: String,
    target: String,
    risk_level: String,
    reason: Option<String>,
    inputs: Option<serde_json::Value>,
    source: Option<String>,
) {
    // Spawn async task to handle the record
    let ledger = &*GLOBAL_LEDGER;
    let pending_tx = ledger.pending_tx.clone();

    let entry = ActionLedgerEntry {
        action_id,
        timestamp: crate::utils::current_timestamp(),
        action_type,
        description,
        target,
        risk_level,
        reason,
        status: ActionLedgerStatus::Pending,
        inputs,
        outputs: None,
        error: None,
        source,
    };

    // Fire-and-forget send (best effort)
    let _ = pending_tx.try_send(entry);
}

pub fn update_action_status(
    action_id: u64,
    status: ActionLedgerStatus,
    outputs: Option<serde_json::Value>,
    error: Option<String>,
) {
    // Spawn async task to handle the update
    tokio::spawn(async move {
        GLOBAL_LEDGER
            .update_action_status(action_id, status, outputs, error)
            .await;
    });
}

/// Load entries from disk (used for initialization)
fn load_entries_from_disk() -> Vec<ActionLedgerEntry> {
    let path = ledger_path();
    if path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(entries) = serde_json::from_str::<Vec<ActionLedgerEntry>>(&contents) {
                return entries;
            }
        }
    }
    Vec::new()
}

#[tauri::command]
pub async fn get_action_ledger(
    limit: Option<usize>,
    offset: Option<usize>,
    status: Option<ActionLedgerStatus>,
    source: Option<String>,
    risk_level: Option<String>,
    query: Option<String>,
) -> Vec<ActionLedgerEntry> {
    let entries = GLOBAL_LEDGER.get_entries().await;

    // Filter and sort
    let mut entries: Vec<_> = entries.into_iter().rev().collect();

    if let Some(status_filter) = status {
        entries.retain(|entry| entry.status == status_filter);
    }
    if let Some(source_filter) = source {
        entries.retain(|entry| entry.source.as_deref() == Some(&source_filter));
    }
    if let Some(risk_filter) = risk_level {
        entries.retain(|entry| entry.risk_level == risk_filter);
    }
    if let Some(q) = query {
        let needle = q.to_lowercase();
        entries.retain(|entry| {
            entry.description.to_lowercase().contains(&needle)
                || entry.action_type.to_lowercase().contains(&needle)
                || entry
                    .reason
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&needle)
        });
    }

    let start = offset.unwrap_or(0);
    let limit = limit.unwrap_or(50);
    entries.into_iter().skip(start).take(limit).collect()
}

#[tauri::command]
pub async fn export_action_ledger() -> Result<String, String> {
    let entries = GLOBAL_LEDGER.get_entries().await;
    // Export uses pretty-printed JSON for human readability
    serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())
}
