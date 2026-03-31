//! Permission storage and persistence.
//!
//! The [`PermissionStore`] trait provides an abstraction over different
//! storage backends for permission decisions.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during permission persistence.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum PersistenceError {
    /// The storage is not available.
    #[error("Storage unavailable: {message}")]
    Unavailable { message: String },

    /// Failed to read from storage.
    #[error("Read error: {message}")]
    ReadError { message: String },

    /// Failed to write to storage.
    #[error("Write error: {message}")]
    WriteError { message: String },

    /// The stored data is corrupted.
    #[error("Data corruption: {message}")]
    Corruption { message: String },

    /// The requested record was not found.
    #[error("Record not found")]
    NotFound,
}

/// Result type for persistence operations.
pub type PersistenceResult<T> = Result<T, PersistenceError>;

/// A persisted permission record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRecord {
    /// The name of the tool.
    pub tool_name: String,
    /// The session ID this record belongs to.
    pub session_id: String,
    /// Whether the tool is allowed.
    pub allowed: bool,
    /// When this record was created.
    pub created_at: DateTime<Utc>,
    /// When this record expires (None for no expiry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

impl PermissionRecord {
    /// Create a new permission record.
    #[must_use]
    pub fn new(
        tool_name: impl Into<String>,
        session_id: impl Into<String>,
        allowed: bool,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            session_id: session_id.into(),
            allowed,
            created_at: Utc::now(),
            expires_at: None,
        }
    }

    /// Set an expiry time for this record.
    #[must_use]
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Check if this record has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Get the age of this record in seconds.
    #[must_use]
    pub fn age_seconds(&self) -> i64 {
        Utc::now().signed_duration_since(self.created_at).num_seconds()
    }
}

/// Trait for permission storage backends.
#[async_trait]
pub trait PermissionStore: Send + Sync {
    /// Get a permission record for a tool and session.
    ///
    /// Returns `Ok(None)` if no record exists.
    async fn get(&self, tool_name: &str, session_id: &str) -> PersistenceResult<Option<PermissionRecord>>;

    /// Store a permission record.
    async fn set(&self, record: PermissionRecord) -> PersistenceResult<()>;

    /// Remove a permission record.
    async fn remove(&self, tool_name: &str, session_id: &str) -> PersistenceResult<()>;

    /// Get all records for a session.
    async fn get_for_session(&self, session_id: &str) -> PersistenceResult<Vec<PermissionRecord>>;

    /// Clear all expired records.
    async fn clear_expired(&self) -> PersistenceResult<usize>;

    /// Clear all records for a session.
    async fn clear_for_session(&self, session_id: &str) -> PersistenceResult<()>;

    /// Get all records (for debugging/admin).
    async fn all_records(&self) -> PersistenceResult<Vec<PermissionRecord>>;
}

/// In-memory permission store.
pub struct MemoryPermissionStore {
    data: DashMap<(String, String), PermissionRecord>,
}

impl MemoryPermissionStore {
    /// Create a new empty memory store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
        }
    }
}

impl Default for MemoryPermissionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PermissionStore for MemoryPermissionStore {
    async fn get(&self, tool_name: &str, session_id: &str) -> PersistenceResult<Option<PermissionRecord>> {
        let key = (tool_name.to_string(), session_id.to_string());

        if let Some(entry) = self.data.get(&key) {
            let record = entry.value().clone();
            if record.is_expired() {
                drop(entry);
                self.data.remove(&key);
                return Ok(None);
            }
            return Ok(Some(record));
        }

        Ok(None)
    }

    async fn set(&self, record: PermissionRecord) -> PersistenceResult<()> {
        let key = (record.tool_name.clone(), record.session_id.clone());
        self.data.insert(key, record);
        Ok(())
    }

    async fn remove(&self, tool_name: &str, session_id: &str) -> PersistenceResult<()> {
        let key = (tool_name.to_string(), session_id.to_string());
        self.data.remove(&key);
        Ok(())
    }

    async fn get_for_session(&self, session_id: &str) -> PersistenceResult<Vec<PermissionRecord>> {
        let mut records = Vec::new();

        for entry in self.data.iter() {
            let record = entry.value();
            if record.session_id == session_id && !record.is_expired() {
                records.push(record.clone());
            }
        }

        Ok(records)
    }

    async fn clear_expired(&self) -> PersistenceResult<usize> {
        let mut count = 0;

        self.data.retain(|_, record| {
            if record.is_expired() {
                count += 1;
                false
            } else {
                true
            }
        });

        Ok(count)
    }

    async fn clear_for_session(&self, session_id: &str) -> PersistenceResult<()> {
        self.data.retain(|_, record| record.session_id != session_id);
        Ok(())
    }

    async fn all_records(&self) -> PersistenceResult<Vec<PermissionRecord>> {
        let records: Vec<PermissionRecord> = self.data
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        Ok(records)
    }
}

/// File-based permission store.
pub struct FilePermissionStore {
    path: PathBuf,
    cache: MemoryPermissionStore,
}

