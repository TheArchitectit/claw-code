//! Tool trait definition and implementations.

use std::sync::Arc;

use async_trait::async_trait;

use crate::context::ToolUseContext;
use crate::messages::ProgressData;
use crate::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::{JsonSchema, ToolUseId};

use super::output::ToolOutput;
use super::validation::{InterruptBehavior, MaxResultSize, McpInfo, SearchOrReadInfo, ValidationResult};

/// A boxed tool trait object.
pub type BoxedTool = Box<dyn Tool + Send + Sync>;

/// A shared (reference-counted) tool trait object.
pub type SharedTool = Arc<dyn Tool + Send + Sync>;

/// This trait defines the interface between the query engine and tools.
/// Each tool has:
/// - A unique name and optional aliases
/// - Input/output schemas for validation
/// - Async execution with progress reporting
/// - Permission checking
/// - Various metadata methods for the UI
#[async_trait]
pub trait Tool: Send + Sync {
    /// The unique name of this tool.
    fn name(&self) -> &str;

    /// Optional aliases for backwards compatibility when a tool is renamed.
    fn aliases(&self) -> &[String] {
        &[]
    }

    /// One-line capability phrase used by ToolSearch for keyword matching.
    /// Helps the model find this tool via keyword search when it's deferred.
    /// 3–10 words, no trailing period.
    fn search_hint(&self) -> Option<&str> {
        None
    }

    /// Execute the tool with the given input and context.
    ///
    /// # Arguments
    /// * `input` - The parsed tool input as JSON
    /// * `context` - The tool use context
    /// * `can_use_tool` - A callback to check if the tool can be used
    /// * `tool_use_id` - The unique ID for this tool use
    /// * `on_progress` - Optional callback for progress updates
    ///
    /// # Returns
    /// The tool result containing output data and optional new messages
    async fn execute(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        tool_use_id: ToolUseId,
        on_progress: Option<Box<dyn Fn(ProgressData) + Send>>,
    ) -> crate::types::ToolResult<ToolOutput>;

    /// Get a description of what this tool will do with the given input.
    ///
    /// This is used for permission prompts and logging.
    async fn describe(&self, input: &serde_json::Value) -> String;

    /// Get the JSON schema for this tool's input.
    fn input_schema(&self) -> JsonSchema;

    /// Get the optional JSON schema for this tool's output.
    fn output_schema(&self) -> Option<JsonSchema> {
        None
    }

    /// Check if this tool is enabled.
    fn is_enabled(&self) -> bool {
        true
    }

    /// Check if this tool is concurrency-safe for the given input.
    ///
    /// Returns false by default (assume not safe).
    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Check if this tool is read-only for the given input.
    ///
    /// Returns false by default (assume writes).
    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Check if this tool performs destructive operations.
    ///
    /// Only returns true for tools that perform irreversible operations
    /// (delete, overwrite, send).
    fn is_destructive(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Get the interrupt behavior for this tool.
    ///
    /// - `'cancel'` — stop the tool and discard its result
    /// - `'block'`  — keep running; the new message waits
    ///
    /// Defaults to `'block'`.
    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Block
    }

    /// Check if this tool use is a search or read operation.
    ///
    /// Returns information about whether the operation is a search or read
    /// operation for UI collapsing purposes.
    fn is_search_or_read_command(
        &self,
        _input: &serde_json::Value,
    ) -> Option<SearchOrReadInfo> {
        None
    }

    /// Check if this tool operates on "open world" data.
    ///
    /// Open world tools interact with external systems whose state may change.
    fn is_open_world(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Check if this tool requires user interaction.
    fn requires_user_interaction(&self) -> bool {
        false
    }

    /// Check if this is an MCP tool.
    fn is_mcp(&self) -> bool {
        false
    }

    /// Check if this is an LSP tool.
    fn is_lsp(&self) -> bool {
        false
    }

    /// Check if this tool should be deferred.
    ///
    /// When true, this tool is deferred and requires ToolSearch to be used
    /// before it can be called.
    fn should_defer(&self) -> bool {
        false
    }

    /// Check if this tool should always be loaded.
    ///
    /// When true, this tool is never deferred — its full schema appears in the
    /// initial prompt even when ToolSearch is enabled.
    fn always_load(&self) -> bool {
        false
    }

    /// Get MCP server info if this is an MCP tool.
    fn mcp_info(&self) -> Option<McpInfo> {
        None
    }

    /// Maximum size in characters for tool result before it gets persisted to disk.
    ///
    /// When exceeded, the result is saved to a file and a preview is returned.
    fn max_result_size_chars(&self) -> MaxResultSize;

    /// Check if strict mode should be enabled for this tool.
    fn strict(&self) -> bool {
        false
    }

    /// Validate the tool input before execution.
    ///
    /// Returns a validation result indicating if the input is valid.
    async fn validate_input(
        &self,
        _input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        ValidationResult::Valid
    }

    /// Check permissions for this tool use.
    ///
    /// This is called after validate_input passes.
    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        context: &ToolUseContext,
    ) -> PermissionResult;

