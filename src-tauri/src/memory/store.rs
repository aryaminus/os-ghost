//! Sled-based memory store
//! Persistent key-value storage with crash safety

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use sled::Db;
use std::path::PathBuf;
use std::sync::Arc;

/// Main memory store backed by sled
pub struct MemoryStore {
    db: Arc<Db>,
}

impl MemoryStore {
    /// Create or open a memory store at the default location
    pub fn new() -> Result<Self> {
        let path = Self::default_path()?;
        Self::open(path)
    }

    /// Open a memory store at a specific path
    pub fn open(path: PathBuf) -> Result<Self> {
        let db = sled::open(&path)?;
        Ok(Self { db: Arc::new(db) })
    }

    /// Get the default database path
    fn default_path() -> Result<PathBuf> {
        let mut path =
            dirs::config_dir().ok_or_else(|| anyhow::anyhow!("No config directory found"))?;
        path.push("os-ghost");
        path.push("memory.db");

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        Ok(path)
    }

    /// Get a typed value by key
    pub fn get<T: DeserializeOwned>(&self, tree: &str, key: &str) -> Result<Option<T>> {
        let tree = self.db.open_tree(tree)?;
        match tree.get(key.as_bytes())? {
            Some(bytes) => {
                let value: T = serde_json::from_slice(&bytes)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Set a typed value by key
    pub fn set<T: Serialize>(&self, tree: &str, key: &str, value: &T) -> Result<()> {
        let tree = self.db.open_tree(tree)?;
        let bytes = serde_json::to_vec(value)?;
        tree.insert(key.as_bytes(), bytes)?;
        // Performance Fix: Removed automatic flush() on every write.
        // Callers should call flush() explicitly at checkpoints.
        Ok(())
    }

    /// Atomically update a value (Read-Modify-Write)
    /// Prevents race conditions when multiple threads modify the same key
    ///
    /// # Note
    /// If serialization fails during the update, the value is silently deleted.
    /// This is a sled API limitation - the closure cannot return errors.
    /// Callers should ensure their types are always serializable.
    pub fn update<T, F>(&self, tree: &str, key: &str, f: F) -> Result<Option<T>>
    where
        T: Serialize + DeserializeOwned,
        F: Fn(Option<T>) -> Option<T>,
    {
        let tree = self.db.open_tree(tree)?;

        let res = tree.fetch_and_update(key.as_bytes(), |old_bytes| {
            // Deserialize old value (if any)
            let old_val = old_bytes.and_then(|bytes| serde_json::from_slice::<T>(bytes).ok());

            // Apply update function
            let new_val = f(old_val);

            // Serialize new value - if serialization fails, treat as deletion
            // This is safer than panicking inside the atomic operation
            new_val.and_then(|v| serde_json::to_vec(&v).ok())
        })?;

        // Return the new value
        match res {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Delete a key
    pub fn delete(&self, tree: &str, key: &str) -> Result<()> {
        let tree = self.db.open_tree(tree)?;
        tree.remove(key.as_bytes())?;
        Ok(())
    }

    /// List all keys in a tree
    pub fn list_keys(&self, tree: &str) -> Result<Vec<String>> {
        let tree = self.db.open_tree(tree)?;
        let keys: Vec<String> = tree
            .iter()
            .keys()
            .filter_map(|k| k.ok())
            .filter_map(|k| String::from_utf8(k.to_vec()).ok())
            .collect();
        Ok(keys)
    }

    /// Count items in a tree (O(1))
    pub fn count(&self, tree: &str) -> Result<usize> {
        let tree = self.db.open_tree(tree)?;
        Ok(tree.len())
    }

    /// Get all values in a tree
    pub fn get_all<T: DeserializeOwned>(&self, tree: &str) -> Result<Vec<T>> {
        let tree = self.db.open_tree(tree)?;
        let values: Vec<T> = tree
            .iter()
            .values()
            .filter_map(|v| v.ok())
            .filter_map(|v| serde_json::from_slice(&v).ok())
            .collect();
        Ok(values)
    }

    /// Clear all data in a tree
    pub fn clear_tree(&self, tree: &str) -> Result<()> {
        let tree = self.db.open_tree(tree)?;
        tree.clear()?;
        // Removed flush() here as well
        Ok(())
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<()> {
        self.db.flush()?;
        Ok(())
    }

    /// Get database size info
    pub fn size_info(&self) -> (u64, u64) {
        (self.db.size_on_disk().unwrap_or(0), self.db.len() as u64)
    }
}

impl Clone for MemoryStore {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_store_operations() {
        let dir = tempdir().unwrap();
        let store = MemoryStore::open(dir.path().join("test.db")).unwrap();

        // Test set/get
        store.set("test", "key1", &"value1".to_string()).unwrap();
        let value: Option<String> = store.get("test", "key1").unwrap();
        assert_eq!(value, Some("value1".to_string()));

        // Test delete
        store.delete("test", "key1").unwrap();
        let value: Option<String> = store.get("test", "key1").unwrap();
        assert_eq!(value, None);
    }
}