impl FilePermissionStore {
    /// Create a new file-based store.
    ///
    /// The store will persist records to the specified path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            cache: MemoryPermissionStore::new(),
        }
    }

    /// Get the storage path.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Load records from disk.
    async fn load(&self) -> PersistenceResult<()> {
        if !self.path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|e| PersistenceError::ReadError {
                message: e.to_string(),
            })?;

        let records: Vec<PermissionRecord> = serde_json::from_str(&content)
            .map_err(|e| PersistenceError::Corruption {
                message: e.to_string(),
            })?;

        for record in records {
            if !record.is_expired() {
                self.cache.set(record).await?;
            }
        }

        Ok(())
    }

    /// Save records to disk.
    async fn save(&self) -> PersistenceResult<()> {
        let records = self.cache.all_records().await?;

        let json = serde_json::to_string_pretty(&records)
            .map_err(|e| PersistenceError::WriteError {
                message: e.to_string(),
            })?;

        tokio::fs::write(&self.path, json)
            .await
            .map_err(|e| PersistenceError::WriteError {
                message: e.to_string(),
            })?;

        Ok(())
    }
}

#[async_trait]
impl PermissionStore for FilePermissionStore {
    async fn get(&self, tool_name: &str, session_id: &str) -> PersistenceResult<Option<PermissionRecord>> {
        self.cache.get(tool_name, session_id).await
    }

    async fn set(&self, record: PermissionRecord) -> PersistenceResult<()> {
        self.cache.set(record).await?;
        self.save().await?;
        Ok(())
    }

    async fn remove(&self, tool_name: &str, session_id: &str) -> PersistenceResult<()> {
        self.cache.remove(tool_name, session_id).await?;
        self.save().await?;
        Ok(())
    }

    async fn get_for_session(&self, session_id: &str) -> PersistenceResult<Vec<PermissionRecord>> {
        self.cache.get_for_session(session_id).await
    }

    async fn clear_expired(&self) -> PersistenceResult<usize> {
        let count = self.cache.clear_expired().await?;
        if count > 0 {
            self.save().await?;
        }
        Ok(count)
    }

    async fn clear_for_session(&self, session_id: &str) -> PersistenceResult<()> {
        self.cache.clear_for_session(session_id).await?;
        self.save().await?;
        Ok(())
    }

    async fn all_records(&self) -> PersistenceResult<Vec<PermissionRecord>> {
        self.cache.all_records().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_permission_record_new() {
        let record = PermissionRecord::new("BashTool", "session-1", true);
        assert_eq!(record.tool_name, "BashTool");
        assert_eq!(record.session_id, "session-1");
        assert!(record.allowed);
        assert!(!record.is_expired());
    }

    #[test]
    fn test_permission_record_with_expiry() {
        let expiry = Utc::now() + Duration::hours(1);
        let record = PermissionRecord::new("BashTool", "session-1", true)
            .with_expiry(expiry);

        assert!(!record.is_expired());
        assert!(record.expires_at.is_some());
    }

    #[test]
    fn test_permission_record_expired() {
        let expiry = Utc::now() - Duration::hours(1);
        let record = PermissionRecord::new("BashTool", "session-1", true)
            .with_expiry(expiry);

        assert!(record.is_expired());
    }

    #[test]
    fn test_permission_record_age() {
        let record = PermissionRecord::new("BashTool", "session-1", true);
        assert!(record.age_seconds() >= 0);
    }

    #[tokio::test]
    async fn test_memory_store_get_set() {
        let store = MemoryPermissionStore::new();
        let record = PermissionRecord::new("BashTool", "session-1", true);

        // Initially empty
        let result = store.get("BashTool", "session-1").await.unwrap();
        assert!(result.is_none());

        // Set the record
        store.set(record.clone()).await.unwrap();

        // Now it should exist
        let result = store.get("BashTool", "session-1").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().allowed, true);
    }

    #[tokio::test]
    async fn test_memory_store_remove() {
        let store = MemoryPermissionStore::new();
        let record = PermissionRecord::new("BashTool", "session-1", true);

        store.set(record).await.unwrap();
        store.remove("BashTool", "session-1").await.unwrap();

        let result = store.get("BashTool", "session-1").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_memory_store_get_for_session() {
        let store = MemoryPermissionStore::new();

        store.set(PermissionRecord::new("Tool1", "session-1", true)).await.unwrap();
        store.set(PermissionRecord::new("Tool2", "session-1", true)).await.unwrap();
        store.set(PermissionRecord::new("Tool3", "session-2", true)).await.unwrap();

        let records = store.get_for_session("session-1").await.unwrap();
        assert_eq!(records.len(), 2);
    }

    #[tokio::test]
    async fn test_memory_store_clear_expired() {
        let store = MemoryPermissionStore::new();
        let expiry = Utc::now() - Duration::hours(1);

        store.set(PermissionRecord::new("Expired", "session-1", true)
            .with_expiry(expiry)).await.unwrap();
        store.set(PermissionRecord::new("Valid", "session-1", true)).await.unwrap();

        let count = store.clear_expired().await.unwrap();
        assert_eq!(count, 1);

        let records = store.all_records().await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tool_name, "Valid");
    }

    #[tokio::test]
    async fn test_memory_store_clear_for_session() {
        let store = MemoryPermissionStore::new();

        store.set(PermissionRecord::new("Tool1", "session-1", true)).await.unwrap();
        store.set(PermissionRecord::new("Tool2", "session-2", true)).await.unwrap();

        store.clear_for_session("session-1").await.unwrap();

        let records = store.all_records().await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].session_id, "session-2");
    }

    #[tokio::test]
    async fn test_memory_store_get_expired() {
        let store = MemoryPermissionStore::new();
        let expiry = Utc::now() - Duration::hours(1);

        store.set(PermissionRecord::new("Expired", "session-1", true)
            .with_expiry(expiry)).await.unwrap();

        // Expired records should not be returned
        let result = store.get("Expired", "session-1").await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_persistence_error_display() {
        let err = PersistenceError::NotFound;
        assert!(err.to_string().contains("not found"));
    }
}
