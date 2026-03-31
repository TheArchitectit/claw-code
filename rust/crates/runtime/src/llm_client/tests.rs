//! Tests for the mock LLM client.

use crate::llm_client::client::LlmClient;
use crate::llm_client::mock::{MockLlmClient, MockResponse};
use crate::llm_client::stream::StopReason;
use crate::llm_client::types::LlmRequest;
use crate::messages::ContentBlock;
use crate::types::Usage;

#[tokio::test]
async fn test_mock_llm_simple_response() {
    let client = MockLlmClient::simple_text_response("Hello, world!");
    let request = LlmRequest::new("test-model");

    let response = client.complete(request).await.unwrap();
    assert_eq!(response.stop_reason, StopReason::EndTurn);

    let text = match &response.content[0] {
        ContentBlock::Text { text, .. } => text.clone(),
        _ => panic!("Expected text block"),
    };
    assert_eq!(text, "Hello, world!");
}

#[tokio::test]
async fn test_mock_llm_streaming() {
    let client = MockLlmClient::simple_text_response("Test");
    let request = LlmRequest::new("test-model");

    let mut stream = client.stream(request).await.unwrap();
    let chunks = stream.collect().await;

    assert!(!chunks.is_empty());
}

#[tokio::test]
async fn test_mock_llm_tool_call() {
    let client = MockLlmClient::with_tool_call(
        "test_tool",
        serde_json::json!({"arg": "value"}),
    );
    let request = LlmRequest::new("test-model");

    let response = client.complete(request).await.unwrap();
    assert_eq!(response.stop_reason, StopReason::ToolUse);

    match &response.content[0] {
        ContentBlock::ToolUse { name, input, .. } => {
            assert_eq!(name, "test_tool");
            assert_eq!(input["arg"], "value");
        }
        _ => panic!("Expected tool use block"),
    }
}

#[tokio::test]
async fn test_mock_llm_records_requests() {
    let client = MockLlmClient::simple_text_response("Response");
    let request1 = LlmRequest::new("model-1").with_system("system prompt");
    let request2 = LlmRequest::new("model-2").with_max_tokens(100);

    let _ = client.complete(request1.clone()).await.unwrap();
    let _ = client.complete(request2.clone()).await.unwrap();

    let recorded = client.get_recorded_requests();
    assert_eq!(recorded.len(), 2);
    assert_eq!(recorded[0].model, "model-1");
    assert_eq!(recorded[1].model, "model-2");
    assert_eq!(client.request_count(), 2);
}

#[tokio::test]
async fn test_mock_llm_multiple_responses() {
    let responses = vec![
        crate::llm_client::types::LlmResponse {
            content: vec![ContentBlock::Text {
                text: "First".to_string(),
                citation: None,
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
            model: "mock".to_string(),
        },
        crate::llm_client::types::LlmResponse {
            content: vec![ContentBlock::Text {
                text: "Second".to_string(),
                citation: None,
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
            model: "mock".to_string(),
        },
    ];

    let client = MockLlmClient::with_responses(responses);
    let request = LlmRequest::new("test-model");

    let resp1 = client.complete(request.clone()).await.unwrap();
    let resp2 = client.complete(request.clone()).await.unwrap();
    let resp3 = client.complete(request).await.unwrap(); // Should cycle back

    assert_eq!(resp1.content[0].as_text().unwrap(), "First");
    assert_eq!(resp2.content[0].as_text().unwrap(), "Second");
    assert_eq!(resp3.content[0].as_text().unwrap(), "First"); // Cycles back
}

#[tokio::test]
async fn test_mock_llm_error_response() {
    let client = MockLlmClient::with_error_response("Something went wrong");
    let request = LlmRequest::new("test-model");

    let response = client.complete(request).await.unwrap();
    assert_eq!(response.stop_reason, StopReason::Error);
}

#[tokio::test]
async fn test_mock_llm_clear_and_reset() {
    let client = MockLlmClient::simple_text_response("Test");
    let request = LlmRequest::new("test-model");

    let _ = client.complete(request.clone()).await.unwrap();
    let _ = client.complete(request.clone()).await.unwrap();

    assert_eq!(client.request_count(), 2);

    client.clear_recorded_requests();
    assert_eq!(client.request_count(), 0);

    client.reset_response_index();
    // After reset, response index should be back to 0
    // (we can verify by checking responses cycle correctly)
}

#[test]
fn test_mock_response_text() {
    let response = MockResponse::text("Hello, world!");
    assert_eq!(response.content.len(), 1);
    assert!(matches!(response.content[0], ContentBlock::Text { .. }));
    assert_eq!(response.stop_reason, StopReason::EndTurn);
}

#[test]
fn test_mock_response_tool_call() {
    let input = serde_json::json!({"arg": "value"});
    let response = MockResponse::tool_call("test_tool", input.clone());
    assert_eq!(response.content.len(), 1);
    assert!(matches!(response.content[0], ContentBlock::ToolUse { .. }));
    assert_eq!(response.stop_reason, StopReason::ToolUse);
}

#[test]
fn test_mock_response_with_thinking() {
    let response = MockResponse::with_thinking("Answer", "Thinking process");
    assert_eq!(response.content.len(), 2);
    assert!(matches!(response.content[0], ContentBlock::Thinking { .. }));
    assert!(matches!(response.content[1], ContentBlock::Text { .. }));
}

#[test]
fn test_mock_response_error() {
    let response = MockResponse::error("Something went wrong");
    assert!(response.error.is_some());
    assert_eq!(response.stop_reason, StopReason::Error);
}

#[test]
fn test_mock_response_with_usage() {
    let usage = Usage::new(100, 50, 25, 10);
    let response = MockResponse::text("Test").with_usage(usage);
    assert_eq!(response.usage.input_tokens, 100);
    assert_eq!(response.usage.output_tokens, 50);
}

#[test]
fn test_mock_response_with_delay() {
    let response = MockResponse::text("Test").with_delay(100);
    assert_eq!(response.delay_ms, 100);
}

#[test]
fn test_mock_response_to_llm_response() {
    let mock = MockResponse::text("Test content");
    let llm_response = mock.to_llm_response("test-model");

    assert_eq!(llm_response.model, "test-model");
    assert_eq!(llm_response.content.len(), 1);
    assert_eq!(llm_response.stop_reason, StopReason::EndTurn);
}

#[tokio::test]
async fn test_mock_llm_with_mock_responses() {
    let mock_responses = vec![
        MockResponse::text("First response"),
        MockResponse::text("Second response"),
    ];

    let client = MockLlmClient::with_mock_responses(mock_responses);
    let request = LlmRequest::new("test-model");

    let resp1 = client.complete(request.clone()).await.unwrap();
    let resp2 = client.complete(request.clone()).await.unwrap();

    assert_eq!(resp1.content[0].as_text().unwrap(), "First response");
    assert_eq!(resp2.content[0].as_text().unwrap(), "Second response");
}

#[tokio::test]
async fn test_mock_llm_record_request_manually() {
    let client = MockLlmClient::simple_text_response("Test");
    let request = LlmRequest::new("manual-model");

    client.record_request_manually(request);

    let recorded = client.get_recorded_requests();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].model, "manual-model");
}
