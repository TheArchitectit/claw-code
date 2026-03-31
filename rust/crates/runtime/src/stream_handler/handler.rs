//! Stream handler implementation.

use crate::llm_client::LlmStreamChunk;
use crate::messages::ContentBlock;
use crate::types::{ToolUseId, Usage as RuntimeUsage};

use super::events::StreamEvent;
use super::state::PartialToolUse;

/// Handler for streaming LLM responses.
///
/// Accumulates streaming chunks into complete content blocks and emits
/// events for UI consumption.
#[derive(Debug, Clone)]
pub struct StreamHandler {
    /// Accumulated text content blocks
    accumulated_text: Vec<ContentBlock>,
    /// Current partial text being accumulated (before finalizing into a block)
    current_text: Option<String>,
    /// Current partial thinking content
    current_thinking: Option<(String, Option<String>)>, // (thinking, signature)
    /// Current partial tool use being accumulated
    current_tool_use: Option<PartialToolUse>,
    /// Accumulated usage statistics
    usage: RuntimeUsage,
    /// Events emitted during processing (for UI updates)
    events: Vec<StreamEvent>,
}

impl StreamHandler {
    /// Create a new stream handler with empty state
    pub fn new() -> Self {
        Self {
            accumulated_text: Vec::new(),
            current_text: None,
            current_thinking: None,
            current_tool_use: None,
            usage: RuntimeUsage::default(),
            events: Vec::new(),
        }
    }

