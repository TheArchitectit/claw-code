# R.A.D Codicological 2.x - Command System Documentation

## Overview

The Command System in R.A.D Codicological 2.x provides a slash-command interface (`/command`) for users to interact with Claude Code. Commands are implemented primarily in TypeScript with a Rust compatibility layer for manifest extraction and harness testing.

### Architecture at a Glance

```
┌─────────────────────────────────────────────────────────────┐
│                    Command System                           │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐ │
│   │   Prompt     │    │    Local     │    │  Local-JSX   │ │
│   │  Commands    │    │   Commands   │    │   Commands   │ │
│   │   (Skills)   │    │              │    │              │ │
│   └──────────────┘    └──────────────┘    └──────────────┘ │
│          │                   │                   │          │
│          └───────────────────┴───────────────────┘          │
│                        │                                    │
│               ┌────────▼────────┐                          │
│               │  CommandRegistry │                          │
│               └────────┬────────┘                          │
│                        │                                    │
│               ┌────────▼────────┐                          │
│               │   Dispatcher    │                          │
│               └─────────────────┘                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Command Types

Commands are categorized into three types based on their execution model:

### 1. Prompt Commands (`type: 'prompt'`)

Prompt commands expand to text that is sent to the LLM. These are typically skills that provide specialized capabilities.

**Key characteristics:**
- Expand to text content (ContentBlockParam[])
- Can specify allowed tools
- Support model override
- Can run inline or forked as sub-agents
- Support effort levels (low/medium/high)

**Example:** `/review`, `/security-review`, `/skills`

### 2. Local Commands (`type: 'local'`)

Local commands execute immediately in the terminal without LLM interaction. They produce direct output.

**Key characteristics:**
- Execute synchronously/asynchronously
- Return text or structured output
- Support non-interactive mode
- Lazy-loaded for performance

**Example:** `/help`, `/status`, `/doctor`, `/diff`

### 3. Local-JSX Commands (`type: 'local-jsx'`)

Local-JSX commands render interactive UI components using the Ink framework (React for terminals).

**Key characteristics:**
- Render interactive UI
- Support stateful interactions
- Can modify conversation state
- Support complex UI flows

**Example:** `/config`, `/compact`, `/tasks`, `/skills`

---

## Complete Command Inventory

### Git Workflow Commands

| Command | Aliases | Type | Description |
|---------|---------|------|-------------|
| `/commit` | `c` | prompt | Create a git commit with AI-generated message |
| `/diff` | `d`, `changes` | local | Show git diff of current changes |
| `/pr_comments` | - | local | Review and manage PR comments |
| `/branch` | - | local-jsx | Branch management operations |

**Implementation Details:**

**`/commit`** (`src/commands/commit.ts`)
- Generates commit messages based on staged changes
- Supports `--amend` flag
- Integrates with git workflow

**`/diff`** (`src/commands/diff/index.ts`)
- Shows working directory changes
- Supports `--cached` for staged changes
- Supports `--stat` for summary statistics
- Supports `--summary` for formatted summary

**`/pr_comments`** (`src/commands/pr_comments/index.ts`)
- Fetches PR comments from GitHub
- Allows responding to review comments

---

### Review Commands

| Command | Aliases | Type | Description |
|---------|---------|------|-------------|
| `/review` | `r`, `pr` | prompt | Review code changes |
| `/security-review` | - | prompt | Security-focused code review |
| `/ultrareview` | - | prompt | Comprehensive AI-assisted review |

**Implementation Details:**

**`/review`** (`src/commands/review.ts`)
- Reviews staged changes or specified files
- Supports `--staged` flag
- Generates review comments

**`/security-review`** (`src/commands/security-review.ts`)
- Focuses on security vulnerabilities
- Checks for common security issues
- Provides remediation suggestions

---

### Session Management Commands

| Command | Aliases | Type | Description |
|---------|---------|------|-------------|
| `/compact` | `compress`, `summary` | local-jsx | Compact conversation history |
| `/resume` | - | local-jsx | Resume a previous session |
| `/share` | - | prompt | Share conversation/session |
| `/session` | - | local-jsx | Session management (QR code, URL) |
| `/rename` | - | local-jsx | Rename current session |
| `/summary` | - | prompt | Generate conversation summary |
| `/teleport` | `tp`, `goto` | local | Switch between git repositories |
| `/rewind` | - | local-jsx | Rewind conversation to previous state |

**Implementation Details:**

**`/compact`** (`src/commands/compact/index.ts`)
- Compresses conversation history
- Summarizes old messages
- Frees up context window

**`/resume`** (`src/commands/resume/index.ts`)
- Lists available sessions
- Supports resuming by ID or title
- Entrypoints: `cli_flag`, `slash_command_picker`, `slash_command_session_id`

**`/teleport`** (`src/commands/teleport/index.ts`)
- Lists nearby git repositories
- Switches working directory
- Maintains context across repos

---

### Diagnostics Commands

| Command | Aliases | Type | Description |
|---------|---------|------|-------------|
| `/doctor` | `checkup`, `health` | local | Check environment health |
| `/cost` | `price`, `$` | local | Show session cost information |
| `/status` | `info`, `state` | local-jsx | Display session information |
| `/stats` | - | local | Show usage statistics |
| `/insights` | - | prompt | Generate usage report |
| `/heapdump` | - | local | Capture heap dump for debugging |

**Implementation Details:**

**`/doctor`** (`src/commands/doctor/index.ts`)
- Checks Rust toolchain installation
- Verifies Git configuration
- Validates environment variables
- Reports issues with fixes

**`/cost`** (`src/commands/cost/index.ts`)
- Tracks token usage
- Calculates API costs
- Shows input/output breakdown

**`/status`** (`src/commands/status/index.ts`)
- Shows current working directory
- Displays session ID
- Lists registered tools
- Shows system information

---

### Configuration Commands

| Command | Aliases | Type | Description |
|---------|---------|------|-------------|
| `/config` | - | local-jsx | Manage configuration settings |
| `/memory` | - | local-jsx | Manage persistent memory |
| `/theme` | - | local-jsx | Change terminal theme |
| `/color` | - | local-jsx | Change agent color |
| `/model` | - | local-jsx | Switch LLM model |
| `/permissions` | - | local-jsx | Manage permission settings |
| `/privacy-settings` | - | local-jsx | Configure privacy options |
| `/output-style` | - | local-jsx | Change output formatting |
| `/keybindings` | - | local-jsx | Customize keyboard shortcuts |
| `/vim` | - | local | Toggle vim mode |
| `/hooks` | - | local-jsx | Manage command hooks |

**Implementation Details:**

**`/config`** (`src/commands/config/index.ts`)
- View/edit settings
- Support for nested configuration
- Validates settings on save

**`/memory`** (`src/commands/memory/index.ts`)
- Manages persistent user memories
- CRUD operations for memory entries
- Categorizes by type (user, project, feedback, reference)

---

### Task and Planning Commands

| Command | Aliases | Type | Description |
|---------|---------|------|-------------|
| `/tasks` | `task`, `t` | local-jsx | Manage tasks and todo lists |
| `/plan` | - | local-jsx | Plan mode toggle |
| `/agents` | - | local-jsx | Manage AI agents |
| `/skills` | `skill` | local-jsx | Manage available skills |
| `/passes` | - | local | Multi-pass execution control |

**Implementation Details:**

**`/tasks`** (`src/commands/tasks/index.ts`)
- CRUD operations for tasks
- Filter by status and owner
- Track task progress

**`/plan`** (`src/commands/plan/index.ts`)
- Toggles plan mode
- Shows pending plan items
- Tracks plan progress

**`/skills`** (`src/commands/skills/index.ts`)
- Lists available skills by category
- Shows skill details
- Enable/disable skills (stub)

---

### File and Context Commands

| Command | Aliases | Type | Description |
|---------|---------|------|-------------|
| `/files` | - | local | List tracked files |
| `/add-dir` | - | prompt | Add directory to context |
| `/context` | - | local-jsx | Manage conversation context |
| `/clear` | - | local-jsx | Clear screen/conversation |
| `/copy` | - | local | Copy last message |
| `/export` | - | local-jsx | Export conversation |

**Implementation Details:**

**`/add-dir`** (`src/commands/add-dir/index.ts`)
- Adds directories to context
- Validates directory structure
- Updates file tracking

**`/context`** (`src/commands/context/index.ts`)
- Manages conversation context
- Shows context window usage
- Supports non-interactive mode

---

### MCP and Integration Commands

| Command | Aliases | Type | Description |
|---------|---------|------|-------------|
| `/mcp` | - | local-jsx | MCP server management |
| `/chrome` | - | local-jsx | Chrome integration |
| `/ide` | - | local-jsx | IDE integration |
| `/desktop` | - | local-jsx | Desktop app features |
| `/mobile` | - | local-jsx | Mobile QR code/display |
| `/install-github-app` | - | local-jsx | Install GitHub app |
| `/install-slack-app` | - | local-jsx | Install Slack app |

**Implementation Details:**

**`/mcp`** (`src/commands/mcp/index.ts`)
- Lists MCP servers
- Adds/removes MCP servers
- Manages MCP configurations
- Includes XAA IDP commands

---

### Advanced/Specialized Commands

| Command | Aliases | Type | Description |
|---------|---------|------|-------------|
| `/effort` | - | local-jsx | Set effort level (low/medium/high) |
| `/fast` | - | local | Toggle fast mode |
| `/btw` | - | prompt | Quick note taking |
| `/feedback` | - | local-jsx | Send feedback |
| `/thinkback` | - | local-jsx | Thinkback feature |
| `/thinkback-play` | - | local-jsx | Replay thinkback |
| `/tag` | - | local | Tag management |
| `/usage` | - | local | Show usage information |
| `/version` | - | local | Show version info |
| `/help` | `?`, `h` | local-jsx | Show help |
| `/exit` | - | local | Exit application |

---

### Feature-Gated Commands (Internal/Experimental)

These commands are only available under specific feature flags or for internal users:

| Command | Feature Flag | Type | Description |
|---------|--------------|------|-------------|
| `/proactive` | `PROACTIVE`, `KAIROS` | prompt | Proactive assistance |
| `/brief` | `KAIROS`, `KAIROS_BRIEF` | prompt | Brief mode |
| `/assistant` | `KAIROS` | local-jsx | Assistant features |
| `/bridge` | `BRIDGE_MODE` | local-jsx | Bridge mode commands |
| `/voice` | `VOICE_MODE` | local-jsx | Voice interaction |
| `/workflows` | `WORKFLOW_SCRIPTS` | local-jsx | Workflow management |
| `/web` | `CCR_REMOTE_SETUP` | local-jsx | Remote setup |
| `/torch` | `TORCH` | prompt | Torch feature |
| `/peers` | `UDS_INBOX` | local-jsx | Peer management |
| `/fork` | `FORK_SUBAGENT` | local-jsx | Fork subagent |
| `/buddy` | `BUDDY` | local-jsx | Buddy feature |
| `/ultraplan` | `ULTRAPLAN` | prompt | Advanced planning |
| `/subscribe-pr` | `KAIROS_GITHUB_WEBHOOKS` | prompt | PR subscription |
| `/force-snip` | `HISTORY_SNIP` | prompt | Force history snip |
| `/agents-platform` | (ant users only) | local-jsx | Agents platform |

### Internal-Only Commands

These commands are for Anthropic internal use only (`USER_TYPE === 'ant'`):

- `/backfill-sessions`
- `/break-cache`
- `/bughunter`
- `/commit` (TS version)
- `/commit-push-pr`
- `/ctx_viz`
- `/good-claude`
- `/issue`
- `/init-verifiers`
- `/mock-limits`
- `/bridge-kick`
- `/version`
- `/ultraplan`
- `/subscribe-pr`
- `/reset-limits`
- `/onboarding`
- `/share`
- `/summary`
- `/teleport`
- `/ant-trace`
- `/perf-issue`
- `/env`
- `/oauth-refresh`
- `/debug-tool-call`

---

## Command Type Definition

### TypeScript (src/types/command.ts)

```typescript
// Base properties all commands share
export type CommandBase = {
  name: string
  description: string
  aliases?: string[]
  availability?: CommandAvailability[]  // 'claude-ai' | 'console'
  isEnabled?: () => boolean
  isHidden?: boolean
  version?: string
  whenToUse?: string
  disableModelInvocation?: boolean
  userInvocable?: boolean
  loadedFrom?: 'commands_DEPRECATED' | 'skills' | 'plugin' | 'managed' | 'bundled' | 'mcp'
  kind?: 'workflow'
  immediate?: boolean
  isSensitive?: boolean
}

