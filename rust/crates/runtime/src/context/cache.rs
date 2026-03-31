//! File state cache and tracking for the tool use context.

use std::collections::HashMap;
use std::path::PathBuf;

/// File state cache entry.
#[derive(Debug, Clone, Default)]
pub struct FileStateCache {
    /// Cached file states.
    pub files: HashMap<PathBuf, FileState>,
}

impl FileStateCache {
    /// Create a new empty file state cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Get a file state by path.
    #[must_use]
    pub fn get(&self, path: impl AsRef<std::path::Path>) -> Option<&FileState> {
        self.files.get(path.as_ref())
    }

    /// Add a file to the cache.
    ///
    /// Records the file with its content hash and current timestamp.
    pub fn add_file(&mut self, path: impl Into<PathBuf>, content_hash: impl Into<String>) {
        let path = path.into();
        let file_state = FileState {
            content_hash: content_hash.into(),
            last_read: chrono::Utc::now(),
            content: None,
            source: FileSource::Read,
        };
        self.files.insert(path, file_state);
    }

    /// Check if a file has been read.
    ///
    /// Returns true if the file exists in the cache.
    #[must_use]
    pub fn is_file_read(&self, path: impl AsRef<std::path::Path>) -> bool {
        self.files.contains_key(path.as_ref())
    }

    /// Get all files that have been read.
    ///
    /// Returns a vector of all paths in the cache.
    #[must_use]
    pub fn get_read_files(&self) -> Vec<PathBuf> {
        self.files.keys().cloned().collect()
    }

    /// Add a glob result to the cache.
    ///
    /// Records a file discovered via glob without content hash.
    pub fn add_glob_result(&mut self, path: PathBuf, pattern: String) {
        let file_state = FileState {
            content_hash: String::new(), // No hash for glob results
            last_read: chrono::Utc::now(),
            content: None,
            source: FileSource::Glob { pattern },
        };
        self.files.insert(path, file_state);
    }

    /// Get files discovered by a specific glob pattern.
    ///
    /// Returns all files that were discovered by the given pattern.
    #[must_use]
    pub fn get_glob_results(&self, pattern: &str) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|(_, state)| matches!(state.source, FileSource::Glob { pattern: ref p } if p == pattern))
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Clear all cached file states.
    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// Get the number of files in the cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }
}

/// State of a cached file.
#[derive(Debug, Clone)]
pub struct FileState {
    /// The file content hash.
    pub content_hash: String,

    /// The last read timestamp.
    pub last_read: chrono::DateTime<chrono::Utc>,

    /// The file content (if cached).
    pub content: Option<String>,

    /// How this file was discovered.
    pub source: FileSource,
}

/// Source of a cached file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSource {
    /// File was read directly.
    Read,
    /// File was discovered via glob pattern.
    Glob {
        /// The glob pattern that discovered this file.
        pattern: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_state_cache() {
        let cache = FileStateCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        use std::path::PathBuf;
        let mut cache = FileStateCache::new();
        cache.add_file(PathBuf::from("/test"), "hash123");

        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);
        assert!(cache.is_file_read("/test"));
        assert!(!cache.is_file_read("/other"));

        let files = cache.get_read_files();
        assert_eq!(files.len(), 1);
        assert!(files.contains(&PathBuf::from("/test")));

        let state = cache.get(&PathBuf::from("/test")).unwrap();
        assert_eq!(state.content_hash, "hash123");
        assert!(matches!(state.source, FileSource::Read));
    }

    #[test]
    fn test_file_state_cache_glob_operations() {
        let mut cache = FileStateCache::new();

        // Add glob results
        cache.add_glob_result(PathBuf::from("/src/main.rs"), "**/*.rs".to_string());
        cache.add_glob_result(PathBuf::from("/src/lib.rs"), "**/*.rs".to_string());
        cache.add_glob_result(PathBuf::from("/tests/test.rs"), "**/test*.rs".to_string());

        assert_eq!(cache.len(), 3);

        // Get results by pattern
        let rs_files = cache.get_glob_results("**/*.rs");
        assert_eq!(rs_files.len(), 2);

        let test_files = cache.get_glob_results("**/test*.rs");
        assert_eq!(test_files.len(), 1);

        // Check source is correct
        let state = cache.get(&PathBuf::from("/src/main.rs")).unwrap();
        assert!(matches!(&state.source, FileSource::Glob { pattern } if pattern == "**/*.rs"));
    }

    #[test]
    fn test_file_state_cache_clear() {
        let mut cache = FileStateCache::new();
        cache.add_file(PathBuf::from("/test1"), "hash1");
        cache.add_file(PathBuf::from("/test2"), "hash2");

        assert_eq!(cache.len(), 2);

        cache.clear();

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }
}
