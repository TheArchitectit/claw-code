//! Team coordination module for multi-agent workflows.
//!
//! This module provides the core infrastructure for coordinating multiple
//! agents working together on tasks. Key features:
//!
//! - **Task claiming**: Atomic task acquisition to prevent duplicate work
//! - **Team inbox**: Progress reporting from agents to team coordinator
//! - **Mode expansion**: Preset configurations for different team sizes
//!
//! ## Multi-Agent Architecture
//!
//! Teams are created with a set of agents that work in parallel. Each agent
//! can claim tasks to prevent duplicate work. Progress is reported through
//! the team inbox system for coordination.

use std::path::PathBuf;

use runtime::TurnProgressReporter;
use serde_json::{json, Value};

// --- Directory Management ---

/// Get the agent mailbox directory for inter-agent communication.
pub fn agent_mailbox_dir() -> PathBuf {
    if let Ok(path) = std::env::var("CLAWD_AGENT_STORE") {
        return PathBuf::from(path).join("mailbox");
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    if let Some(workspace_root) = cwd.ancestors().nth(2) {
        return workspace_root.join(".clawd-agents").join("mailbox");
    }
    cwd.join(".clawd-agents").join("mailbox")
}

/// Get the claims directory for task locking.
pub fn claims_dir() -> PathBuf {
    if let Ok(path) = std::env::var("CLAWD_AGENT_STORE") {
        return PathBuf::from(path).join("claims");
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    if let Some(workspace_root) = cwd.ancestors().nth(2) {
        return workspace_root.join(".clawd-agents").join("claims");
    }
    cwd.join(".clawd-agents").join("claims")
}

// --- Task Claiming ---

/// Atomically claim a task for an agent within a team.
///
/// Returns `true` if the claim was successful, `false` if already claimed.
/// Uses atomic rename to prevent race conditions between agents.
pub fn claim_task(task_id: &str, agent_id: &str, team_id: &str) -> Result<bool, String> {
    let dir = claims_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let lock_path = dir.join(format!("{task_id}.lock"));
    if lock_path.exists() {
        return Ok(false);
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let entry = json!({
        "task_id": task_id,
        "agent_id": agent_id,
        "team_id": team_id,
        "claimed_at": ts,
    });
    // Atomic claim: write to temp file then rename
    let tmp_path = dir.join(format!("{task_id}.lock.tmp.{agent_id}"));
    std::fs::write(&tmp_path, serde_json::to_string_pretty(&entry).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    match std::fs::rename(&tmp_path, &lock_path) {
        Ok(()) => Ok(true),
        Err(_) => {
            // Another agent claimed it first
            let _ = std::fs::remove_file(&tmp_path);
            Ok(false)
        }
    }
}

/// Release a task claim.
pub fn release_claim(task_id: &str) -> Result<(), String> {
    let lock_path = claims_dir().join(format!("{task_id}.lock"));
    if lock_path.exists() {
        std::fs::remove_file(&lock_path).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

/// List all claims, optionally filtered by team.
pub fn list_claims(team_id: Option<&str>) -> Vec<Value> {
    let dir = claims_dir();
    let mut claims = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "lock") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(v) = serde_json::from_str::<Value>(&content) {
                        if team_id.map_or(true, |tid| v.get("team_id").map_or(false, |t| t == tid)) {
                            claims.push(v);
                        }
                    }
                }
            }
        }
    }
    claims
}

// --- Team Inbox Reporter ---

/// Progress reporter that writes to team inbox for coordination.
///
/// Used by agents to report their progress back to the team coordinator.
/// Enables real-time monitoring of agent activity and progress tracking.
pub struct TeamInboxReporter {
    team_id: String,
    agent_id: String,
    agent_name: String,
    inbox_dir: PathBuf,
}

impl TeamInboxReporter {
    pub fn new(team_id: String, agent_id: String, agent_name: String) -> Self {
        let inbox_dir = agent_mailbox_dir().join("team").join(&team_id);
        let _ = std::fs::create_dir_all(&inbox_dir);
        Self { team_id, agent_id, agent_name, inbox_dir }
    }
}

impl TurnProgressReporter for TeamInboxReporter {
    fn on_tool_result(
        &self,
        iteration: usize,
        max_iterations: usize,
        tool_name: &str,
        input: &str,
        result: Result<&str, &str>,
    ) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let (result_preview, is_error) = match result {
            Ok(output) => (output.chars().take(500).collect::<String>(), false),
            Err(err) => (err.chars().take(500).collect::<String>(), true),
        };
        let input_preview: String = input.chars().take(300).collect();
        let entry = serde_json::json!({
            "event": "tool_progress",
            "agent_id": self.agent_id,
            "name": self.agent_name,
            "tool_name": tool_name,
            "input_preview": input_preview,
            "result_preview": result_preview,
            "is_error": is_error,
            "iteration": iteration,
            "max_iterations": max_iterations,
            "timestamp": ts,
        });
        let msg_file = self.inbox_dir.join(format!(
            "tp-{}-{}-{ts}.json",
            self.agent_id, iteration
        ));
        if let Ok(line) = serde_json::to_string(&entry) {
            let _ = std::fs::write(&msg_file, line);
        }

        // Periodic git commit (every 5 tool calls) to preserve progress
        if iteration > 0 && iteration % 5 == 0 {
            let _ = std::process::Command::new("git")
                .args(["add", "-A"])
                .output();
            let diff_check = std::process::Command::new("git")
                .args(["diff", "--cached", "--quiet"])
                .output();
            if diff_check.map_or(true, |o| !o.status.success()) {
                let _ = std::process::Command::new("git")
                    .args(["commit", "-m", &format!("agent {} progress: iteration {iteration}", self.agent_id)])
                    .output();
            }
        }

        // Check for kill signal from team lead
        for entry in std::fs::read_dir(&self.inbox_dir).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()) {
            if let Ok(e) = entry {
                let name = e.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with(&format!("kill-{}-", self.agent_id)) {
                    // Kill signal received — panic to abort
                    std::fs::remove_file(e.path()).ok();
                    panic!("agent {} received kill signal", self.agent_id);
                }
            }
        }
    }
}

