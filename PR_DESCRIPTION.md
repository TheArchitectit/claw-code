# Fix: Handle reasoning/thinking content from models

## Problem
When using reasoning-capable models (e.g., Claude with extended thinking, Grok 3, OpenAI o3/o4), the application fails with:
```
❌ Request failed
[error-kind: unknown]
error: assistant stream produced no content
```

This occurs when the model returns **only** thinking/reasoning blocks without regular text content.

## Root Cause
The SSE stream parser and event converter were explicitly ignoring `Thinking` and `RedactedThinking` content blocks:

```rust
// In rust/crates/tools/src/lib.rs
OutputContentBlock::Thinking { .. } | OutputContentBlock::RedactedThinking { .. } => {}

ContentBlockDelta::ThinkingDelta { .. } | ContentBlockDelta::SignatureDelta { .. } => {}
```

When a model returned only thinking content, zero `AssistantEvent` content events were produced. The `build_assistant_message` function then correctly rejected this as "no content".

## Solution
1. **Added `ThinkingDelta` event variant** (`rust/crates/runtime/src/conversation.rs`)
   - New `AssistantEvent::ThinkingDelta { thinking, signature }` variant
   - Accumulates thinking content and flushes it as text blocks wrapped in `<thinking>` tags
   - Updated "no content" check to consider thinking as valid content

2. **Emit thinking events from stream** (`rust/crates/tools/src/lib.rs`)
   - `push_output_block` now emits `ThinkingDelta` for thinking blocks
   - `ContentBlockDelta` handler processes `ThinkingDelta` and `SignatureDelta`
   - Synthetic `MessageStop` check includes thinking as valid content

## Changes Checklist

| File | Change | Why |
|------|--------|-----|
| `runtime/src/conversation.rs` | Added `ThinkingDelta` variant to `AssistantEvent` | Allow thinking content to flow through the runtime |
| `runtime/src/conversation.rs` | Added `flush_thinking_block()` helper | Convert accumulated thinking to displayable text blocks |
| `runtime/src/conversation.rs` | Updated `build_assistant_message()` | Accept thinking as valid content; prevent false "no content" errors |
| `runtime/src/conversation.rs` | Added tests for thinking content | Verify fix works for thinking-only and thinking+signature cases |
| `tools/src/lib.rs` | Updated `push_output_block()` | Emit thinking events instead of ignoring |
| `tools/src/lib.rs` | Updated `ContentBlockDelta` handler | Process thinking deltas and signatures |
| `tools/src/lib.rs` | Updated synthetic stop check | Treat thinking as valid content for stream completion |

## Testing
- Added `build_assistant_message_accepts_thinking_content` test
- Added `build_assistant_message_accepts_thinking_with_signature` test
- All 23 conversation tests pass

## Impact
- Enables use of reasoning models that return thinking content
- Backward compatible: regular text/tool content flows unchanged
- Redacted thinking is intentionally skipped (no useful content to display)