    /// Process a single chunk from the LLM stream
    pub fn process_chunk(&mut self, chunk: LlmStreamChunk) -> Option<StreamEvent> {
        match chunk {
            LlmStreamChunk::Text { text } => {
                // Accumulate text content
                if let Some(ref mut current) = self.current_text {
                    current.push_str(&text);
                } else {
                    // If there was previous thinking content, finalize it first
                    self.finalize_thinking();
                    self.current_text = Some(text.clone());
                }

                let event = StreamEvent::TextDelta { text };
                self.events.push(event.clone());
                Some(event)
            }
            LlmStreamChunk::Thinking { thinking } => {
                // Accumulate thinking content
                if let Some(ref mut current) = self.current_thinking {
                    current.0.push_str(&thinking);
                } else {
                    // If there was previous text content, finalize it first
                    self.finalize_text();
                    self.current_thinking = Some((thinking.clone(), None));
                }

                let event = StreamEvent::ThinkingDelta { thinking };
                self.events.push(event.clone());
                Some(event)
            }
            LlmStreamChunk::ToolUseStart { id, name } => {
                // Finalize any pending content before starting a tool use
                self.finalize_text();
                self.finalize_thinking();

                // Start a new tool use
                self.current_tool_use = Some(PartialToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    partial_json: String::new(),
                });

                let event = StreamEvent::ToolUseDelta {
                    id: id.to_string(),
                    name,
                    partial_json: String::new(),
                };
                self.events.push(event.clone());
                Some(event)
            }
            LlmStreamChunk::ToolUseDelta {
                id: _,
                partial_json,
            } => {
                // Accumulate JSON for the current tool use
                if let Some(ref mut pending) = self.current_tool_use {
                    pending.partial_json.push_str(&partial_json);

                    let event = StreamEvent::ToolUseDelta {
                        id: pending.id.to_string(),
                        name: pending.name.clone(),
                        partial_json: pending.partial_json.clone(),
                    };
                    self.events.push(event.clone());
                    Some(event)
                } else {
                    None
                }
            }
            LlmStreamChunk::ToolUseComplete { id, input } => {
                // Complete the tool use with parsed input
                self.finalize_tool_use_with_input(id, input)
            }
            LlmStreamChunk::Usage { usage } => {
                self.usage = usage;

                let event = StreamEvent::UsageDelta {
                    input_tokens: self.usage.input_tokens,
                    output_tokens: self.usage.output_tokens,
                };
                self.events.push(event.clone());
                Some(event)
            }
            LlmStreamChunk::Stop { reason } => {
                let event = StreamEvent::StopEvent {
                    stop_reason: reason.to_string(),
                };
                self.events.push(event.clone());
                Some(event)
            }
            LlmStreamChunk::Error { message: _ } => {
                // Errors are handled by the caller, no event needed
                None
            }
        }
    }

    /// Get current accumulated content blocks (including partial state)
    pub fn current_content(&self) -> Vec<ContentBlock> {
        let mut blocks = self.accumulated_text.clone();

        // Include current partial text
        if let Some(ref text) = self.current_text {
            blocks.push(ContentBlock::Text {
                text: text.clone(),
                citation: None,
            });
        }

        // Include current partial thinking
        if let Some((ref thinking, ref signature)) = self.current_thinking {
            blocks.push(ContentBlock::Thinking {
                thinking: thinking.clone(),
                signature: signature.clone(),
            });
        }

        // Include current partial tool use
        if let Some(ref tool_use) = self.current_tool_use {
            // Try to parse accumulated JSON
            let input = if tool_use.partial_json.is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&tool_use.partial_json)
                    .unwrap_or_else(|_| serde_json::json!({ "raw": tool_use.partial_json }))
            };

            blocks.push(ContentBlock::ToolUse {
                id: tool_use.id.clone(),
                name: tool_use.name.clone(),
                input,
            });
        }

        blocks
    }

    /// Complete the stream and return final content blocks
    pub fn complete(mut self) -> Vec<ContentBlock> {
        // Finalize any pending content
        self.finalize_text();
        self.finalize_thinking();
        self.finalize_incomplete_tool_use();

        self.accumulated_text
    }

    /// Get accumulated usage statistics
    pub fn usage(&self) -> RuntimeUsage {
        self.usage
    }

    /// Get all events emitted during processing
    pub fn events(&self) -> &[StreamEvent] {
        &self.events
    }

    // Private helper methods

    fn finalize_text(&mut self) {
        if let Some(text) = self.current_text.take() {
            self.accumulated_text.push(ContentBlock::Text {
                text,
                citation: None,
            });
        }
    }

    fn finalize_thinking(&mut self) {
        if let Some((thinking, signature)) = self.current_thinking.take() {
            self.accumulated_text.push(ContentBlock::Thinking {
                thinking,
                signature,
            });
        }
    }

    fn finalize_tool_use_with_input(
        &mut self,
        id: ToolUseId,
        input: serde_json::Value,
    ) -> Option<StreamEvent> {
        // If we have accumulated JSON but the stream gives us parsed input, use the parsed input
        if let Some(tool_use) = self.current_tool_use.take() {
            self.accumulated_text.push(ContentBlock::ToolUse {
                id: tool_use.id,
                name: tool_use.name.clone(),
                input: input.clone(),
            });

            let event = StreamEvent::ToolUseDelta {
                id: id.to_string(),
                name: tool_use.name,
                partial_json: input.to_string(),
            };
            self.events.push(event.clone());
            Some(event)
        } else {
            // Stream provided complete info directly
            self.accumulated_text.push(ContentBlock::ToolUse {
                id,
                name: String::new(),
                input,
            });
            None
        }
    }

    fn finalize_incomplete_tool_use(&mut self) {
        if let Some(tool_use) = self.current_tool_use.take() {
            // Try to parse the accumulated JSON
            let input = if tool_use.partial_json.is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&tool_use.partial_json).unwrap_or_else(|_| {
                    // If parsing fails, wrap the raw string
                    serde_json::json!({ "raw": tool_use.partial_json })
                })
            };

            self.accumulated_text.push(ContentBlock::ToolUse {
                id: tool_use.id,
                name: tool_use.name,
                input,
            });
        }
    }
}

