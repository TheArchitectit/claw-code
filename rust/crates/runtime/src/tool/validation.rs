//! Validation types and enums.

/// Information about whether an operation is a search or read operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOrReadInfo {
    /// True for search operations (grep, find, glob patterns).
    pub is_search: bool,

    /// True for read operations (cat, head, tail, file read).
    pub is_read: bool,

    /// True for directory-listing operations (ls, tree, du).
    pub is_list: bool,
}

/// Interrupt behavior for tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterruptBehavior {
    /// Stop the tool and discard its result.
    Cancel,

    /// Keep running; the new message waits.
    #[default]
    Block,
}

impl InterruptBehavior {
    /// Check if this is Cancel.
    #[must_use]
    pub fn is_cancel(&self) -> bool {
        matches!(self, Self::Cancel)
    }

    /// Check if this is Block.
    #[must_use]
    pub fn is_block(&self) -> bool {
        matches!(self, Self::Block)
    }
}

/// MCP tool information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpInfo {
    /// The server name.
    pub server_name: String,

    /// The tool name as received from the MCP server.
    pub tool_name: String,
}

/// Maximum result size configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaxResultSize {
    /// A specific character limit.
    Limit(usize),

    /// No limit (Infinity in TypeScript).
    Unlimited,
}

impl MaxResultSize {
    /// Get the limit as an option (None for unlimited).
    #[must_use]
    pub fn as_option(&self) -> Option<usize> {
        match self {
            Self::Limit(n) => Some(*n),
            Self::Unlimited => None,
        }
    }

    /// Check if the given size exceeds the limit.
    #[must_use]
    pub fn exceeds_limit(&self, size: usize) -> bool {
        match self {
            Self::Limit(n) => size > *n,
            Self::Unlimited => false,
        }
    }
}

/// Result of validating tool input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// The input is valid.
    Valid,

    /// The input is invalid with a message and error code.
    Invalid { message: String, error_code: i32 },
}

impl ValidationResult {
    /// Check if the result is valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Check if the result is invalid.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid { .. })
    }

    /// Create an invalid result.
    #[must_use]
    pub fn invalid(message: impl Into<String>, error_code: i32) -> Self {
        Self::Invalid {
            message: message.into(),
            error_code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result() {
        assert!(ValidationResult::Valid.is_valid());
        assert!(!ValidationResult::Valid.is_invalid());

        let invalid = ValidationResult::invalid("test", 42);
        assert!(!invalid.is_valid());
        assert!(invalid.is_invalid());
    }

    #[test]
    fn test_max_result_size() {
        let limit = MaxResultSize::Limit(100);
        assert!(limit.exceeds_limit(101));
        assert!(!limit.exceeds_limit(100));
        assert!(!limit.exceeds_limit(99));

        let unlimited = MaxResultSize::Unlimited;
        assert!(!unlimited.exceeds_limit(usize::MAX));
    }
}
