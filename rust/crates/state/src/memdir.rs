//! Memory directory (memdir) implementation
//!
//! Provides:
//! - MEMORY.md management
//! - Memory file operations
//! - Auto-memory path management
//! - Memory entry parsing and validation

use crate::error::StateResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// Entrypoint filename
pub const ENTRYPOINT_NAME: &str = "MEMORY.md";

/// Maximum lines in entrypoint
pub const MAX_ENTRYPOINT_LINES: usize = 200;

/// Maximum bytes in entrypoint
pub const MAX_ENTRYPOINT_BYTES: usize = 25_000;

/// Memory type taxonomy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// Information about the user
    User,
    /// Feedback on behaviors to avoid or repeat
    Feedback,
    /// Project context (deadlines, constraints, decisions)
    Project,
    /// Pointers to external resources
    Reference,
}

impl MemoryType {
    /// Get the display name for this memory type
    pub fn display_name(&self) -> &'static str {
        match self {
            MemoryType::User => "User",
            MemoryType::Feedback => "Feedback",
            MemoryType::Project => "Project",
            MemoryType::Reference => "Reference",
        }
    }

    /// Get description for this memory type
    pub fn description(&self) -> &'static str {
        match self {
            MemoryType::User => "Information about the user: role, goals, preferences, work style",
            MemoryType::Feedback => "Corrections and confirmations: behaviors to avoid or repeat",
            MemoryType::Project => "Project context: deadlines, constraints, decisions, incidents",
            MemoryType::Reference => "Pointers to external resources: dashboards, docs, channels",
        }
    }
}

/// Memory entry frontmatter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub name: String,
    pub description: String,
    pub memory_type: MemoryType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// File path relative to memory directory
    pub file_path: PathBuf,
    /// Whether this memory is team-shared
    pub is_team: bool,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl MemoryEntry {
    /// Create a new memory entry
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        memory_type: MemoryType,
        file_path: impl Into<PathBuf>,
    ) -> Self {
        let now = Utc::now();
        Self {
            name: name.into(),
            description: description.into(),
            memory_type,
            created_at: now,
            updated_at: now,
            file_path: file_path.into(),
            is_team: false,
            metadata: HashMap::new(),
        }
    }

    /// Create frontmatter YAML string
    pub fn to_frontmatter(&self) -> String {
        format!(
            "---\nname: {}\ndescription: {}\ntype: {}\ncreated_at: {}\nupdated_at: {}\n---\n",
            self.name,
            self.description,
            serde_json::to_string(&self.memory_type).unwrap_or_default(),
            self.created_at.to_rfc3339(),
            self.updated_at.to_rfc3339()
        )
    }

    /// Mark as team memory
    pub fn set_team(&mut self, is_team: bool) {
        self.is_team = is_team;
        self.updated_at = Utc::now();
    }

    /// Update content (call when file is modified)
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

/// Entrypoint truncation result
#[derive(Debug, Clone)]
pub struct EntrypointTruncation {
    pub content: String,
    pub line_count: usize,
    pub byte_count: usize,
    pub was_line_truncated: bool,
    pub was_byte_truncated: bool,
}

/// Memory directory configuration
#[derive(Debug, Clone)]
pub struct MemDirConfig {
    pub memory_dir: PathBuf,
    pub entrypoint_name: String,
    pub max_lines: usize,
    pub max_bytes: usize,
    pub auto_create: bool,
}

impl MemDirConfig {
    /// Create default config for project directory
    pub fn new(project_dir: &Path) -> Self {
        Self {
            memory_dir: project_dir.join(".claude").join("memory"),
            entrypoint_name: ENTRYPOINT_NAME.to_string(),
            max_lines: MAX_ENTRYPOINT_LINES,
            max_bytes: MAX_ENTRYPOINT_BYTES,
            auto_create: true,
        }
    }

    /// Get the entrypoint path
    pub fn entrypoint_path(&self) -> PathBuf {
        self.memory_dir.join(&self.entrypoint_name)
    }
}

/// Memory directory manager
#[derive(Debug, Clone)]
pub struct MemDirManager {
    config: MemDirConfig,
    entries: Arc<RwLock<Vec<MemoryEntry>>>,
}

