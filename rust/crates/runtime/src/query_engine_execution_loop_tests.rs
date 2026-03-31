// =========================================================================
// Execution Loop Tests for QueryEngine
// =========================================================================
// These tests should be added to the tests module in query_engine.rs

#[tokio::test]
async fn test_conversation_loop_natural_completion() {
    // Test that a simple conversation completes naturally with EndTurn
    let registry = create_registry();
    let llm_client = Arc::new(MockLlmClient::simple_text_response("Hello! I can help you."));

    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .with_max_turns(10)
        .build();

    let mut execution = engine
        .submit_message("Hi there", None)
        .await
        .expect("Failed to submit message");

    let messages = execution.execute().await.expect("Execution failed");

    // Verify the conversation completed successfully
    assert!(execution.is_complete());
    let result = execution.get_result().expect("Should have a result");

    // Check that we got a success result
    match result {
        QueryResultMessage::Success { stop_reason, num_turns, .. } => {
            assert_eq!(stop_reason.as_deref(), Some("end_turn"));
            assert_eq!(*num_turns, 1); // Single turn for simple response
        }
        _ => panic!("Expected Success result"),
    }

    // Verify we have user message, assistant message, and result
    let assistant_count = messages.iter().filter(|m| matches!(m, NormalizedMessage::Assistant{..})).count();
    assert!(assistant_count >= 1, "Should have at least one assistant message");
}

#[tokio::test]
async fn test_conversation_loop_max_turns_enforced() {
    // Test that the conversation loop enforces max turns limit
    let registry = create_registry();

    // Create a mock that returns ToolUse to force multiple turns
    let tool_response = LlmResponse {
        content: vec![ContentBlock::ToolUse {
            id: ToolUseId::generate(),
            name: "stub".to_string(),
            input: serde_json::json!({}),
        }],
        stop_reason: StopReason::ToolUse,
        usage: RuntimeUsage::default(),
        model: "mock".to_string(),
    };

    let final_response = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "Final response".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::EndTurn,
        usage: RuntimeUsage::default(),
        model: "mock".to_string(),
    };

    let llm_client = Arc::new(MockLlmClient::new(vec![
        tool_response.clone(),
        tool_response.clone(),
        tool_response.clone(),
        tool_response,
        final_response,
    ]));

    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .with_max_turns(3) // Limit to 3 turns
        .build();

    let mut execution = engine
        .submit_message("Call a tool", None)
        .await
        .expect("Failed to submit message");

    let result = execution.execute().await;

    // Should fail with MaxTurnsExceeded
    assert!(result.is_err());
    match result.unwrap_err() {
        QueryEngineError::MaxTurnsExceeded { max_turns } => {
            assert_eq!(max_turns, 3);
        }
        other => panic!("Expected MaxTurnsExceeded, got {:?}", other),
    }
}

#[tokio::test]
async fn test_conversation_loop_budget_enforced() {
    // Test that the conversation loop enforces budget limit
    let registry = create_registry();

    // Create responses that simulate usage accumulating
    let response_with_usage = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "Response".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::EndTurn,
        usage: RuntimeUsage::new(1000, 500, 0, 0), // High usage per turn
        model: "mock".to_string(),
    };

    let llm_client = Arc::new(MockLlmClient::new(vec![
        response_with_usage.clone(),
        response_with_usage.clone(),
        response_with_usage.clone(),
    ]));

    // Set a low budget
    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .with_max_budget(0.001) // Very low budget ($0.001)
        .with_max_turns(100)
        .build();

    // Manually set a high cost to trigger budget exceeded
    {
        let mut cost = engine.total_cost.lock().await;
        *cost = 0.002; // Already over budget before starting
    }

    let mut execution = engine
        .submit_message("Test message", None)
        .await
        .expect("Failed to submit message");

    let result = execution.execute().await;

    // Should fail with MaxBudgetExceeded
    assert!(result.is_err());
    match result.unwrap_err() {
        QueryEngineError::MaxBudgetExceeded { max_budget } => {
            assert!((max_budget - 0.001).abs() < 0.0001);
        }
        other => panic!("Expected MaxBudgetExceeded, got {:?}", other),
    }
}

