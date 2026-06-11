# Bug: `claw setup` Writes `model` Inside `provider` Instead of Top-Level

## Status
- **Discovered:** 2026-04-27
- **Impact:** Users running `claw setup` get their model config ignored; session falls back to unknown model selection behavior

## Problem

The `claw setup` wizard writes settings in this format:

```json
{
  "provider": {
    "apiKey": "...",
    "baseUrl": "...",
    "kind": "openai",
    "model": "glm-5.1-fast"
  }
}
```

But `RuntimeConfig::model()` in `crates/runtime/src/config.rs` parses the `model` field from the **top level**:

```rust
fn parse_optional_model(merged: &JsonValue) -> Option<String> {
    // Expects { "model": "glm-5.1-fast", ... }
    merged.get("model").and_then(|v| v.as_str()).map(str::to_string)
}
```

Result: The model setting is silently ignored, and sessions use whatever fallback/default logic applies.

## Workaround

Manually edit `~/.claw/settings.json` to move `model` to the top level:

```json
{
  "model": "glm-5.1-fast",
  "provider": {
    "apiKey": "...",
    "baseUrl": "...",
    "kind": "openai"
  }
}
```

## Fix Required

In the setup wizard code, ensure the `model` field is written at the top level of the JSON object, not nested under `provider`.

**Likely location:** `crates/rusty-claude-cli/src/setup.rs` or similar setup-writer module.

**Change:** After collecting model input, write to top-level `"model"` key instead of `provider.model`.

## Related

- Config loading logic: `crates/runtime/src/config.rs`
- RuntimeFeatureConfig parses: `model: parse_optional_model(&merged_value)`