impl MemDirManager {
    /// Create a new memory directory manager
    pub async fn new(project_dir: &Path) -> StateResult<Self> {
        let config = MemDirConfig::new(project_dir);

        if config.auto_create {
            fs::create_dir_all(&config.memory_dir).await?;
        }

        Ok(Self {
            config,
            entries: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Get the memory directory path
    pub fn memory_dir(&self) -> &Path {
        &self.config.memory_dir
    }

    /// Get the entrypoint path
    pub fn entrypoint_path(&self) -> PathBuf {
        self.config.entrypoint_path()
    }

    /// Ensure the memory directory exists
    pub async fn ensure_exists(&self) -> StateResult<()> {
        fs::create_dir_all(&self.config.memory_dir).await?;
        Ok(())
    }

    /// Check if the entrypoint exists
    pub async fn entrypoint_exists(&self) -> bool {
        self.entrypoint_path().exists()
    }

    /// Read and optionally truncate the entrypoint content
    pub async fn read_entrypoint(&self) -> StateResult<EntrypointTruncation> {
        let path = self.entrypoint_path();

        if !path.exists() {
            return Ok(EntrypointTruncation {
                content: String::new(),
                line_count: 0,
                byte_count: 0,
                was_line_truncated: false,
                was_byte_truncated: false,
            });
        }

        let content = fs::read_to_string(&path).await?;
        Ok(self.truncate_entrypoint_content(&content))
    }

    /// Write entrypoint content
    pub async fn write_entrypoint(&self, content: impl AsRef<str>) -> StateResult<()> {
        let path = self.entrypoint_path();
        fs::write(path, content.as_ref()).await?;
        Ok(())
    }

    /// Truncate entrypoint content
    fn truncate_entrypoint_content(&self, raw: &str) -> EntrypointTruncation {
        let trimmed = raw.trim();
        let content_lines: Vec<_> = trimmed.lines().collect();
        let line_count = content_lines.len();
        let byte_count = trimmed.len();

        let was_line_truncated = line_count > self.config.max_lines;
        let was_byte_truncated = byte_count > self.config.max_bytes;

        if !was_line_truncated && !was_byte_truncated {
            return EntrypointTruncation {
                content: trimmed.to_string(),
                line_count,
                byte_count,
                was_line_truncated,
                was_byte_truncated,
            };
        }

        let mut truncated = if was_line_truncated {
            content_lines[..self.config.max_lines].join("\n")
        } else {
            trimmed.to_string()
        };

        if truncated.len() > self.config.max_bytes {
            let cut_at = truncated[..self.config.max_bytes].rfind('\n').unwrap_or(self.config.max_bytes);
            truncated.truncate(cut_at);
        }

        let reason = if was_byte_truncated && !was_line_truncated {
            format!(
                "{} bytes (limit: {} bytes) — index entries are too long",
                byte_count, self.config.max_bytes
            )
        } else if was_line_truncated && !was_byte_truncated {
            format!("{} lines (limit: {})", line_count, self.config.max_lines)
        } else {
            format!(
                "{} lines and {} bytes",
                line_count, byte_count
            )
        };

        let warning = format!(
            "\n\n> WARNING: {} is {}. Only part of it was loaded. Keep index entries to one line under ~200 chars; move detail into topic files.",
            ENTRYPOINT_NAME, reason
        );

        EntrypointTruncation {
            content: truncated + &warning,
            line_count,
            byte_count,
            was_line_truncated,
            was_byte_truncated,
        }
    }

    /// Scan for memory files and build entries
    pub async fn scan_entries(&self) -> StateResult<Vec<MemoryEntry>> {
        let mut entries = Vec::new();

        let mut dir_entries = fs::read_dir(&self.config.memory_dir).await?;
        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "md") {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if file_name != ENTRYPOINT_NAME {
                    if let Some(entry) = self.parse_memory_file(&path).await? {
                        entries.push(entry);
                    }
                }
            }
        }

        let mut stored = self.entries.write().await;
        *stored = entries.clone();

        Ok(entries)
    }

    /// Parse a memory file for frontmatter
    async fn parse_memory_file(&self, path: &Path) -> StateResult<Option<MemoryEntry>> {
        let content = fs::read_to_string(path).await?;

        // Simple frontmatter parsing
        if let Some(frontmatter_start) = content.find("---") {
            if let Some(frontmatter_end) = content[frontmatter_start + 3..].find("---") {
                let frontmatter = &content[frontmatter_start + 3..frontmatter_start + 3 + frontmatter_end];

                // Parse basic fields (simplified - full YAML parsing would be better)
                let mut name = None;
                let mut description = None;
                let mut memory_type = MemoryType::User;

                for line in frontmatter.lines() {
                    let parts: Vec<_> = line.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let key = parts[0].trim();
                        let value = parts[1].trim();

                        match key {
                            "name" => name = Some(value.to_string()),
                            "description" => description = Some(value.to_string()),
                            "type" => {
                                memory_type = match value {
                                    "user" => MemoryType::User,
                                    "feedback" => MemoryType::Feedback,
                                    "project" => MemoryType::Project,
                                    "reference" => MemoryType::Reference,
                                    _ => MemoryType::User,
                                }
                            }
                            _ => {}
                        }
                    }
                }

                let relative_path = path.strip_prefix(&self.config.memory_dir).unwrap_or(path);

                if let (Some(name), Some(description)) = (name, description) {
                    return Ok(Some(MemoryEntry::new(
                        name,
                        description,
                        memory_type,
                        relative_path,
                    )));
                }
            }
        }

        // No frontmatter - create entry from filename
        let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
        let relative_path = path.strip_prefix(&self.config.memory_dir).unwrap_or(path);

        Ok(Some(MemoryEntry::new(
            file_name,
            "(No description)",
            MemoryType::User,
            relative_path,
        )))
    }

