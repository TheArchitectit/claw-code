//! Anthropic API client implementation.

use async_trait::async_trait;
use futures::StreamExt;

use crate::anthropic::config::AnthropicConfig;
use crate::llm_client::{LlmClient, LlmRequest, LlmResponse, LlmStream, LlmStreamChunk, StopReason};
use crate::messages::{ContentBlock, Message};
use crate::types::{QueryResult, ToolUseId, Usage};

use crate::anthropic::AnthropicClient;

impl AnthropicClient {
    /// Convert runtime messages to Anthropic format.
    pub(crate) fn convert_messages(&self, messages: &[Message]) -> Vec<serde_json::Value> {
        messages
            .iter()
            .filter_map(|msg| {
                let role = match msg {
                    Message::User(_) => "user",
                    Message::Assistant(_) => "assistant",
                    _ => return None,
                };

                match msg {
                    Message::User(u) => {
                        // Handle user message content - extract text from content blocks
                        let content_blocks: Vec<_> = u.content.content.iter().map(|block| {
                            match block {
                                ContentBlock::Text { text, .. } => {
                                    serde_json::json!({
                                        "type": "text",
                                        "text": text
                                    })
                                }
                                _ => serde_json::json!({
                                    "type": "text",
                                    "text": format!("{:?}", block)
                                })
                            }
                        }).collect();

                        Some(serde_json::json!({
                            "role": role,
                            "content": content_blocks
                        }))
                    }
                    Message::Assistant(a) => {
                        // Convert assistant content blocks from AssistantMessageContent
                        let blocks: Vec<_> = a.content.content.iter().map(|block| match block {
                            ContentBlock::Text { text, .. } => {
                                serde_json::json!({
                                    "type": "text",
                                    "text": text
                                })
                            }
                            ContentBlock::ToolUse { id, name, input } => {
                                serde_json::json!({
                                    "type": "tool_use",
                                    "id": &id.0,
                                    "name": name,
                                    "input": input
                                })
                            }
                            ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                                // Tool results go back as user messages in Anthropic
                                serde_json::json!({
                                    "type": "tool_result",
                                    "tool_use_id": &tool_use_id.0,
                                    "content": content,
                                    "is_error": is_error.unwrap_or(false)
                                })
                            }
                            _ => serde_json::json!({"type": "text", "text": ""}),
                        }).collect();

                        Some(serde_json::json!({
                            "role": role,
                            "content": blocks
                        }))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    /// Build the request body.
    pub(crate) fn build_request_body(&self, request: &LlmRequest) -> serde_json::Value {
        let model = if request.model.is_empty() {
            self.config.default_model.clone()
        } else {
            request.model.clone()
        };

        let messages = self.convert_messages(&request.messages);

        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
        });

        if let Some(system) = &request.system {
            body["system"] = serde_json::json!(system);
        }

        if let Some(temperature) = request.temperature {
            body["temperature"] = serde_json::json!(temperature);
        }

        if !request.tools.is_empty() {
            body["tools"] = serde_json::json!(request.tools);
        }

        body
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn stream(&self, request: LlmRequest) -> QueryResult<LlmStream> {
        let url = format!("{}/messages", self.config.api_base);
        let mut body = self.build_request_body(&request);
        body["stream"] = serde_json::json!(true);

        let response = self
            .http_client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::types::QueryEngineError::Api { message: e.to_string() })?;

        // Check for rate limiting
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after: u64 = response
                .headers()
                .get("retry-after")
                .and_then(|v: &reqwest::header::HeaderValue| v.to_str().ok())
                .and_then(|v: &str| v.parse().ok())
                .unwrap_or(60);

            return Err(crate::types::QueryEngineError::Api {
                message: format!("Rate limited. Retry after {retry_after}s")
            });
        }

        // Check for other errors
        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(crate::types::QueryEngineError::Api {
                message: format!("API error: {error_text}")
            });
        }

        // Create channel for streaming chunks
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn task to process stream
        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut current_tool_use: Option<(ToolUseId, String, String)> = None;
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let text = String::from_utf8_lossy(&chunk);
                        buffer.push_str(&text);

                        // Process SSE lines
                        for line in buffer.lines() {
                            if line.starts_with("data: ") {
                                let data = &line[6..];

                                if data == "[DONE]" {
                                    let _ = tx.send(LlmStreamChunk::Stop {
                                        reason: StopReason::EndTurn,
                                    }).await;
                                    return;
                                }

                                // Parse the event
                                match serde_json::from_str::<serde_json::Value>(data) {
                                    Ok(event) => {
                                        if let Some(event_type) = event.get("type").and_then(|v: &serde_json::Value| v.as_str()) {
                                            match event_type {
                                                "content_block_start" => {
                                                    if let Some(block) = event.get("content_block") {
                                                        if let Some(block_type) = block.get("type").and_then(|v: &serde_json::Value| v.as_str()) {
                                                            if block_type == "tool_use" {
                                                                let id = block.get("id").and_then(|v: &serde_json::Value| v.as_str())
                                                                    .map(|s: &str| ToolUseId::new(s))
                                                                    .unwrap_or_else(ToolUseId::generate);
                                                                let name = block.get("name").and_then(|v: &serde_json::Value| v.as_str())
                                                                    .unwrap_or("unknown").to_string();
                                                                current_tool_use = Some((id, name, String::new()));
                                                            }
                                                        }
                                                    }
                                                }
                                                "content_block_delta" => {
                                                    if let Some(delta) = event.get("delta") {
                                                        if let Some(text) = delta.get("text").and_then(|v: &serde_json::Value| v.as_str()) {
                                                            let _ = tx.send(LlmStreamChunk::Text {
                                                                text: text.to_string(),
                                                            }).await;
                                                        } else if let Some(partial_json) = delta.get("partial_json").and_then(|v: &serde_json::Value| v.as_str()) {
                                                            if let Some((ref id, ref _name, ref mut input)) = current_tool_use {
                                                                input.push_str(partial_json);
                                                                let _ = tx.send(LlmStreamChunk::ToolUseDelta {
                                                                    id: id.clone(),
                                                                    partial_json: partial_json.to_string(),
                                                                }).await;
                                                            }
                                                        }
                                                    }
                                                }
                                                "content_block_stop" => {
                                                    if let Some((id, _name, input)) = current_tool_use.take() {
                                                        let input_json = serde_json::from_str(&input)
                                                            .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
                                                        let _ = tx.send(LlmStreamChunk::ToolUseComplete {
                                                            id,
                                                            input: input_json,
                                                        }).await;
                                                    }
                                                }
                                                "message_stop" => {
                                                    if let Some(usage) = event.get("usage") {
                                                        let input_tokens: u32 = usage.get("input_tokens").and_then(|v: &serde_json::Value| v.as_u64()).unwrap_or(0) as u32;
                                                        let output_tokens: u32 = usage.get("output_tokens").and_then(|v: &serde_json::Value| v.as_u64()).unwrap_or(0) as u32;
                                                        let _ = tx.send(LlmStreamChunk::Usage {
                                                            usage: Usage::new(input_tokens, output_tokens, 0, 0),
                                                        }).await;
                                                    }
                                                    let _ = tx.send(LlmStreamChunk::Stop {
                                                        reason: StopReason::EndTurn,
                                                    }).await;
                                                    return;
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    Err(_) => {}
                                }
                            }
                        }

                        // Keep incomplete line in buffer
                        if !buffer.ends_with('\n') {
                            if let Some(last_newline) = buffer.rfind('\n') {
                                buffer = buffer[last_newline + 1..].to_string();
                            }
                        } else {
                            buffer.clear();
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(LlmStreamChunk::Error {
                            message: format!("Stream error: {e}"),
                        }).await;
                        return;
                    }
                }
            }
        });

        Ok(LlmStream::new(rx))
    }

