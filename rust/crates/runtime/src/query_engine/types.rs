//! Types for the query engine module.

use crate::messages::{NormalizedMessage, QueryResultMessage, UserMessage};
use crate::types::{QueryEngineError, QueryResult, SessionId, Usage as RuntimeUsage};

/// The result of a conversation turn.
#[derive(Debug, Clone)]
pub struct ConversationResult {
    /// The final assistant response text.
    pub response: String,
    /// All messages in the conversation turn.
    pub messages: Vec<NormalizedMessage>,
    /// Duration of the conversation in milliseconds.
    pub duration_ms: u64,
    /// Token usage for this conversation.
    pub usage: RuntimeUsage,
    /// Estimated cost in USD.
    pub cost_usd: f64,
    /// The session ID.
    pub session_id: SessionId,
}

/// Options for submitting a message.
#[derive(Debug, Clone, Default)]
pub struct SubmitMessageOptions {
    /// Optional UUID for the message.
    pub uuid: Option<crate::types::MessageId>,

    /// Whether this is a meta message.
    pub is_meta: bool,

    /// Whether to force a specific permission decision.
    pub force_decision: Option<crate::permissions::PermissionResult>,
}

/// Internal state for query execution.
pub(crate) enum QueryExecutionState {
    /// Initial state, ready to start.
    Initial,

    /// Processing the query.
    Processing,

    /// Completed with a result.
    Completed(QueryResultMessage),

    /// Failed with an error.
    #[allow(dead_code)]
    Failed(QueryEngineError),
}

/// A stream of messages from a query execution.
pub struct MessageStream {
    pub(crate) receiver: tokio::sync::mpsc::Receiver<NormalizedMessage>,
}

impl MessageStream {
    /// Receive the next message from the stream.
    pub async fn next(&mut self) -> Option<NormalizedMessage> {
        self.receiver.recv().await
    }
}