    /// Write a memory file
    pub async fn write_memory(
        &self,
        entry: &MemoryEntry,
        content: impl AsRef<str>,
    ) -> StateResult<()> {
        let file_path = self.config.memory_dir.join(&entry.file_path);

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let full_content = entry.to_frontmatter() + content.as_ref();
        fs::write(file_path, full_content).await?;

        Ok(())
    }

    /// Read a memory file
    pub async fn read_memory(&self, file_path: impl AsRef<Path>) -> StateResult<String> {
        let path = self.config.memory_dir.join(file_path.as_ref());
        let content = fs::read_to_string(&path).await?;
        Ok(content)
    }

    /// Delete a memory file
    pub async fn delete_memory(&self, file_path: impl AsRef<Path>) -> StateResult<()> {
        let path = self.config.memory_dir.join(file_path.as_ref());
        fs::remove_file(path).await?;

        // Also remove from entries
        let mut entries = self.entries.write().await;
        entries.retain(|e| e.file_path != file_path.as_ref());

        Ok(())
    }

    /// Get all entries
    pub async fn get_entries(&self) -> StateResult<Vec<MemoryEntry>> {
        let entries = self.entries.read().await;
        Ok(entries.clone())
    }

    /// Get entries by type
    pub async fn get_entries_by_type(&self, memory_type: MemoryType) -> StateResult<Vec<MemoryEntry>> {
        let entries = self.entries.read().await;
        Ok(entries
            .iter()
            .filter(|e| e.memory_type == memory_type)
            .cloned()
            .collect())
    }