// Prompt command type
export type PromptCommand = {
  type: 'prompt'
  progressMessage: string
  contentLength: number
  argNames?: string[]
  allowedTools?: string[]
  model?: string
  source: SettingSource | 'builtin' | 'mcp' | 'plugin' | 'bundled'
  context?: 'inline' | 'fork'
  agent?: string
  effort?: EffortValue
  paths?: string[]
  getPromptForCommand(args: string, context: ToolUseContext): Promise<ContentBlockParam[]>
}

// Local command type
export type LocalCommand = {
  type: 'local'
  supportsNonInteractive: boolean
  load: () => Promise<LocalCommandModule>
}

// Local-JSX command type
export type LocalJSXCommand = {
  type: 'local-jsx'
  load: () => Promise<LocalJSXCommandModule>
}

// Combined Command type
export type Command = CommandBase & (PromptCommand | LocalCommand | LocalJSXCommand)
```

### Rust Commands Crate

The Rust commands crate (`rust/crates/commands/src/`) provides a compatibility layer:

**Core Types (lib.rs):**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandManifestEntry {
    pub name: String,
    pub source: CommandSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSource {
    Builtin,
    InternalOnly,
    FeatureGated,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRegistry {
    entries: Vec<CommandManifestEntry>,
}
```

**Available Commands (Rust implementations):**