    /// Get the user-facing name for this tool.
    ///
    /// Defaults to the tool's name.
    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        self.name().to_string()
    }

    /// Get a summary of this tool use for display.
    fn tool_use_summary(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        None
    }

    /// Get a human-readable activity description for this tool.
    ///
    /// Example: "Reading src/foo.ts", "Running bun test"
    fn activity_description(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        None
    }

    /// Get compact representation for the auto-mode classifier.
    ///
    /// Examples: `ls -la` for Bash, `/tmp/x: new content` for Edit.
    fn to_auto_classifier_input(&self, _input: &serde_json::Value) -> String {
        String::new()
    }

    /// Check if two inputs are equivalent.
    ///
    /// Used for caching and deduplication.
    fn inputs_equivalent(&self, a: &serde_json::Value, b: &serde_json::Value) -> bool {
        a == b
    }

    /// Get the path this tool operates on, if applicable.
    fn get_path(&self, _input: &serde_json::Value) -> Option<&str> {
        None
    }

    /// Prepare a matcher for hook `if` conditions.
    ///
    /// Called once per hook-input pair; returns a closure that matches
    /// against hook patterns.
    async fn prepare_permission_matcher(
        &self,
        _input: &serde_json::Value,
    ) -> Option<Box<dyn Fn(&str) -> bool + Send>> {
        None
    }

    /// Get the system prompt for this tool.
    ///
    /// This text is included in the system prompt sent to the LLM.
    async fn prompt(
        &self,
        tools: &[BoxedTool],
        permission_context: &ToolPermissionContext,
    ) -> String;
}

/// Implement Tool for Box<dyn Tool> to allow dereferencing.
#[async_trait]
impl Tool for Box<dyn Tool + Send + Sync> {
    fn name(&self) -> &str {
        self.as_ref().name()
    }

    fn aliases(&self) -> &[String] {
        self.as_ref().aliases()
    }

    fn search_hint(&self) -> Option<&str> {
        self.as_ref().search_hint()
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        tool_use_id: ToolUseId,
        on_progress: Option<Box<dyn Fn(ProgressData) + Send>>,
    ) -> crate::types::ToolResult<ToolOutput> {
        self.as_ref().execute(input, context, tool_use_id, on_progress).await
    }

    async fn describe(&self, input: &serde_json::Value) -> String {
        self.as_ref().describe(input).await
    }

    fn input_schema(&self) -> JsonSchema {
        self.as_ref().input_schema()
    }

    fn output_schema(&self) -> Option<JsonSchema> {
        self.as_ref().output_schema()
    }

    fn is_enabled(&self) -> bool {
        self.as_ref().is_enabled()
    }

    fn is_concurrency_safe(&self, input: &serde_json::Value) -> bool {
        self.as_ref().is_concurrency_safe(input)
    }

    fn is_read_only(&self, input: &serde_json::Value) -> bool {
        self.as_ref().is_read_only(input)
    }

    fn is_destructive(&self, input: &serde_json::Value) -> bool {
        self.as_ref().is_destructive(input)
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        self.as_ref().interrupt_behavior()
    }

    fn is_search_or_read_command(
        &self,
        input: &serde_json::Value,
    ) -> Option<SearchOrReadInfo> {
        self.as_ref().is_search_or_read_command(input)
    }

    fn is_open_world(&self, input: &serde_json::Value) -> bool {
        self.as_ref().is_open_world(input)
    }

    fn requires_user_interaction(&self) -> bool {
        self.as_ref().requires_user_interaction()
    }

    fn is_mcp(&self) -> bool {
        self.as_ref().is_mcp()
    }

    fn is_lsp(&self) -> bool {
        self.as_ref().is_lsp()
    }

    fn should_defer(&self) -> bool {
        self.as_ref().should_defer()
    }

    fn always_load(&self) -> bool {
        self.as_ref().always_load()
    }

    fn mcp_info(&self) -> Option<McpInfo> {
        self.as_ref().mcp_info()
    }

    fn max_result_size_chars(&self) -> MaxResultSize {
        self.as_ref().max_result_size_chars()
    }

    fn strict(&self) -> bool {
        self.as_ref().strict()
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        context: &ToolUseContext,
    ) -> ValidationResult {
        self.as_ref().validate_input(input, context).await
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        context: &ToolUseContext,
    ) -> PermissionResult {
        self.as_ref().check_permissions(input, context).await
    }

    fn user_facing_name(&self, input: Option<&serde_json::Value>) -> String {
        self.as_ref().user_facing_name(input)
    }

    fn tool_use_summary(&self, input: Option<&serde_json::Value>) -> Option<String> {
        self.as_ref().tool_use_summary(input)
    }

    fn activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        self.as_ref().activity_description(input)
    }

    fn to_auto_classifier_input(&self, input: &serde_json::Value) -> String {
        self.as_ref().to_auto_classifier_input(input)
    }

    fn inputs_equivalent(&self, a: &serde_json::Value, b: &serde_json::Value) -> bool {
        self.as_ref().inputs_equivalent(a, b)
    }

    fn get_path(&self, input: &serde_json::Value) -> Option<&str> {
        self.as_ref().get_path(input)
    }

    async fn prepare_permission_matcher(
        &self,
        input: &serde_json::Value,
    ) -> Option<Box<dyn Fn(&str) -> bool + Send>> {
        self.as_ref().prepare_permission_matcher(input).await
    }

    async fn prompt(
        &self,
        tools: &[BoxedTool],
        permission_context: &ToolPermissionContext,
    ) -> String {
        self.as_ref().prompt(tools, permission_context).await
    }
}