impl Default for StreamHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::events::StreamEvent;
    use super::super::StreamHandler;
    use crate::llm_client::{LlmStreamChunk, StopReason};
    use crate::messages::ContentBlock;
    use crate::types::{ToolUseId, Usage as RuntimeUsage};

    #[test]
    fn test_stream_handler_new() {
        let handler = StreamHandler::new();
        assert!(handler.current_content().is_empty());
        assert_eq!(handler.usage().input_tokens, 0);
        assert!(handler.events().is_empty());
    }

    #[test]
    fn test_process_text_chunk() {
        let mut handler = StreamHandler::new();

        let event = handler.process_chunk(LlmStreamChunk::Text {
            text: "Hello".to_string(),
        });

        assert!(matches!(event, Some(StreamEvent::TextDelta { text }) if text == "Hello"));

        let event2 = handler.process_chunk(LlmStreamChunk::Text {
            text: " World".to_string(),
        });

        assert!(matches!(event2, Some(StreamEvent::TextDelta { text }) if text == " World"));

        let content = handler.current_content();
        assert_eq!(content.len(), 1);
        assert!(matches!(&content[0], ContentBlock::Text { text, .. } if text == "Hello World"));
    }

    #[test]
    fn test_process_thinking_chunk() {
        let mut handler = StreamHandler::new();

        let event = handler.process_chunk(LlmStreamChunk::Thinking {
            thinking: "Thinking...".to_string(),
        });

        assert!(
            matches!(event, Some(StreamEvent::ThinkingDelta { thinking }) if thinking == "Thinking...")
        );

        let content = handler.current_content();
        assert_eq!(content.len(), 1);
        assert!(
            matches!(&content[0], ContentBlock::Thinking { thinking, .. } if thinking == "Thinking...")
        );
    }

    #[test]
    fn test_process_tool_use() {
        let mut handler = StreamHandler::new();
        let tool_id = ToolUseId::generate();

        // Start tool use
        let event = handler.process_chunk(LlmStreamChunk::ToolUseStart {
            id: tool_id.clone(),
            name: "test_tool".to_string(),
        });
        assert!(event.is_some());

        // Add partial JSON
        let event2 = handler.process_chunk(LlmStreamChunk::ToolUseDelta {
            id: tool_id.clone(),
            partial_json: r#"{"key": "val"}"#.to_string(),
        });
        assert!(
            matches!(event2, Some(StreamEvent::ToolUseDelta { id, name, .. }) if id == tool_id.to_string() && name == "test_tool")
        );

        // Complete tool use
        let event3 = handler.process_chunk(LlmStreamChunk::ToolUseComplete {
            id: tool_id.clone(),
            input: serde_json::json!({"key": "value"}),
        });
        assert!(event3.is_some());

        // Verify content
        let content = handler.complete();
        assert_eq!(content.len(), 1);
        assert!(
            matches!(&content[0], ContentBlock::ToolUse { id, name, .. } if id == &tool_id && name == "test_tool")
        );
    }

    #[test]
    fn test_process_usage() {
        let mut handler = StreamHandler::new();

        let event = handler.process_chunk(LlmStreamChunk::Usage {
            usage: RuntimeUsage {
                input_tokens: 10,
                output_tokens: 20,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
        });

        assert!(matches!(
            event,
            Some(StreamEvent::UsageDelta {
                input_tokens: 10,
                output_tokens: 20
            })
        ));

        assert_eq!(handler.usage().input_tokens, 10);
        assert_eq!(handler.usage().output_tokens, 20);
    }

    #[test]
    fn test_process_stop() {
        let mut handler = StreamHandler::new();

        let event = handler.process_chunk(LlmStreamChunk::Stop {
            reason: StopReason::EndTurn,
        });

        assert!(matches!(
            event,
            Some(StreamEvent::StopEvent { stop_reason }) if stop_reason == "end_turn"
        ));
    }

    #[test]
    fn test_finalize_incomplete_tool_use() {
        let mut handler = StreamHandler::new();
        let tool_id = ToolUseId::generate();

        // Start tool use but don't complete it
        handler.process_chunk(LlmStreamChunk::ToolUseStart {
            id: tool_id.clone(),
            name: "test_tool".to_string(),
        });

        handler.process_chunk(LlmStreamChunk::ToolUseDelta {
            id: tool_id.clone(),
            partial_json: r#"{"incomplete""#.to_string(),
        });

        // Complete handler - should handle malformed JSON gracefully
        let content = handler.complete();
        assert_eq!(content.len(), 1);

        // Verify malformed JSON is wrapped in raw field
        match &content[0] {
            ContentBlock::ToolUse { input, .. } => {
                assert!(input.get("raw").is_some());
            }
            _ => panic!("Expected ToolUse block"),
        }
    }

    #[test]
    fn test_text_and_thinking_separation() {
        let mut handler = StreamHandler::new();

        // Add text
        handler.process_chunk(LlmStreamChunk::Text {
            text: "First text".to_string(),
        });

        // Add thinking - should finalize text first
        handler.process_chunk(LlmStreamChunk::Thinking {
            thinking: "Some thinking".to_string(),
        });

        // Add more text - should finalize thinking first
        handler.process_chunk(LlmStreamChunk::Text {
            text: "Second text".to_string(),
        });

        // Complete and verify both are present
        let content = handler.complete();
        assert_eq!(content.len(), 3);

        assert!(matches!(&content[0], ContentBlock::Text { text, .. } if text == "First text"));
        assert!(
            matches!(&content[1], ContentBlock::Thinking { thinking, .. } if thinking == "Some thinking")
        );
        assert!(matches!(&content[2], ContentBlock::Text { text, .. } if text == "Second text"));
    }

    #[test]
    fn test_default_implementation() {
        let handler: StreamHandler = Default::default();
        assert!(handler.current_content().is_empty());
    }
}