// --- Team Mode Expansion ---

/// Expand a mode preset into a list of agent tasks.
///
/// Mode presets define common team configurations:
/// - "1x" / "tiny": 1x scaling (3 roles + reviewers)
/// - "2x" / "small": 2x scaling
/// - "3x" / "medium": 3x scaling
/// - "4x" / "large": 4x scaling
/// - "5x" / "xlarge": 5x scaling
/// - "6x" / "mega": 6x scaling
///
/// Each mode creates agents for Explore, Plan, and Verification roles,
/// plus Reviewer agents (1 per 3 builders, minimum 1).
pub fn expand_team_mode(mode: &str, base_prompt: &str, team_id: &str) -> Result<Vec<Value>, String> {
    let n = match mode {
        "1x" | "tiny" => 1,
        "2x" | "small" => 2,
        "3x" | "medium" => 3,
        "4x" | "large" => 4,
        "5x" | "xlarge" => 5,
        "6x" | "mega" => 6,
        other => return Err(format!("unknown team mode '{other}'. Use 1x-6x or tiny/small/medium/large/xlarge/mega")),
    };
    let short_team_id = &team_id[team_id.len().saturating_sub(8)..];
    let roles: &[&str] = &["Explore", "Plan", "Verification"];
    let mut tasks = Vec::new();
    for role in roles {
        for i in 0..n {
            let prompt = format!("[{role} agent {}/{}] {base_prompt}", i + 1, n);
            let description = format!("{role} agent {}/{}", i + 1, n);
            let task_id = format!("{short_team_id}-{role}-{i}");
            tasks.push(json!({
                "prompt": prompt,
                "description": description,
                "subagent_type": role,
                "task_id": task_id,
            }));
        }
    }
    // Add read-only Reviewer agents (1 per 3 builders, minimum 1)
    let reviewer_count = std::cmp::max(1, (roles.len() * n) / 3);
    for i in 0..reviewer_count {
        let prompt = format!("[Reviewer {}/{}] Review the work of other agents. Read their output files, check code quality, identify issues, and report findings via AgentMessage. Only use read-only tools.", i + 1, reviewer_count);
        let description = format!("Reviewer {}/{}", i + 1, reviewer_count);
        let task_id = format!("{short_team_id}-Reviewer-{i}");
        tasks.push(json!({
            "prompt": prompt,
            "description": description,
            "subagent_type": "Reviewer",
            "task_id": task_id,
        }));
    }
    Ok(tasks)
}

// --- Team Event Logging ---

/// Append an event to the team event log.
pub fn append_team_event(
    events_path: &std::path::Path,
    team_id: &str,
    agent_id: &str,
    event_type: &str,
    name: &str,
    detail: Option<&str>,
) {
    let entry = json!({
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        "team_id": team_id,
        "agent_id": agent_id,
        "event": event_type,
        "name": name,
        "detail": detail,
    });
    if let Ok(line) = serde_json::to_string(&entry) {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(events_path)
        {
            let _ = std::writeln!(file, "{line}");
        }
    }
}