| Command | File | Description |
|---------|------|-------------|
| `/init` | `init.rs` | Initialize project |
| `/skills` | `skills.rs` | Manage skills |
| `/diff` | `diff.rs` | Git diff |
| `/doctor` | `doctor.rs` | Environment diagnostics |
| `/teleport` | `teleport.rs` | Repository switching |
| `/status` | `status.rs` | Session status |
| `/tasks` | `tasks.rs` | Task management |

---

## Command Dispatch System

### Command Resolution Flow

```
User Input: /command arg1 arg2 --flag=value
                    │
                    ▼
            ┌───────────────┐
            │  Parse Input  │
            └───────────────┘
                    │
                    ▼
            ┌───────────────┐
            │ Find Command  │
            │  (name/alias) │
            └───────────────┘
                    │
                    ▼
            ┌───────────────┐
            │ Check Enabled │
            └───────────────┘
                    │
                    ▼
            ┌───────────────┐
            │Check Availability
            └───────────────┘
                    │
                    ▼
            ┌───────────────┐
            │ Execute Cmd   │
            └───────────────┘
```

### Command Lookup

Commands are looked up by:
1. Primary name (`cmd.name`)
2. User-facing name (`cmd.userFacingName()`)
3. Any alias in `cmd.aliases[]`

### Availability Checking

