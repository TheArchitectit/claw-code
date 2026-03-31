#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::types::Usage as RuntimeUsage;

    #[test]
    fn test_message_id_methods() {
        let user_msg = Message::User(UserMessage {
            id: MessageId::new(),
            session_id: SessionId::new(),
            timestamp: Utc::now(),
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: None,
            parent_tool_use_id: None,
            is_replay: None,
            is_synthetic: None,
        });

        assert!(user_msg.is_user());
        assert!(!user_msg.is_assistant());
        assert!(!user_msg.is_system());
    }

    #[test]
    fn test_message_is_methods() {
        let user = Message::User(UserMessage {
            id: MessageId::new(),
            session_id: SessionId::new(),
            timestamp: Utc::now(),
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: None,
            parent_tool_use_id: None,
            is_replay: None,
            is_synthetic: None,
        });

        let assistant = Message::Assistant(AssistantMessage {
            id: MessageId::new(),
            session_id: SessionId::new(),
            timestamp: Utc::now(),
            content: AssistantMessageContent {
                role: "assistant".to_string(),
                content: vec![],
                model: "claude".to_string(),
                stop_reason: None,
                usage: None,
            },
            parent_tool_use_id: None,
        });

        let system = Message::System(SystemMessage {
            id: MessageId::new(),
            session_id: SessionId::new(),
            timestamp: Utc::now(),
            subtype: SystemMessageSubtype::Informational {
                level: MessageLevel::Info,
                content: "test".to_string(),
            },
        });

        assert!(user.is_user());
        assert!(!user.is_assistant());
        assert!(!user.is_system());

        assert!(!assistant.is_user());
        assert!(assistant.is_assistant());
        assert!(!assistant.is_system());

        assert!(!system.is_user());
        assert!(!system.is_assistant());
        assert!(system.is_system());
    }

    #[test]
    fn test_message_timestamp() {
        let before = Utc::now();
        let msg = Message::User(UserMessage {
            id: MessageId::new(),
            session_id: SessionId::new(),
            timestamp: Utc::now(),
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: None,
            parent_tool_use_id: None,
            is_replay: None,
            is_synthetic: None,
        });
        let after = Utc::now();

        let ts = msg.timestamp();
        assert!(ts >= before && ts <= after);
    }

    #[test]
    fn test_message_normalization_user() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();
        let timestamp = Utc::now();

        let user_msg = Message::User(UserMessage {
            id: msg_id,
            session_id,
            timestamp,
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![ContentBlock::Text {
                    text: "Hello".to_string(),
                    citation: None,
                }],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: None,
            parent_tool_use_id: None,
            is_replay: None,
            is_synthetic: None,
        });

        let normalized = user_msg.normalize().unwrap();
        match normalized {
            NormalizedMessage::User(normalized) => {
                assert_eq!(normalized.uuid, msg_id);
                assert_eq!(normalized.timestamp, timestamp);
                assert_eq!(normalized.message.content.len(), 1);
            }
            _ => panic!("Expected NormalizedMessage::User"),
        }
    }

    #[test]
    fn test_message_normalization_assistant() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();
        let parent_tool_id = ToolUseId::new("parent");
        let timestamp = Utc::now();
        let usage = RuntimeUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: Some(5),
        };

        let assistant_msg = Message::Assistant(AssistantMessage {
            id: msg_id,
            session_id,
            timestamp,
            content: AssistantMessageContent {
                role: "assistant".to_string(),
                content: vec![ContentBlock::Text {
                    text: "Response".to_string(),
                    citation: None,
                }],
                model: "claude".to_string(),
                stop_reason: Some("end_turn".to_string()),
                usage: Some(usage.clone()),
            },
            parent_tool_use_id: Some(parent_tool_id),
        });

        let normalized = assistant_msg.normalize().unwrap();
        match normalized {
            NormalizedMessage::Assistant(normalized) => {
                assert_eq!(normalized.uuid, msg_id);
                assert_eq!(normalized.timestamp, timestamp);
                assert!(normalized.usage.is_some());
                let n_usage = normalized.usage.unwrap();
                assert_eq!(n_usage.input_tokens, 100);
                assert_eq!(n_usage.output_tokens, 50);
            }
            _ => panic!("Expected NormalizedMessage::Assistant"),
        }
    }

    #[test]
    fn test_message_normalization_system() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();
        let timestamp = Utc::now();

        let system_msg = Message::System(SystemMessage {
            id: msg_id,
            session_id,
            timestamp,
            subtype: SystemMessageSubtype::Informational {
                level: MessageLevel::Info,
                content: "System notification".to_string(),
            },
        });

        let normalized = system_msg.normalize().unwrap();
        match normalized {
            NormalizedMessage::System { id, content, timestamp: ts } => {
                assert_eq!(id, msg_id);
                assert_eq!(ts, timestamp);
                assert!(content.contains("System notification"));
            }
            _ => panic!("Expected NormalizedMessage::System"),
        }
    }

    #[test]
    fn test_message_normalization_progress() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();
        let tool_use_id = ToolUseId::new("tool");
        let timestamp = Utc::now();

        let progress_msg = Message::Progress(ProgressMessage {
            id: msg_id,
            session_id,
            timestamp,
            tool_use_id,
            data: ProgressData::ToolProgress {
                message: "Processing...".to_string(),
            },
        });

        let normalized = progress_msg.normalize().unwrap();
        match normalized {
            NormalizedMessage::Progress { id, data, timestamp: ts } => {
                assert_eq!(id, msg_id);
                assert_eq!(ts, timestamp);
                match data {
                    ProgressData::ToolProgress { message } => {
                        assert_eq!(message, "Processing...");
                    }
                    _ => panic!("Expected ToolProgress"),
                }
            }
            _ => panic!("Expected NormalizedMessage::Progress"),
        }
    }

    #[test]
    fn test_message_normalization_tool_result() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();
        let tool_use_id = ToolUseId::new("tool123");
        let timestamp = Utc::now();

        let user_msg = Message::User(UserMessage {
            id: msg_id,
            session_id,
            timestamp,
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: Some(ToolUseResult {
                tool_use_id,
                content: vec![ToolResultContent::Text {
                    text: "Tool output".to_string(),
                }],
                is_error: false,
            }),
            parent_tool_use_id: Some(tool_use_id),
            is_replay: None,
            is_synthetic: None,
        });

        let normalized = user_msg.normalize().unwrap();
        match normalized {
            NormalizedMessage::ToolResult { tool_use_id: tuid, content, is_error } => {
                assert_eq!(tuid, tool_use_id);
                assert!(!is_error);
                assert_eq!(content.len(), 1);
            }
            _ => panic!("Expected NormalizedMessage::ToolResult"),
        }
    }

    #[test]
    fn test_message_normalization_attachment() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();
        let timestamp = Utc::now();

        let attachment_msg = Message::Attachment(AttachmentMessage {
            id: msg_id,
            session_id,
            timestamp,
            attachment: Attachment::MaxTurnsReached {
                turn_count: 10,
                max_turns: 50,
            },
        });

        let normalized = attachment_msg.normalize().unwrap();
        match normalized {
            NormalizedMessage::System { id, content, timestamp: ts } => {
                assert_eq!(id, msg_id);
                assert_eq!(ts, timestamp);
                assert!(content.contains("Max turns reached"));
                assert!(content.contains("10/50"));
            }
            _ => panic!("Expected NormalizedMessage::System for attachment"),
        }
    }

    #[test]
    fn test_message_normalization_tool_use_summary() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();
        let timestamp = Utc::now();

        let summary_msg = Message::ToolUseSummary(ToolUseSummaryMessage {
            id: msg_id,
            session_id,
            timestamp,
            summary: "Files were modified".to_string(),
            preceding_tool_use_ids: vec![
                ToolUseId::new("tool1"),
                ToolUseId::new("tool2"),
            ],
        });

        let normalized = summary_msg.normalize().unwrap();
        match normalized {
            NormalizedMessage::System { id, content, timestamp: ts } => {
                assert_eq!(id, msg_id);
                assert_eq!(ts, timestamp);
                assert!(content.contains("Tool use summary (2 tools)"));
                assert!(content.contains("Files were modified"));
            }
            _ => panic!("Expected NormalizedMessage::System for tool use summary"),
        }
    }

    #[test]
    fn test_message_normalization_tombstone() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();

        let tombstone_msg = Message::Tombstone(TombstoneMessage {
            id: msg_id,
            session_id,
            timestamp: Utc::now(),
            remove_message_ids: vec![MessageId::new()],
        });

        assert!(tombstone_msg.normalize().is_none());
    }

    #[test]
    fn test_message_metadata_preservation() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();
        let parent_id = ToolUseId::new("parent");
        let timestamp = Utc::now();

        let user_msg = Message::User(UserMessage {
            id: msg_id,
            session_id,
            timestamp,
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: None,
            parent_tool_use_id: Some(parent_id),
            is_replay: None,
            is_synthetic: None,
        });

        assert_eq!(user_msg.id(), msg_id);
        assert_eq!(user_msg.session_id(), session_id);
        assert_eq!(user_msg.timestamp(), timestamp);
        assert_eq!(user_msg.parent_tool_use_id(), Some(parent_id));
        assert!(user_msg.is_user());
        assert!(!user_msg.is_assistant());
        assert!(!user_msg.is_system());
        assert!(!user_msg.is_progress());
        assert!(!user_msg.is_tool_result());
    }

    #[test]
    fn test_message_is_tool_result_detection() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();
        let tool_use_id = ToolUseId::new("tool");

        let plain_user = Message::User(UserMessage {
            id: msg_id,
            session_id,
            timestamp: Utc::now(),
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: None,
            parent_tool_use_id: None,
            is_replay: None,
            is_synthetic: None,
        });
        assert!(!plain_user.is_tool_result());

        let tool_result_user = Message::User(UserMessage {
            id: MessageId::new(),
            session_id,
            timestamp: Utc::now(),
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: Some(ToolUseResult {
                tool_use_id,
                content: vec![ToolResultContent::Text {
                    text: "result".to_string(),
                }],
                is_error: false,
            }),
            parent_tool_use_id: Some(tool_use_id),
            is_replay: None,
            is_synthetic: None,
        });
        assert!(tool_result_user.is_tool_result());
    }

    #[test]
    fn test_message_json_roundtrip() {
        let session_id = SessionId::new();
        let msg_id = MessageId::new();

        let original = Message::User(UserMessage {
            id: msg_id,
            session_id,
            timestamp: Utc::now(),
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![ContentBlock::Text {
                    text: "Test message".to_string(),
                    citation: None,
                }],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: None,
            parent_tool_use_id: None,
            is_replay: None,
            is_synthetic: None,
        });

        let json = original.to_json().unwrap();
        let restored = Message::from_json(&json).unwrap();

        assert_eq!(restored.id(), msg_id);
        assert_eq!(restored.session_id(), session_id);
        assert!(restored.is_user());
    }

    #[test]
    fn test_message_parent_tool_use_id() {
        let session_id = SessionId::new();
        let tool_use_id = ToolUseId::new("tool1");

        let user = Message::User(UserMessage {
            id: MessageId::new(),
            session_id,
            timestamp: Utc::now(),
            content: UserMessageContent {
                role: "user".to_string(),
                content: vec![],
            },
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: None,
            parent_tool_use_id: Some(tool_use_id.clone()),
            is_replay: None,
            is_synthetic: None,
        });
        assert_eq!(user.parent_tool_use_id(), Some(tool_use_id.clone()));

        let progress = Message::Progress(ProgressMessage {
            id: MessageId::new(),
            session_id,
            timestamp: Utc::now(),
            tool_use_id: tool_use_id.clone(),
            data: ProgressData::ToolProgress {
                message: "test".to_string(),
            },
        });
        assert_eq!(progress.parent_tool_use_id(), Some(tool_use_id));

        let system = Message::System(SystemMessage {
            id: MessageId::new(),
            session_id,
            timestamp: Utc::now(),
            subtype: SystemMessageSubtype::Informational {
                level: MessageLevel::Info,
                content: "test".to_string(),
            },
        });
        assert!(system.parent_tool_use_id().is_none());
    }

    #[test]
    fn test_system_message_subtype_normalization() {
        let test_cases = vec![
            (
                SystemMessageSubtype::ApiMetrics { ttft_ms: 500 },
                "TTFT=500ms"
            ),
            (
                SystemMessageSubtype::LocalCommand {
                    content: "output text".to_string(),
                },
                "Local command output"
            ),
            (
                SystemMessageSubtype::MemorySaved {
                    path: "/tmp/test.md".to_string(),
                },
                "Memory saved"
            ),
            (
                SystemMessageSubtype::TurnDuration { duration_ms: 1000 },
                "Turn duration: 1000ms"
            ),
            (
                SystemMessageSubtype::PermissionRetry {
                    tool_use_id: ToolUseId::new("tool1"),
                },
                "Permission retry"
            ),
        ];

        for (subtype, expected_content) in test_cases {
            let msg = SystemMessage {
                id: MessageId::new(),
                session_id: SessionId::new(),
                timestamp: Utc::now(),
                subtype,
            };

            let normalized = msg.normalize();
            match normalized {
                NormalizedMessage::System { content, .. } => {
                    assert!(content.contains(expected_content));
                }
                _ => panic!("Expected NormalizedMessage::System"),
            }
        }
    }

    #[test]
    fn test_attachment_variants_normalization() {
        let test_cases = vec![
            (
                Attachment::StructuredOutput {
                    data: serde_json::json!({"key": "value"}),
                },
                "Structured output"
            ),
            (
                Attachment::QueuedCommand {
                    prompt: "test command".to_string(),
                    source_uuid: None,
                },
                "Queued command"
            ),
            (
                Attachment::Hook {
                    hook_name: "test_hook".to_string(),
                    data: serde_json::json!({}),
                },
                "Hook 'test_hook'"
            ),
            (
                Attachment::Memory {
                    path: "/tmp/test.md".to_string(),
                    content: "memory content".to_string(),
                },
                "Memory file '/tmp/test.md'"
            ),
        ];

        for (attachment, expected_prefix) in test_cases {
            let msg = AttachmentMessage {
                id: MessageId::new(),
                session_id: SessionId::new(),
                timestamp: Utc::now(),
                attachment,
            };

            let normalized = msg.normalize().unwrap();
            match normalized {
                NormalizedMessage::System(n) => {
                    assert!(n.content.starts_with(expected_prefix));
                }
                _ => panic!("Expected NormalizedMessage::System for attachment"),
            }
        }
    }

    #[test]
    fn test_progress_data_variants_normalization() {
        let test_cases = vec![
            ProgressData::Bash {
                command: "echo hi".to_string(),
                output: Some("hi".to_string()),
            },
            ProgressData::FileRead {
                path: "/tmp/file.txt".to_string(),
                bytes_read: 50,
                total_bytes: 100,
            },
            ProgressData::Mcp {
                server_name: "test_server".to_string(),
                operation: "read".to_string(),
            },
            ProgressData::Agent {
                agent_name: "agent1".to_string(),
                status: "working".to_string(),
            },
            ProgressData::Hook {
                hook_name: "test_hook".to_string(),
                message: "hook message".to_string(),
            },
        ];

        for data in test_cases {
            let msg = ProgressMessage {
                id: MessageId::new(),
                session_id: SessionId::new(),
                timestamp: Utc::now(),
                tool_use_id: ToolUseId::new("tool"),
                data,
            };

            let normalized = msg.normalize();
            assert!(matches!(normalized, NormalizedMessage::Progress(_)));
        }
    }

    #[test]
    fn test_conversation_normalization() {
        let session_id = SessionId::new();
        let messages: Vec<Message> = vec![
            Message::User(UserMessage {
                id: MessageId::new(),
                session_id,
                timestamp: Utc::now(),
                content: UserMessageContent {
                    role: "user".to_string(),
                    content: vec![ContentBlock::Text {
                        text: "Hello".to_string(),
                        citation: None,
                    }],
                },
                is_meta: None,
                is_visible_in_transcript_only: None,
                tool_use_result: None,
                parent_tool_use_id: None,
                is_replay: None,
                is_synthetic: None,
            }),
            Message::Assistant(AssistantMessage {
                id: MessageId::new(),
                session_id,
                timestamp: Utc::now(),
                content: AssistantMessageContent {
                    role: "assistant".to_string(),
                    content: vec![ContentBlock::Text {
                        text: "Hi there".to_string(),
                        citation: None,
                    }],
                    model: "claude".to_string(),
                    stop_reason: None,
                    usage: Some(RuntimeUsage::default()),
                },
                parent_tool_use_id: None,
            }),
            Message::System(SystemMessage {
                id: MessageId::new(),
                session_id,
                timestamp: Utc::now(),
                subtype: SystemMessageSubtype::Informational {
                    level: MessageLevel::Info,
                    content: "System info".to_string(),
                },
            }),
        ];

        let normalized: Vec<NormalizedMessage> = messages
            .iter()
            .filter_map(|m| m.normalize())
            .collect();

        assert_eq!(normalized.len(), 3);
        assert!(matches!(normalized[0], NormalizedMessage::User(_)));
        assert!(matches!(normalized[1], NormalizedMessage::Assistant(_)));
        assert!(matches!(normalized[2], NormalizedMessage::System(_)));
    }
}
