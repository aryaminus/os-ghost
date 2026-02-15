//! Hybrid Memory Search - SQLite + FTS5 + Vector
//!
//! Reference: ZeroClaw memory implementation

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct HybridMemory {
    conn: Connection,
    #[allow(dead_code)]
    vector_dim: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredResult {
    pub key: String,
    pub content: String,
    pub score: f32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_entries: usize,
    pub entries_with_embeddings: usize,
}

impl HybridMemory {
    pub fn new(db_path: Option<PathBuf>, vector_dim: usize) -> Result<Self, String> {
        let path = db_path.unwrap_or_else(|| {
            let mut p = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
            p.push("os-ghost");
            p.push("memory.db");
            p
        });

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(&path).map_err(|e| e.to_string())?;
        let memory = Self { conn, vector_dim };
        memory.init_schema()?;

        tracing::info!("Hybrid memory initialized at {:?}", path);
        Ok(memory)
    }

    fn init_schema(&self) -> Result<(), String> {
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                embedding BLOB,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
                [],
            )
            .map_err(|e| e.to_string())?;

        self.conn
            .execute(
                "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                key, content, content=memories, content_rowid=id
            )",
                [],
            )
            .map_err(|e| e.to_string())?;

        self.conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key)",
                [],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn store(&self, key: &str, content: &str, embedding: Option<&[f32]>) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let embedding_blob =
            embedding.map(|e| e.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());

        self.conn.execute(
            "INSERT OR REPLACE INTO memories (key, content, embedding, created_at, updated_at)
             VALUES (?1, ?2, ?3, COALESCE((SELECT created_at FROM memories WHERE key = ?1), ?4), ?4)",
            params![key, content, embedding_blob, now],
        ).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn recall(&self, key: &str) -> Result<Option<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT content FROM memories WHERE key = ?1")
            .map_err(|e| e.to_string())?;

        match stmt.query_row([key], |row| row.get::<_, String>(0)) {
            Ok(content) => Ok(Some(content)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn delete(&self, key: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM memories WHERE key = ?1", [key])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_keys(&self) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT key FROM memories ORDER BY updated_at DESC")
            .map_err(|e| e.to_string())?;

        let keys = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(keys)
    }

    pub fn keyword_search(&self, query: &str, limit: usize) -> Result<Vec<ScoredResult>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT m.key, m.content, bm25(memories_fts) as score
             FROM memories_fts f
             JOIN memories m ON f.rowid = m.id
             WHERE memories_fts MATCH ?1
             ORDER BY score
             LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let results = stmt
            .query_map(params![query, limit as i64], |row| {
                Ok(ScoredResult {
                    key: row.get(0)?,
                    content: row.get(1)?,
                    score: row.get(2)?,
                    source: "keyword".to_string(),
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    pub fn stats(&self) -> Result<MemoryStats, String> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;

        let with_embedding: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE embedding IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        Ok(MemoryStats {
            total_entries: total as usize,
            entries_with_embeddings: with_embedding as usize,
        })
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

lazy_static::lazy_static! {
    static ref HYBRID_MEMORY: Mutex<Option<HybridMemory>> = Mutex::new(None);
}

pub fn init_hybrid_memory(vector_dim: usize) -> Result<(), String> {
    let memory = HybridMemory::new(None, vector_dim)?;
    if let Ok(mut m) = HYBRID_MEMORY.lock() {
        *m = Some(memory);
    }
    Ok(())
}

#[tauri::command]
pub fn memory_store(
    key: String,
    content: String,
    embedding: Option<Vec<f32>>,
) -> Result<(), String> {
    if let Ok(m) = HYBRID_MEMORY.lock() {
        if let Some(mem) = m.as_ref() {
            return mem
                .store(&key, &content, embedding.as_deref())
                .map_err(|e| e.to_string());
        }
    }
    Err("Memory not initialized".to_string())
}

#[tauri::command]
pub fn memory_recall(key: String) -> Result<Option<String>, String> {
    if let Ok(m) = HYBRID_MEMORY.lock() {
        if let Some(mem) = m.as_ref() {
            return mem.recall(&key).map_err(|e| e.to_string());
        }
    }
    Err("Memory not initialized".to_string())
}

#[tauri::command]
pub fn memory_delete(key: String) -> Result<(), String> {
    if let Ok(m) = HYBRID_MEMORY.lock() {
        if let Some(mem) = m.as_ref() {
            return mem.delete(&key).map_err(|e| e.to_string());
        }
    }
    Err("Memory not initialized".to_string())
}

#[tauri::command]
pub fn memory_search(query: String, limit: Option<usize>) -> Result<Vec<ScoredResult>, String> {
    if let Ok(m) = HYBRID_MEMORY.lock() {
        if let Some(mem) = m.as_ref() {
            return mem
                .keyword_search(&query, limit.unwrap_or(10))
                .map_err(|e| e.to_string());
        }
    }
    Err("Memory not initialized".to_string())
}

#[tauri::command]
pub fn memory_stats() -> Result<MemoryStats, String> {
    if let Ok(m) = HYBRID_MEMORY.lock() {
        if let Some(mem) = m.as_ref() {
            return mem.stats().map_err(|e| e.to_string());
        }
    }
    Err("Memory not initialized".to_string())
}