#[tokio::test]
async fn test_conversation_loop_multiple_turns_with_tools() {
    // Test a multi-turn conversation with tool calls
    let registry = create_registry();

    // First response: tool call
    let tool_response = LlmResponse {
        content: vec![ContentBlock::ToolUse {
            id: ToolUseId::generate(),
            name: "stub".to_string(),
            input: serde_json::json!({"query": "test"}),
        }],
        stop_reason: StopReason::ToolUse,
        usage: RuntimeUsage::new(50, 30, 0, 0),
        model: "mock".to_string(),
    };

    // Second response: final text
    let final_response = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "Tool result processed".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::EndTurn,
        usage: RuntimeUsage::new(80, 40, 0, 0),
        model: "mock".to_string(),
    };

    let llm_client = Arc::new(MockLlmClient::new(vec![
        tool_response,
        final_response,
    ]));

    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .with_max_turns(10)
        .build();

    let mut execution = engine
        .submit_message("Use the tool", None)
        .await
        .expect("Failed to submit message");

    let messages = execution.execute().await.expect("Execution failed");

    // Verify multi-turn completion
    assert!(execution.is_complete());
    let result = execution.get_result().expect("Should have result");

    match result {
        QueryResultMessage::Success { num_turns, .. } => {
            assert!(*num_turns >= 2, "Should have at least 2 turns (tool + response)");
        }
        _ => panic!("Expected Success result"),
    }

    // Verify conversation history includes tool interactions
    let messages_vec = engine.get_messages().await;
    let has_tool_use = messages_vec.iter().any(|m| {
        if let Message::Assistant(msg) = m {
            msg.content.content.iter().any(|block| matches!(block, ContentBlock::ToolUse { .. }))
        } else {
            false
        }
    });
    assert!(has_tool_use, "Should have a tool_use block in history");
}

#[tokio::test]
async fn test_conversation_loop_usage_accumulation() {
    // Test that token usage is properly accumulated across turns
    let registry = create_registry();

    let response1 = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "First response".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::EndTurn,
        usage: RuntimeUsage::new(100, 50, 10, 5),
        model: "mock".to_string(),
    };

    let response2 = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "Second response".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::EndTurn,
        usage: RuntimeUsage::new(80, 40, 8, 4),
        model: "mock".to_string(),
    };

    let llm_client = Arc::new(MockLlmClient::new(vec![response1, response2]));

    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .build();

    // First turn
    let mut execution1 = engine
        .submit_message("Message 1", None)
        .await
        .expect("Failed to submit first message");
    let _ = execution1.execute().await.expect("First execution failed");

    let usage_after_first = engine.total_usage().await;
    assert_eq!(usage_after_first.input_tokens, 100);
    assert_eq!(usage_after_first.output_tokens, 50);
    assert_eq!(usage_after_first.cache_read_input_tokens, 10);
    assert_eq!(usage_after_first.cache_creation_input_tokens, 5);

    // Second turn
    let mut execution2 = engine
        .submit_message("Message 2", None)
        .await
        .expect("Failed to submit second message");
    let _ = execution2.execute().await.expect("Second execution failed");

    let usage_after_second = engine.total_usage().await;
    assert_eq!(usage_after_second.input_tokens, 180); // 100 + 80
    assert_eq!(usage_after_second.output_tokens, 90);  // 50 + 40
    assert_eq!(usage_after_second.cache_read_input_tokens, 18); // 10 + 8
    assert_eq!(usage_after_second.cache_creation_input_tokens, 9); // 5 + 4
}

