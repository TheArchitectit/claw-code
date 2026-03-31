//! Events emitted by the stream handler for UI consumption.

/// Events emitted by the stream handler for UI consumption
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// Text content delta
    TextDelta { text: String },
    /// Tool use delta (partial JSON)
    ToolUseDelta {
        id: String,
        name: String,
        partial_json: String,
    },
    /// Thinking content delta
    ThinkingDelta { thinking: String },
    /// Usage statistics update
    UsageDelta {
        input_tokens: u32,
        output_tokens: u32,
    },
    /// Stream stop event
    StopEvent { stop_reason: String },
}