    async fn complete(&self, request: LlmRequest) -> QueryResult<LlmResponse> {
        let url = format!("{}/messages", self.config.api_base);
        let body = self.build_request_body(&request);

        let response = self
            .http_client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::types::QueryEngineError::Api { message: e.to_string() })?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after: u64 = response
                .headers()
                .get("retry-after")
                .and_then(|v: &reqwest::header::HeaderValue| v.to_str().ok())
                .and_then(|v: &str| v.parse().ok())
                .unwrap_or(60);

            return Err(crate::types::QueryEngineError::Api {
                message: format!("Rate limited. Retry after {retry_after}s")
            });
        }

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(crate::types::QueryEngineError::Api {
                message: format!("API error: {error_text}")
            });
        }

        let api_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| crate::types::QueryEngineError::Api {
                message: format!("Failed to parse response: {e}")
            })?;

        // Extract content blocks
        let mut content = Vec::new();
        if let Some(content_array) = api_response.get("content").and_then(|v: &serde_json::Value| v.as_array()) {
            for block in content_array {
                if let Some(block_type) = block.get("type").and_then(|v: &serde_json::Value| v.as_str()) {
                    match block_type {
                        "text" => {
                            if let Some(text) = block.get("text").and_then(|v: &serde_json::Value| v.as_str()) {
                                content.push(ContentBlock::Text {
                                    text: text.to_string(),
                                    citation: None,
                                });
                            }
                        }
                        "tool_use" => {
                            let id = block.get("id").and_then(|v: &serde_json::Value| v.as_str())
                                .map(|s: &str| ToolUseId::new(s))
                                .unwrap_or_else(ToolUseId::generate);
                            let name = block.get("name").and_then(|v: &serde_json::Value| v.as_str())
                                .unwrap_or("unknown").to_string();
                            let input = block.get("input").cloned()
                                .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
                            content.push(ContentBlock::ToolUse { id, name, input });
                        }
                        _ => {}
                    }
                }
            }
        }

        // Extract stop reason
        let stop_reason = api_response
            .get("stop_reason")
            .and_then(|v: &serde_json::Value| v.as_str())
            .map(|s: &str| match s {
                "end_turn" => StopReason::EndTurn,
                "max_tokens" => StopReason::MaxTokens,
                "stop_sequence" => StopReason::StopSequence,
                "tool_use" => StopReason::ToolUse,
                _ => StopReason::EndTurn,
            })
            .unwrap_or(StopReason::EndTurn);

        // Extract usage
        let usage = if let Some(usage_obj) = api_response.get("usage") {
            let input_tokens: u32 = usage_obj.get("input_tokens").and_then(|v: &serde_json::Value| v.as_u64()).unwrap_or(0) as u32;
            let output_tokens: u32 = usage_obj.get("output_tokens").and_then(|v: &serde_json::Value| v.as_u64()).unwrap_or(0) as u32;
            let cache_read: u32 = usage_obj.get("cache_read_input_tokens").and_then(|v: &serde_json::Value| v.as_u64()).unwrap_or(0) as u32;
            let cache_creation: u32 = usage_obj.get("cache_creation_input_tokens").and_then(|v: &serde_json::Value| v.as_u64()).unwrap_or(0) as u32;
            Usage::new(input_tokens, output_tokens, cache_read, cache_creation)
        } else {
            Usage::default()
        };

        let model = api_response
            .get("model")
            .and_then(|v: &serde_json::Value| v.as_str())
            .unwrap_or(&self.config.default_model)
            .to_string();

        Ok(LlmResponse {
            content,
            stop_reason,
            usage,
            model,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use super::*;
    use crate::messages::{UserMessage, UserMessageContent, AssistantMessage, AssistantMessageContent};
    use crate::types::{MessageId, SessionId};

    fn create_test_client() -> AnthropicClient {
        let config = AnthropicConfig::new("test-api-key");
        AnthropicClient::new(config).unwrap()
    }

    fn create_test_user_message(text: &str) -> Message {
        Message::User(UserMessage {
            id: MessageId::new(),
            session_id: SessionId::new(),
            timestamp: Utc::now(),
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![ContentBlock::Text {
                    text: text.to_string(),
                    citation: None,
                }],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: None,
            parent_tool_use_id: None,
            is_replay: None,
            is_synthetic: None,
        })
    }

    fn create_test_assistant_message(content_blocks: Vec<ContentBlock>) -> Message {
        Message::Assistant(AssistantMessage {
            id: MessageId::new(),
            session_id: SessionId::new(),
            timestamp: Utc::now(),
            content: AssistantMessageContent {
                role: "assistant".to_string(),
                content: content_blocks,
                model: "claude-3-5-sonnet".to_string(),
                stop_reason: None,
                usage: None,
            },
            parent_tool_use_id: None,
        })
    }

    #[test]
    fn test_convert_messages_user() {
        let client = create_test_client();
        let messages = vec![create_test_user_message("Hello Claude")];

        let converted = client.convert_messages(&messages);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0]["role"], "user");
        assert_eq!(converted[0]["content"][0]["type"], "text");
        assert_eq!(converted[0]["content"][0]["text"], "Hello Claude");
    }

    #[test]
    fn test_convert_messages_assistant() {
        let client = create_test_client();
        let messages = vec![create_test_assistant_message(vec![ContentBlock::Text {
            text: "Hello!".to_string(),
            citation: None,
        }])];

        let converted = client.convert_messages(&messages);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0]["role"], "assistant");
        assert_eq!(converted[0]["content"][0]["type"], "text");
    }

    #[test]
    fn test_convert_messages_tool_use() {
        let client = create_test_client();
        let messages = vec![create_test_assistant_message(vec![ContentBlock::ToolUse {
            id: ToolUseId::new("tool-123"),
            name: "get_weather".to_string(),
            input: serde_json::json!({"city": "Paris"}),
        }])];

        let converted = client.convert_messages(&messages);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0]["content"][0]["type"], "tool_use");
        assert_eq!(converted[0]["content"][0]["id"], "tool-123");
        assert_eq!(converted[0]["content"][0]["name"], "get_weather");
    }

    #[test]
    fn test_build_request_body_basic() {
        let client = create_test_client();
        let request = LlmRequest::new("claude-3-5-sonnet")
            .with_messages(vec![create_test_user_message("Hello")]);

        let body = client.build_request_body(&request);

        assert_eq!(body["model"], "claude-3-5-sonnet");
        assert_eq!(body["max_tokens"], 4096);
        assert!(body["messages"].is_array());
    }

    #[test]
    fn test_build_request_body_with_system() {
        let client = create_test_client();
        let request = LlmRequest::new("claude-3-haiku")
            .with_system("You are a helpful assistant")
            .with_max_tokens(2048);

        let body = client.build_request_body(&request);

        assert_eq!(body["system"], "You are a helpful assistant");
        assert_eq!(body["max_tokens"], 2048);
    }

    #[test]
    fn test_build_request_body_with_tools() {
        let client = create_test_client();
        let tool = serde_json::json!({
            "name": "get_weather",
            "description": "Get weather for a city",
            "input_schema": {
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                }
            }
        });
        let request = LlmRequest::new("claude-3-5-sonnet")
            .with_tools(vec![tool.clone()]);

        let body = client.build_request_body(&request);

        assert!(body["tools"].is_array());
        assert_eq!(body["tools"][0]["name"], "get_weather");
    }

    #[test]
    fn test_build_request_body_uses_default_model() {
        let client = create_test_client();
        let request = LlmRequest::new("");  // Empty model

        let body = client.build_request_body(&request);

        assert_eq!(body["model"], "claude-sonnet-4-6");  // Default from config
    }
}
