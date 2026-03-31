use crate::tool::SharedTool;

/// Information about a missing tool request.
#[derive(Debug, Clone)]
pub struct MissingToolInfo {
    /// The name of the tool that was requested.
    pub requested_name: String,

    /// Similar tool names that exist in the registry.
    pub suggestions: Vec<String>,

    /// Timestamp of the request.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl MissingToolInfo {
    /// Create a new missing tool info.
    #[must_use]
    pub fn new(requested_name: impl Into<String>) -> Self {
        Self {
            requested_name: requested_name.into(),
            suggestions: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Add a suggestion.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }
}

/// Tool lookup result that may include suggestions.
#[derive(Clone)]
pub enum ToolLookupResult {
    /// Tool was found.
    Found(SharedTool),

    /// Tool was not found, with suggestions.
    NotFound { requested: String, suggestions: Vec<String> },
}

impl std::fmt::Debug for ToolLookupResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Found(_) => f.debug_tuple("Found").field(&"<SharedTool>").finish(),
            Self::NotFound { requested, suggestions } => f
                .debug_struct("NotFound")
                .field("requested", requested)
                .field("suggestions", suggestions)
                .finish(),
        }
    }
}

impl ToolLookupResult {
    /// Get the tool if found, None otherwise.
    #[must_use]
    pub fn ok(self) -> Option<SharedTool> {
        match self {
            ToolLookupResult::Found(tool) => Some(tool),
            ToolLookupResult::NotFound { .. } => None,
        }
    }

    /// Check if the lookup was successful.
    #[must_use]
    pub fn is_found(&self) -> bool {
        matches!(self, ToolLookupResult::Found(_))
    }

    /// Check if the lookup failed.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, ToolLookupResult::NotFound { .. })
    }
}

/// Simple Levenshtein distance calculation for string similarity.
pub(crate) fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for (i, c1) in s1.chars().enumerate() {
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }

    matrix[len1][len2]
}

/// Find a tool by name, checking both primary names and aliases.
///
/// This helper can be used when searching through a collection of tools
/// that aren't necessarily in a registry.
pub fn find_tool_by_name<'a>(
    tools: &'a [SharedTool],
    name: &str,
) -> Option<&'a SharedTool> {
    let name_lower = name.to_lowercase();

    tools.iter().find(|tool| {
        let tool_name = tool.name().to_lowercase();
        if tool_name == name_lower {
            return true;
        }

        // Check aliases
        for alias in tool.aliases() {
            if alias.to_lowercase() == name_lower {
                return true;
            }
        }

        false
    })
}

/// Check if a tool's name matches the given name (case-insensitive).
///
/// This checks both the primary name and any aliases.
pub fn tool_matches_name(tool: &SharedTool, name: &str) -> bool {
    let name_lower = name.to_lowercase();

    if tool.name().to_lowercase() == name_lower {
        return true;
    }

    tool.aliases().iter().any(|alias| alias.to_lowercase() == name_lower)
}