#[tokio::test]
async fn test_conversation_loop_handles_max_tokens() {
    // Test handling of MaxTokens stop reason
    let registry = create_registry();

    let max_tokens_response = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "Partial response".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::MaxTokens,
        usage: RuntimeUsage::new(1000, 4000, 0, 0), // Max tokens used
        model: "mock".to_string(),
    };

    let llm_client = Arc::new(MockLlmClient::new(vec![max_tokens_response]));

    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .build();

    let mut execution = engine
        .submit_message("Generate long text", None)
        .await
        .expect("Failed to submit message");

    let _ = execution.execute().await.expect("Execution should complete");

    // Verify max tokens was recorded
    let result = execution.get_result().expect("Should have result");
    match result {
        QueryResultMessage::Success { stop_reason, .. } => {
            assert_eq!(stop_reason.as_deref(), Some("max_tokens"));
        }
        _ => panic!("Expected Success result"),
    }
}

#[tokio::test]
async fn test_conversation_loop_permission_denials_tracked() {
    // Test that permission denials are tracked during execution
    let registry = create_registry();

    let tool_response = LlmResponse {
        content: vec![ContentBlock::ToolUse {
            id: ToolUseId::generate(),
            name: "stub".to_string(),
            input: serde_json::json!({}),
        }],
        stop_reason: StopReason::ToolUse,
        usage: RuntimeUsage::default(),
        model: "mock".to_string(),
    };

    let final_response = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "Done".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::EndTurn,
        usage: RuntimeUsage::default(),
        model: "mock".to_string(),
    };

    let llm_client = Arc::new(MockLlmClient::new(vec![tool_response, final_response]));

    // Create a permission checker that denies the tool
    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .with_permission_checker(Arc::new(|_, _, _, _| {
            Box::pin(async { PermissionResult::deny() })
        }))
        .build();

    let mut execution = engine
        .submit_message("Use tool", None)
        .await
        .expect("Failed to submit message");

    let _ = execution.execute().await;

    // Should complete but with permission denials recorded
    assert!(execution.is_complete());

    // Check that permission denials were tracked
    let denials = engine.permission_denials.lock().await;
    assert!(!denials.is_empty(), "Should have recorded permission denials");
}

#[tokio::test]
async fn test_conversation_loop_stop_sequence_handling() {
    // Test handling of StopSequence stop reason
    let registry = create_registry();

    let stop_sequence_response = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "Response with stop sequence".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::StopSequence,
        usage: RuntimeUsage::default(),
        model: "mock".to_string(),
    };

    let llm_client = Arc::new(MockLlmClient::new(vec![stop_sequence_response]));

    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .build();

    let mut execution = engine
        .submit_message("Test", None)
        .await
        .expect("Failed to submit message");

    let _ = execution.execute().await.expect("Execution should complete");

    let result = execution.get_result().expect("Should have result");
    match result {
        QueryResultMessage::Success { stop_reason, .. } => {
            assert_eq!(stop_reason.as_deref(), Some("stop_sequence"));
        }
        _ => panic!("Expected Success result"),
    }
}

#[tokio::test]
async fn test_conversation_loop_empty_tool_calls_handling() {
    // Test that ToolUse stop reason with no actual tool calls ends conversation
    let registry = create_registry();

    // Response claims ToolUse but has no actual ToolUse blocks
    let empty_tool_response = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "I don't need tools".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::ToolUse, // Claims tool use but none in content
        usage: RuntimeUsage::default(),
        model: "mock".to_string(),
    };

    let llm_client = Arc::new(MockLlmClient::new(vec![empty_tool_response]));

    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .build();

    let mut execution = engine
        .submit_message("Do something", None)
        .await
        .expect("Failed to submit message");

    let _ = execution.execute().await.expect("Execution should complete");

    // Should complete naturally when no tools found
    let result = execution.get_result().expect("Should have result");
    match result {
        QueryResultMessage::Success { stop_reason, .. } => {
            // Falls back to end_turn when no tools found
            assert_eq!(stop_reason.as_deref(), Some("end_turn"));
        }
        _ => panic!("Expected Success result"),
    }
}
