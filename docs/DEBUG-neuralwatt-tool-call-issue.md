# Debug Report: NeuralWatt API Tool Call Issue

**Date:** 2026-04-27
**Reporter:** Claw Code user via OpenClaw
**Model:** glm-5.1-fast (and glm-5-fast)
**Issue:** Intermittent 400 Bad Request errors with tool calls

## Summary

User reports getting HTTP 400 errors when using tool calls with Claw Code against NeuralWatt API. The error message shows:
```
[error-kind: api_http_error]
error: api returned 400 Bad Request (invalid_request_error): HTTP 400 from backend (no parseable body)
```

**Note:** The "no parseable body" part suggests the backend response was not valid JSON, which may indicate a backend error rather than a validation issue.

## Successful Test Cases

### 1. Basic chat completion (no tools)
```bash
curl -s "https://api.neuralwatt.com/v1/chat/completions" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "glm-5.1-fast",
    "messages": [{"role": "user", "content": "Say hello"}],
    "max_tokens": 50
  }'
```
**Result:** ✅ Works

### 2. Simple tool call
```bash
curl -s "https://api.neuralwatt.com/v1/chat/completions" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "glm-5.1-fast",
    "messages": [{"role": "user", "content": "What is the weather in Chicago?"}],
    "tools": [{
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get the current weather for a location",
        "parameters": {
          "type": "object",
          "properties": {
            "location": {"type": "string", "description": "City name"}
          },
          "required": ["location"]
        }
      }
    }],
    "tool_choice": "auto"
  }'
```
**Result:** ✅ Works - model correctly returns tool call

### 3. Tool with `additionalProperties: false` (Claw normalization)
```bash
curl -s "https://api.neuralwatt.com/v1/chat/completions" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "glm-5.1-fast",
    "messages": [{"role": "user", "content": "List files in current directory"}],
    "tools": [{
      "type": "function",
      "function": {
        "name": "bash",
        "description": "Execute a bash command",
        "parameters": {
          "type": "object",
          "properties": {
            "command": {"type": "string", "description": "The command to execute"}
          },
          "required": ["command"],
          "additionalProperties": false
        }
      }
    }],
    "tool_choice": "auto"
  }'
```
**Result:** ✅ Works

### 4. Tool with optional parameters
```bash
curl -s "https://api.neuralwatt.com/v1/chat/completions" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "glm-5.1-fast",
    "messages": [{"role": "user", "content": "Read the file test.txt"}],
    "tools": [{
      "type": "function",
      "function": {
        "name": "read_file",
        "description": "Read file contents",
        "parameters": {
          "type": "object",
          "properties": {
            "path": {"type": "string", "description": "Path to the file"},
            "offset": {"type": "number", "description": "Line number to start reading from"},
            "limit": {"type": "number", "description": "Maximum number of lines to read"}
          },
          "required": ["path"],
          "additionalProperties": false
        }
      }
    }],
    "tool_choice": "auto"
  }'
```
**Result:** ✅ Works

## Claw Tool Definition Format

Claw Code normalizes tool schemas before sending to OpenAI-compatible APIs:

```rust
fn normalize_object_schema(schema: &mut Value) {
    if let Some(obj) = schema.as_object_mut() {
        if obj.get("type").and_then(Value::as_str) == Some("object") {
            obj.entry("properties").or_insert_with(|| json!({}));
            obj.entry("additionalProperties")
                .or_insert(Value::Bool(false));
        }
        // Recursively normalize nested objects
        // ...
    }
}
```

This adds:
- `"properties": {}` if missing for object types
- `"additionalProperties": false` if missing for object types

## Possible Causes

1. **Large request body** - Claw may send many tools with large schemas
2. **Streaming mode** - Claw uses streaming, may differ from non-streaming
3. **Backend transient errors** - "no parseable body" suggests backend crash/error
4. **Specific schema patterns** - Certain nested schemas may trigger validation issues

## Information Needed from Provider

1. Raw HTTP request body that caused the 400 error
2. Actual response body returned (for "no parseable body" cases)
3. Backend logs for the failing request
4. Any schema validation errors on the backend

## Actual Failure Scenario

The 400 error occurred when **starting a new session**, not resuming an existing tool call.

Session evidence:
- `session-1777295962205-0.jsonl`: Previous session using `moonshotai/Kimi-K2.6`, completed successfully with no incomplete tool calls
- `session-1777301106352-0.jsonl`: New session with `glm-5.1-fast`, contains only session_meta - no messages

**The failure happened on the initial API call** when Claw sends:
1. System prompt (large, includes tool documentation)
2. Tool definitions (~20+ tools with complex schemas)

This is NOT a "resumed tool call" issue - it's an initial session startup failure.

## Reproduction Attempt

To reproduce with a realistic Claw-like request, try:

```bash
# This simulates a typical Claw request with multiple tools
curl -v "https://api.neuralwatt.com/v1/chat/completions" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d @- << 'EOF'
{
  "model": "glm-5.1-fast",
  "messages": [
    {"role": "user", "content": "Read the file /mnt/data/git/RadGameRandom01/godot-rad-defense/scripts/placement_controller.gd"}
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "read_file",
        "description": "Read the contents of a file. Supports text files and images.",
        "parameters": {
          "type": "object",
          "properties": {
            "path": {"type": "string", "description": "Path to the file to read"},
            "offset": {"type": "number", "description": "Line number to start reading from (1-indexed)"},
            "limit": {"type": "number", "description": "Maximum number of lines to read"}
          },
          "required": ["path"],
          "additionalProperties": false
        }
      }
    }
  ],
  "tool_choice": "auto",
  "stream": true
}
EOF
```

## Contact

For follow-up, the user can provide:
- Claw session file with the failing request
- Timestamp of the error for backend log correlation
- Request ID if available in response headers
