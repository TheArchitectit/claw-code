#!/usr/bin/env python3
import re

# Read the file
with open('/mnt/ollama/git/claw-code/rust/crates/runtime/src/query_engine.rs', 'r') as f:
    content = f.read()

# 1. Update execute_tool to use with_panic_catch
old_tool_execution = '''        // Execute the tool
        let output = tool
            .execute(input, &context, tool_use_id, None)
            .await
            .map_err(QueryEngineError::Tool)?;'''

new_tool_execution = '''        // Execute the tool with panic catching
        let output = with_panic_catch(
            || async {
                tool.execute(input, &context, tool_use_id, None).await
            },
            tool_name,
        )
        .await
        .map_err(QueryEngineError::Tool)?;'''

content = content.replace(old_tool_execution, new_tool_execution)

# 2. Find and update get_llm_response to use with_retry (if it exists)
# Check if get_llm_response method exists and wrap it with retry
llm_response_pattern = r'(async fn get_llm_response\([^)]+\)[^\{]*\{[^}]*?)(client\.complete\(request\)\.await)'

if re.search(llm_response_pattern, content):
    content = re.sub(
        llm_response_pattern,
        r'\1with_retry(|| async { \2.map_err(|e| QueryEngineError::LlmApi(LlmApiError::Other { message: e.to_string() })) }, 3, 1000).await',
        content
    )

# Write the file back
with open('/mnt/ollama/git/claw-code/rust/crates/runtime/src/query_engine.rs', 'w') as f:
    f.write(content)

print("Updated query_engine.rs with error handling")