Commands can specify availability requirements:
- `'claude-ai'`: Available to claude.ai OAuth subscribers
- `'console'`: Available to direct Console API key users

---

## Command Registration

### TypeScript Registration (src/commands.ts)

```typescript
const COMMANDS = memoize((): Command[] => [
  addDir,
  advisor,
  agents,
  branch,
  btw,
  chrome,
  clear,
  // ... ~70 more commands
])
```

### Lazy Loading

Local and Local-JSX commands use lazy loading:

```typescript
const myCommand: LocalCommand = {
  type: 'local',
  supportsNonInteractive: true,
  load: () => import('./commands/my-command/index.js')
}
```

### Dynamic Skills

Skills can be loaded dynamically from:
1. Skill directories (`getSkillDirCommands`)
2. Plugins (`getPluginSkills`)
3. Bundled skills (`getBundledSkills`)
4. Built-in plugin skills (`getBuiltinPluginSkillCommands`)

---

## Command Security

### Bridge Safety

Commands marked as `BRIDGE_SAFE_COMMANDS` can be executed over the Remote Control bridge:
- `/compact`
- `/clear`
- `/cost`
- `/summary`
- `/releaseNotes`
- `/files`

### Remote Mode Safety

Commands in `REMOTE_SAFE_COMMANDS` are available in --remote mode:
- `/session`, `/exit`, `/clear`, `/help`, `/theme`, `/color`
- `/vim`, `/cost`, `/usage`, `/copy`, `/btw`, `/feedback`
- `/plan`, `/keybindings`, `/statusline`, `/stickers`, `/mobile`