    /// Build the memory prompt lines for system prompt
    pub fn build_memory_lines(&self, display_name: &str, extra_guidelines: Option<&[String]>) -> Vec<String> {
        let mut lines = vec![
            format!("# {}", display_name),
            String::new(),
            format!("You have a persistent, file-based memory system at `{}`. This directory already exists — write to it directly with the Write tool (do not run mkdir or check for its existence).", self.config.memory_dir.display()),
            String::new(),
            "You should build up this memory system over time so that future conversations can have a complete picture of who the user is, how they'd like to collaborate with you, what behaviors to avoid or repeat, and the context behind the work the user gives you.".to_string(),
            String::new(),
            "If the user explicitly asks you to remember something, save it immediately as whichever type fits best. If they ask you to forget something, find and remove the relevant entry.".to_string(),
            String::new(),
            "## Memory Types".to_string(),
            String::new(),
        ];

        // Add type descriptions
        for mem_type in [MemoryType::User, MemoryType::Feedback, MemoryType::Project, MemoryType::Reference] {
            lines.push(format!("**{}**: {}", mem_type.display_name(), mem_type.description()));
            lines.push(String::new());
        }

        lines.push("## How to save memories".to_string());
        lines.push(String::new());
        lines.push("Saving a memory is a two-step process:".to_string());
        lines.push(String::new());
        lines.push("**Step 1** — write the memory to its own file (e.g., `user_role.md`, `feedback_testing.md`) using frontmatter format:".to_string());
        lines.push(String::new());
        lines.push("```yaml".to_string());
        lines.push("---".to_string());
        lines.push("name: Memory title".to_string());
        lines.push("description: One-line description".to_string());
        lines.push("type: user | feedback | project | reference".to_string());
        lines.push("---".to_string());
        lines.push("```".to_string());
        lines.push(String::new());
        lines.push(format!("**Step 2** — add a pointer to that file in `{}`. `{}` is an index, not a memory — each entry should be one line, under ~150 characters.", ENTRYPOINT_NAME, ENTRYPOINT_NAME));
        lines.push(String::new());
        lines.push(format!("- `{}` is always loaded into your conversation context — lines after {} will be truncated, so keep the index concise", ENTRYPOINT_NAME, self.config.max_lines));
        lines.push("- Keep the name, description, and type fields in memory files up-to-date with the content".to_string());
        lines.push("- Organize memory semantically by topic, not chronologically".to_string());
        lines.push("- Update or remove memories that turn out to be wrong or outdated".to_string());
        lines.push("- Do not write duplicate memories. First check if there is an existing memory you can update before writing a new one.".to_string());

        if let Some(guidelines) = extra_guidelines {
            lines.push(String::new());
            for guideline in guidelines {
                lines.push(guideline.clone());
            }
        }

        lines.push(String::new());
        lines.push("## Memory and other forms of persistence".to_string());
        lines.push(String::new());
        lines.push("Memory is one of several persistence mechanisms available to you. The distinction is often that memory can be recalled in future conversations and should not be used for persisting information that is only useful within the scope of the current conversation.".to_string());
        lines.push("- When to use or update a plan instead of memory: If you are about to start a non-trivial implementation task and would like to reach alignment with the user on your approach, you should use a Plan rather than saving this information to memory.".to_string());
        lines.push("- When to use or update tasks instead of memory: When you need to break your work in current conversation into discrete steps or keep track of your progress, use tasks instead of saving to memory. Tasks are great for persisting information about the work that needs to be done in the current conversation, but memory should be reserved for information that will be useful in future conversations.".to_string());

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_memory_type_display() {
        assert_eq!(MemoryType::User.display_name(), "User");
        assert_eq!(MemoryType::Feedback.display_name(), "Feedback");
        assert_eq!(MemoryType::Project.display_name(), "Project");
        assert_eq!(MemoryType::Reference.display_name(), "Reference");
    }

    #[test]
    fn test_entry_truncation() {
        let temp_dir = TempDir::new().unwrap();
        let config = MemDirConfig::new(temp_dir.path());
        let manager = MemDirManager {
            config,
            entries: Arc::new(RwLock::new(Vec::new())),
        };

        // Content under limits
        let content = "Line 1\nLine 2\nLine 3";
        let result = manager.truncate_entrypoint_content(content);
        assert!(!result.was_line_truncated);
        assert!(!result.was_byte_truncated);

        // Content over line limit
        let mut long_content = String::new();
        for i in 0..250 {
            long_content.push_str(&format!("Line {}\n", i));
        }
        let result = manager.truncate_entrypoint_content(&long_content);
        assert!(result.was_line_truncated);
    }

    #[tokio::test]
    async fn test_memory_write_read() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemDirManager::new(temp_dir.path()).await.unwrap();

        let entry = MemoryEntry::new(
            "Test Memory",
            "A test memory entry",
            MemoryType::User,
            "test.md",
        );

        manager.write_memory(&entry, "This is the memory content.").await.unwrap();

        let content = manager.read_memory("test.md").await.unwrap();
        assert!(content.contains("Test Memory"));
        assert!(content.contains("This is the memory content."));
    }

    #[tokio::test]
    async fn test_entrypoint_operations() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemDirManager::new(temp_dir.path()).await.unwrap();

        assert!(!manager.entrypoint_exists().await);

        manager.write_entrypoint("# Memory Index\n\n- [Test](test.md)").await.unwrap();

        assert!(manager.entrypoint_exists().await);

        let content = manager.read_entrypoint().await.unwrap();
        assert_eq!(content.line_count, 3);
    }
}
