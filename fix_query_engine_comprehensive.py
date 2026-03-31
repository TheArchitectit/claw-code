#!/usr/bin/env python3
"""Comprehensive fix for query_engine.rs syntax issues."""

# Read the file
with open('rust/crates/runtime/src/query_engine.rs', 'r') as f:
    content = f.read()

# Track changes
changes = []

# Fix 1: Remove orphaned 'self' and extra } after get_available_tool_definitions
# Pattern: .collect()\n    }\n        self\n    }\n\n/// The QueryEngine
old_pattern1 = '''            .collect()
    }
        self
    }

/// The QueryEngine owns the query lifecycle'''

new_pattern1 = '''            .collect()
    }

/// The QueryEngine owns the query lifecycle'''

if old_pattern1 in content:
    content = content.replace(old_pattern1, new_pattern1)
    changes.append("Fixed orphaned 'self' and extra '}' after get_available_tool_definitions")

# Fix 2: Remove extra } after update_total_usage
# Pattern: }\n\n}\n\n    /// Run a complete conversation turn
old_pattern2 = '''        }
    }

    }

    /// Run a complete conversation turn with simulated tool-call loop.'''

new_pattern2 = '''        }
    }

    /// Run a complete conversation turn with simulated tool-call loop.'''

if old_pattern2 in content:
    content = content.replace(old_pattern2, new_pattern2)
    changes.append("Fixed extra '}' after update_total_usage")

# Fix 3: Remove extra } between total_usage and total_cost
old_pattern3 = '''    }

    }
    /// Get the total cost for this session.'''

new_pattern3 = '''    }

    /// Get the total cost for this session.'''

if old_pattern3 in content:
    content = content.replace(old_pattern3, new_pattern3)
    changes.append("Fixed extra '}' between total_usage and total_cost")

# Write the fixed file
with open('rust/crates/runtime/src/query_engine.rs', 'w') as f:
    f.write(content)

print(f"Fixed {len(changes)} issues:")
for c in changes:
    print(f"  - {c}")