### Permission Checking

Commands can specify `isSensitive: true` to redact arguments from history.

---

## Implementing New Commands

### Prompt Command Example

```typescript
// src/commands/my-skill/index.ts
import type { Command, ContentBlockParam } from '../../types/command.js'

const mySkill: Command = {
  type: 'prompt',
  name: 'my-skill',
  description: 'Does something useful',
  aliases: ['ms'],
  progressMessage: 'doing something useful',
  contentLength: 500,
  source: 'builtin',
  allowedTools: ['Bash', 'Read'],
  async getPromptForCommand(args, context) {
    return [
      {
        type: 'text',
        text: `Please perform the following: ${args}`
      }
    ]
  }
}

export default mySkill
```

### Local Command Example

```typescript
// src/commands/my-local/index.ts
import type { Command, LocalCommandResult } from '../../types/command.js'

const myCommand: Command = {
  type: 'local',
  name: 'my-local',
  description: 'Shows some information',
  aliases: ['ml'],
  supportsNonInteractive: true,
  async load() {
    return {
      async call(args, context) {
        return {
          type: 'text',
          value: `You said: ${args}`
        }
      }
    }
  }
}

export default myCommand
```

### Local-JSX Command Example

```typescript
// src/commands/my-jsx/index.ts
import type { Command } from '../../types/command.js'

const myJSXCommand: Command = {
  type: 'local-jsx',
  name: 'my-jsx',
  description: 'Interactive command',
  async load() {
    return {
      async call(onDone, context, args) {
        const { default: MyComponent } = await import('./MyComponent.js')
        return <MyComponent onDone={onDone} args={args} />
      }
    }
  }
}

export default myJSXCommand
```

---

## Testing Commands

### Unit Testing (Rust)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_metadata() {
        let cmd = MyCommand::new();
        let meta = cmd.metadata();
        assert_eq!(meta.name, "my-command");
    }

    #[tokio::test]
    async fn test_command_execution() {
        let cmd = MyCommand::new();
        let ctx = CommandContext::new("/tmp");
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
    }
}
```

---

## Summary

The R.A.D Codicological 2.x Command System provides:

1. **~50 built-in commands** available to all users
2. **20+ feature-gated commands** for experimental/internal features
3. **3 command types**: prompt (skills), local (direct execution), local-jsx (interactive UI)
4. **Dynamic skill loading** from directories, plugins, and MCP servers
5. **Comprehensive security** with bridge safety and permission checking
6. **Lazy loading** for performance optimization
7. **Full TypeScript implementation** with Rust compatibility layer

### Command Count Summary

| Category | Count |
|----------|-------|
| Git Workflow | 4 |
| Review | 3 |
| Session Management | 8 |
| Diagnostics | 6 |
| Configuration | 10 |
| Task & Planning | 5 |
| File & Context | 6 |
| MCP & Integration | 7 |
| Advanced/Specialized | 11 |
| Feature-Gated | 15+ |
| Internal-Only | 20+ |
| **Total** | **~95** |

---

## File Locations

### TypeScript Source
- Main registry: `/mnt/ollama/git/claw-code/src/commands.ts`
- Type definitions: `/mnt/ollama/git/claw-code/src/types/command.ts`
- Command implementations: `/mnt/ollama/git/claw-code/src/commands/*.ts`

### Rust Source
- Compatibility layer: `/mnt/ollama/git/claw-code/rust/crates/commands/src/`
- Command implementations: `lib.rs`, `init.rs`, `skills.rs`, `diff.rs`, `doctor.rs`, `teleport.rs`, `status.rs`, `tasks.rs`, `tests.rs`

---

*Generated: 2026-03-31*
*Version: R.A.D Codicological 2.x*
