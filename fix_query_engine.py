#!/usr/bin/env python3
"""Fix the corrupted query_engine.rs file."""
import re

# Read the file
with open('rust/crates/runtime/src/query_engine.rs', 'r') as f:
    content = f.read()

# Fix 1: Remove the corrupted lines after get_available_tool_definitions
# This removes: \n} followed by 8 spaces, self, newline, 4 spaces, }
pattern1 = r'(\.collect\(\)\n    \}\n)\s+self\n\s+\}\n'
replacement1 = r'\1'
content = re.sub(pattern1, replacement1, content)

# Fix 2: Remove extra closing brace after update_total_usage
# Look for pattern: \n}\n\n}\n followed by doc comment
pattern2 = r'(\}\n\}\n\n)    /// Run a complete conversation turn'
replacement2 = r'\1/// Run a complete conversation turn'
content = re.sub(pattern2, replacement2, content)

# Write the fixed file
with open('rust/crates/runtime/src/query_engine.rs', 'w') as f:
    f.write(content)

print("File fixed successfully")
