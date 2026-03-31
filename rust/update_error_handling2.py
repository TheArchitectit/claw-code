#!/usr/bin/env python3
with open('/mnt/ollama/git/claw-code/rust/crates/runtime/src/query_engine.rs', 'r') as f:
    lines = f.readlines()

# Find lines 1539-1543 (0-indexed: 1538-1542) and replace
# Looking for the pattern:
#         // Execute the tool
#         let output = tool
#             .execute(input, &context, tool_use_id, None)
#             .await
#             .map_err(QueryEngineError::Tool)?;

output_lines = []
i = 0
while i < len(lines):
    line = lines[i]
    if '// Execute the tool' in line and 'let output = tool' in lines[i+1]:
        # Found the pattern, replace with new code
        indent = line[:len(line) - len(line.lstrip())]
        output_lines.append(f'{indent}// Execute the tool with panic catching\n')
        output_lines.append(f'{indent}let output = with_panic_catch(\n')
        output_lines.append(f'{indent}    || async {{\n')
        output_lines.append(f'{indent}        tool.execute(input, &context, tool_use_id, None).await\n')
        output_lines.append(f'{indent}    }},\n')
        output_lines.append(f'{indent}    tool_name,\n')
        output_lines.append(f'{indent})\n')
        output_lines.append(f'{indent}.await\n')
        output_lines.append(f'{indent}.map_err(QueryEngineError::Tool)?;\n')
        # Skip the original 4 lines (the comment + 3 lines of code)
        i += 5  # Skip // Execute the tool + let output + .execute + .await + .map_err
    else:
        output_lines.append(line)
        i += 1

with open('/mnt/ollama/git/claw-code/rust/crates/runtime/src/query_engine.rs', 'w') as f:
    f.writelines(output_lines)

print("Done")
