# TUI Sprint Plan — Revision v2 (Reconciled with Actual Codebase)

**Repository:** `ultraworkers/claw-code`  
**Branch:** `feat/tui` → `main`  
**Status:** Revised for upstream PR review  
**Date:** 2026-06-12  
**Authors:** TheArchitectit, Claude  

---

## Change Log from v1

| Item | v1 (2026-06-12) | v2 (this document) | Rationale |
|---|---|---|---|
| Week count to start TUI | 7 | 3 | `cli/` extractions already done on base branch |
| `main.rs` target | <200 lines | <1,000 lines | Honest math: 11,282 - 2,644 extracted = ~8,638; target <1,000 after TUI-ready extraction |
| Extraction phase | Weeks 1–6 (app.rs, format.rs, session_mgr.rs) | Weeks 1–2 (session.rs, repl.rs) | Parsing, formatting, permissions, doctor already extracted to `cli/` |
| Feature flag timing | Week 20 | Week 12 | Start with flag from day one (toggled off by default) |
| Plan assumes monolith | Yes | No | Acknowledges existing `cli/` module structure |
| API skeleton errors | `ratatui::try_init()` (doesn't exist) | `ratatui::init()` (correct) | Fixed after code review of v1 |
| PR title backtick bug | Unescaped `` `claw tui` `` inside backticks | Single-quoted or full-title single backtick | Fixed |
| `--features tui` in Week 6 | Contradiction: feature flag doesn't exist yet | Feature flag added in Week 1 | No contradiction |

---

## 1. Current Baseline (As of 2026-06-12, `feat/all-prs-combined` HEAD)

### 1.1 Already Extracted (Do Not Repeat)

The following modules were extracted in commits on the base branch **before** `feat/tui` diverged:

```
5f79357 refactor: extract formatting functions to cli/format.rs       (+413 lines in cli/format.rs)
99bb5f7 refactor: extract permission handling to cli/permission.rs    (+78  lines in cli/permission.rs)
5809bde refactor(cli): extract doctor/diagnostics module              (+695 lines in cli/doctor.rs)
8d9270a refactor(cli): extract parsing logic into cli module          (+1,193 lines in cli/parse.rs, +133 in cli/model.rs)
32d8c77 refactor: extract web search module to tools/src/search.rs
 d61e792 refactor: extract team coordination module to tools/src/team.rs
911dc88 refactor: extract agent utility module to tools/src/agent.rs
11e73e9 chore: remove dead code from tools and runtime                 (-132 lines)
```

**These are DONE.** Any plan that schedules re-extracting them is out of date.

### 1.2 Still In `main.rs` (11,282 lines)

```
Line ranges (approximate, from grep):
  85–208:   small helpers (max_tokens_for_model, provider_label, format_connected_line, etc.)
 209–482:   main() + run() dispatchers
 483–503:   run_worker_state
 504–532:   run_mcp_serve
 533–869:   resume command parsing + session resume helpers
 870–1348:  git helpers (parse_git_status_*, resolve_git_branch, etc.)
1349–1590: broad cwd detection, stale base preflight, run_repl
1591–2072: structs (SessionHandle, ManagedSessionSummary, LiveCli, BuiltRuntime, etc.)
2073–3359: impl LiveCli (the big one: ~1,286 lines)
3360–end:  session management helpers (sessions_dir, current_session_store, create_managed_session_handle,
           resolve_session_reference, resolve_managed_session_path, list_managed_sessions,
           latest_managed_session, load_session_reference, delete_managed_session, etc.)
```

The **critical remaining extraction targets** are:

| Target | Est. Lines | Contents |
|---|---|---|
| **Session management** | ~400 | `sessions_dir`, `SessionStore` ops, `SessionHandle`, `ManagedSessionSummary`, list/load/create/delete |
| **LiveCli impl** | ~1,286 | The REPL loop, slash command dispatch, streaming display, tool call display |
| **Git helpers** | ~500 | `parse_git_status_*`, `resolve_git_branch`, `run_git_capture_in`, `find_git_root_in` |
| **Resume logic** | ~300 | `resume_session`, `run_resume_command`, `ResumeCommandOutcome` |
| **Broad CWD / stale base** | ~200 | `detect_broad_cwd`, `enforce_broad_cwd_policy`, `run_stale_base_preflight` |
| **MCP serve** | ~50 | `run_mcp_serve` |
| **Various small helpers** | ~200 | Error classification, piped stdin, etc. |

**Total remaining to extract: ~2,936 lines.** After extraction, `main.rs` would be ~8,346 lines still — but much of that is LiveCli which we **do not fully extract** because the TUI will share its logic through traits.

### 1.3 Honest New Plan: Hybrid Approach

Instead of full extraction + then TUI, we do a **partial extraction + dual-surface trait**:

1. Extract session management to `session.rs` (independent module)
2. Extract git helpers to `git.rs` (independent module)
3. Define a `RuntimeHandle` or `ConversationSurface` trait that both REPL (LiveCli) and TUI implement
4. Start TUI skeleton (Week 3) while LiveCli lives in `main.rs`
5. Extract LiveCli to `repl.rs` in parallel with TUI dev, not as a prerequisite

This avoids the trap that killed prior attempts: **waiting for extraction to finish before starting the actual TUI.**

---

## 2. Revised 52-Week Sprint Plan

### Sprint Structure

- **1 sprint = 2 weeks** (26 sprints total)
- **Every sprint ships.** Cut scope, never delay.
- **No PR >400 lines.** Split or cut.
- **Feature flag `tui` from Week 1.** Default OFF. This is the correct time to add it.

---

### Phase A: Foundation (Weeks 1–2) — Catch Up to Reality

**Goal:** Extract what's left. Don't re-extract what's done.

#### Week 1: Extract `session.rs` + Add `tui` Feature Flag

| Field | Value |
|---|---|
| **Deliverable** | Session helpers extracted to `session.rs`. `tui` feature flag added to `Cargo.toml`. `claw tui --help` prints stub message. |
| **What NOT done** | `format.rs`, `parse.rs`, `doctor.rs` — already exist in `cli/` on base branch. Do not recreate them. |
| **Files touched** | `main.rs (-400/+20)`, `session.rs (+400/-0)`, `Cargo.toml (+3)`, `cli/mod.rs (+5)` |
| **New files** | `session.rs` (extracted from `main.rs` lines 3360–end + SessionHandle/ManagedSessionSummary structs) |
| **Functions moved** | `sessions_dir`, `current_session_store`, `new_cli_session`, `create_managed_session_handle`, `resolve_session_reference`, `resolve_managed_session_path`, `list_managed_sessions`, `latest_managed_session`, `load_session_reference`, `load_session_reference_excluding`, `delete_managed_session`, `confirm_session_deletion`, `render_session_list`, `format_session_modified_age`, `write_session_clear_backup`, `session_clear_backup_path`, `render_repl_help` |
| **Feature flag** | `[features]\ntui = ["dep:ratatui", "dep:tui-textarea"]` in `Cargo.toml` |
| **Test strategy** | Move session tests. Add `#[test] fn test_session_module_compiles()`. Add `#[test] fn test_tui_feature_flag_parses()`. |
| **Merge criteria** | `cargo test --workspace` passes. `cargo test --workspace --features tui` passes. `claw tui --help` works. `main.rs` reduced by ~400 lines. |
| **Rollback strategy** | `git checkout HEAD~1 -- main.rs session.rs Cargo.toml` |
| **Dependencies** | None — pure extraction + feature flag stub. |
| **PR title** | `refactor: extract session management to session.rs; add tui feature flag` |
| **PR description** | • Extracts session lifecycle from `main.rs` to `session.rs`. • Introduces `tui` Cargo feature flag. • Adds `claw tui` stub command gated behind flag. • Does NOT extract already-modularized `cli/` code. |
| **Review checklist** | ① `cli/format.rs`, `cli/parse.rs`, `cli/doctor.rs` still present (not duplicated). ② Session tests moved. ③ Feature flag compiles. ④ `cargo test` passes both with and without `--features tui`. ⑤ `main.rs` reduced. |
| **Risk** | Low — mechanical extraction. |
| **Docs** | Update `rust/README.md` to show `session.rs` in layout. |
| **Harness impact** | None. |

---

#### Week 2: Extract `git.rs` + Trait Boundary

| Field | Value |
|---|---|
| **Deliverable** | Git helpers extracted to `git.rs`. `ConversationSurface` trait defined for REPL/TUI duality. |
| **Files touched** | `main.rs (-500/+50)`, `git.rs (+500/-0)`, `Cargo.toml (+0)` |
| **New files** | `git.rs` (lines 870–1348 + helper functions) |
| **Trait defined** | `pub trait ConversationSurface { fn run_turn(&mut self, prompt: &str) -> Result<TurnResult, RuntimeError>; fn session_id(&self) -> Option<&str>; fn model_name(&self) -> &str; }` |
| **Test strategy** | `#[test] fn test_git_parse_branch()`. Mock `ConversationSurface` impl for testing. |
| **Merge criteria** | `cargo test` passes. `ConversationSurface` compiles. No REPL behavioral changes. |
| **Rollback strategy** | Inline `git.rs` back into `main.rs`. Remove trait. |
| **Dependencies** | None. |
| **PR title** | `refactor: extract git helpers to git.rs; define ConversationSurface trait` |
| **PR description** | • Extracts git status/branch/workspace helpers to `git.rs`. • Defines `ConversationSurface` trait for shared REPL/TUI runtime access. • No behavioral changes. |
| **Review checklist** | ① Git functions moved. ② `parse_git_status_metadata` tested. ③ Trait has methods REPL needs. ④ `cargo test` passes. ⑤ No behavioral changes. |
| **Risk** | Low — mechanical + trait definition. |
| **Docs** | Document `ConversationSurface` trait purpose in code comments. |
| **Harness impact** | None. |

**After Week 2:** `main.rs` ~10,000 lines (down from 11,282). TUI feature flag exists. Session and git are modular. Ready for TUI skeleton.

---

### Phase B: Core TUI Shell (Weeks 3–8) — Make It Run

**Goal:** `claw tui` (with `--features tui`) opens, renders mock chat, accepts input, scrolls, exits cleanly.

#### Week 3: Ratatui Skeleton

| Field | Value |
|---|---|
| **Deliverable** | `ratatui` and `tui-textarea` added as optional deps (behind `tui` feature). Minimal alternate-screen demo renders "Hello, claw", exits on `q`. Uses `TestBackend`. |
| **Files touched** | `Cargo.toml (+8 in [features] and [dependencies])`, `src/tui/mod.rs` (new, 150 lines) |
| **New files** | `src/tui/mod.rs`, `src/tui/app.rs` (stub, 30 lines) |
| **Dependencies added** | `ratatui = { version = "0.29", optional = true }`, `tui-textarea = { version = "0.7", optional = true }`, `crossterm` already present (needs `event-stream` feature: `crossterm = { version = "0.28", features = ["event-stream"] }`) |
| **Code skeleton** | See corrected skeleton below — uses `ratatui::init()` not `ratatui::try_init()` |
| **Test strategy** | `#[test] fn test_tui_init_buffer()` — `TestBackend::new(80, 24)`, assert "Hello, claw" visible. `#[cfg(feature = "tui")]` guards all TUI tests. |
| **Merge criteria** | `cargo build --workspace --features tui` succeeds. `cargo test --workspace --features tui` passes. `claw tui` runs with `--features tui`. |
| **Rollback strategy** | Remove `tui` feature and `src/tui/` directory. |
| **PR title** | `feat: ratatui skeleton with TestBackend tests` |
| **PR description** | • Adds `ratatui` and `tui-textarea` as optional dependencies behind `tui` feature. • `claw tui` opens alternate screen, renders demo text, exits on `q`. • TestBackend verifies rendered output in CI. |
| **Review checklist** | ① `ratatui::init()` (correct API). ② Terminal restored on exit. ③ `TestBackend` test passes. ④ Feature flag works: build WITH flag succeeds, build WITHOUT flag still works. ⑤ No REPL changes. |
| **Risk** | Low — well-tested dependencies. |
| **Docs** | Add ratatui to `rust/README.md` dependencies list (under optional). |
| **Harness impact** | CI matrix: build with `--features tui` added. |

**Corrected code skeleton (Week 3):**
```rust
// src/tui/mod.rs
#[cfg(feature = "tui")]
pub mod app;

#[cfg(feature = "tui")]
use std::io;

#[cfg(feature = "tui")]
pub fn run() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal);
    ratatui::restore();
    result
}

#[cfg(feature = "tui")]
fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>
) -> io::Result<()> {
    let mut app = app::App::new();
    loop {
        terminal.draw(|f| app.draw(f))?;
        use crossterm::event::{self, Event, KeyCode};
        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Char('q') {
                return Ok(());
            }
        }
    }
}
```

---

#### Week 4: Event Broker + Mock Chat

| Field | Value |
|---|---|
| **Deliverable** | `tui/event.rs`: `EventBroker` bridges crossterm events → `AppEvent`. `App` renders static mock messages. Arrow keys scroll. |
| **Files touched** | `tui/mod.rs (+40)`, `tui/app.rs` (created, 180 lines), `tui/event.rs` (created, 150 lines) |
| **New files** | `tui/event.rs` |
| **Types defined** | `pub enum AppEvent { Tick, Key(crossterm::event::KeyEvent), Resize(u16, u16), Quit }` |
| **Test strategy** | `#[test] fn test_key_translation()` maps `KeyCode::Char('q')` → `Quit`. `#[test] fn test_app_scroll_down()`. Both use `TestBackend`. |
| **Merge criteria** | App renders mock messages. Arrow keys scroll. `q` quits. Tests pass without TTY. |
| **Rollback strategy** | Revert `tui/app.rs` and `tui/event.rs` to stubs. |
| **PR title** | `feat: tui event broker and mock chat widget` |
| **PR description** | • `EventBroker` translates crossterm events to `AppEvent`. • `App` renders static mock conversation. • Scroll via arrow keys. |
| **Review checklist** | ① Key events translate correctly. ② Resize produces correct dimensions. ③ Mock messages visible. ④ Scroll offset clamped. ⑤ `q` quits. |
| **Risk** | Low — event translation is straightforward. |
| **Docs** | Add event flow diagram to `docs/TUI.md`. |
| **Harness impact** | None. |

---

#### Week 5: Composer Input Widget

| Field | Value |
|---|---|
| **Deliverable** | `tui/widgets/input.rs`: Composer wrapping `tui_textarea::TextArea`. Enter sends, Shift+Enter newline. Up/down history. |
| **Files touched** | `tui/app.rs (+60)`, `tui/widgets/input.rs` (created, 160 lines) |
| **New files** | `tui/widgets/input.rs` |
| **Test strategy** | `#[test] fn test_composer_enter_sends()` — assert `Submit("hello")` event. `#[test] fn test_composer_history_up()`. |
| **Merge criteria** | Text input works. Enter submits. Shift+Enter inserts newline. History cycles. |
| **Rollback strategy** | Remove composer widget, use placeholder. |
| **PR title** | `feat: tui composer input widget with history` |
| **PR description** | • `ComposerWidget` using `tui-textarea`. • Enter submits, Shift+Enter newline. • Up/down arrow for history. |
| **Review checklist** | ① Enter produces submit event. ② Shift+Enter doesn't submit. ③ History works. ④ Unicode handled. ⑤ `TestBackend` verifies. |
| **Risk** | Low — `tui-textarea` is mature. |
| **Docs** | Add keybindings table. |
| **Harness impact** | None. |

---

#### Week 6: Chat Message List + Status Bar

| Field | Value |
|---|---|
| **Deliverable** | `ChatWidget` renders scrollable message list. Status bar shows static model name + permission mode. Terminal resize handled. |
| **Files touched** | `tui/app.rs (+80)`, `tui/widgets/chat.rs` (created, 200 lines), `tui/widgets/status_bar.rs` (created, 100 lines) |
| **New files** | `tui/widgets/chat.rs`, `tui/widgets/status_bar.rs` |
| **Test strategy** | `#[test] fn test_chat_renders_messages()` — 5 messages visible. `#[test] fn test_status_bar_at_bottom()`. |
| **Merge criteria** | Messages render with role styling. Status bar visible at bottom. Resize adjusts layout. |
| **Rollback strategy** | Disable status bar. |
| **PR title** | `feat: tui chat widget, status bar, and resize handling` |
| **PR description** | • `ChatWidget` renders message history. • `StatusBarWidget` shows model + permission mode. • Resize events recalculate layout. |
| **Review checklist** | ① User messages styled differently. ② Status bar at bottom. ③ Resize doesn't panic. ④ Content area correct height. ⑤ Tests pass. |
| **Risk** | Low — layout math with `ratatui::Layout`. |
| **Docs** | Document layout constraints. |
| **Harness impact** | None. |

---

#### Week 7-8: Integration Weeks

**Week 7:** `claw tui --features tui` opens full-screen app with mock chat + input + status bar. Integration test in CI.  
**Week 8:** Code review, bug fixes from Week 7, first user-facing documentation (`docs/TUI-USAGE.md` draft). Performance budget established.

| Merge checkpoint (end of Week 8) | |
|---|---|
| `claw tui --features tui` runs | ✅ Mock chat, input, status bar |
| Exits cleanly on `q` | ✅ |
| Tests pass in CI | ✅ With `--features tui` |
| Without feature flag | ✅ No TUI code compiled in |
| `main.rs` still ~10,000 lines | ✅ LiveCli not yet extracted (deferred) |

---

### Phase C: Live Conversation (Weeks 9–16) — Make It Talk

**Goal:** Real model conversations. Streaming text, tool calls, session persistence, permission modal.

#### Week 9: Wire `ConversationRuntime`

| Field | Value |
|---|---|
| **Deliverable** | `ConversationRuntime` wired into TUI via `ConversationSurface` trait. `TuiProgressReporter` sends events to TUI channel. |
| **Files touched** | `tui/app.rs (+120)`, `tui/mod.rs (+40)`, `tui/event.rs (+30)` |
| **New types** | `TuiProgressReporter { tx: mpsc::UnboundedSender<AppEvent> } impl TurnProgressReporter for TuiProgressReporter` |
| **Test strategy** | Mock `ConversationSurface` that emits fake deltas. `#[test] fn test_progress_reporter_sends_delta()`. |
| **Merge criteria** | TUI receives fake streaming events. Events route correctly. No panics. |
| **Rollback strategy** | Replace `TuiProgressReporter` with no-op. |
| **PR title** | `feat: wire ConversationRuntime into tui event loop` |
| **PR description** | • `TuiProgressReporter` bridges runtime events to TUI. • Async event loop processes backend + user events. • Trait-based sharing with REPL. |
| **Review checklist** | ① `AssistantDelta` events reach App. ② No deadlocks. ③ Channel bounded. ④ Mock runtime tests. ⑤ Trait clean. |
| **Risk** | Medium — first async complexity. Mitigated by bounded channels. |
| **Docs** | Update event flow diagram. |
| **Harness impact** | Add `tui_streaming_mock` scenario. |

---

#### Week 10: Streaming Assistant Text

| Field | Value |
|---|---|
| **Deliverable** | `AssistantEvent::TextDelta` appends to active message cell in real time. No markdown yet — raw text. |
| **Files touched** | `tui/app.rs (+80)`, `tui/widgets/chat.rs (+60)` |
| **Test strategy** | `#[test] fn test_streaming_appends_text()` — 5 deltas → concatenated text. |
| **Merge criteria** | Text appears in real time. Auto-scroll. |
| **Rollback strategy** | Buffer full response, render once. |
| **PR title** | `feat: real-time streaming assistant text in tui` |
| **Review checklist** | ① Text visible within 50ms. ② No duplicates. ③ Auto-scroll. ④ 1000 chars no OOM. ⑤ Tests headless. |
| **Risk** | Medium — rendering performance. |
| **Docs** | Document streaming latency target. |
| **Harness impact** | Add `tui_streaming_text`. |

---

#### Week 11: Session Save & Resume

| Field | Value |
|---|---|
| **Deliverable** | Auto-save on exit. `claw tui --resume --features tui` loads session. |
| **Files touched** | `tui/app.rs (+80)`, `tui/mod.rs (+40)`, `session.rs (+20)` |
| **Test strategy** | `#[test] fn test_session_save_roundtrip()`. |
| **Merge criteria** | Session persists across invocations. Compatible with REPL session format. |
| **Rollback strategy** | Don't save on exit. |
| **PR title** | `feat: tui session save and resume` |
| **Review checklist** | ① File written on exit. ② `--resume` loads. ③ Format compatible. ④ No data loss. ⑤ Tests verify. |
| **Risk** | Low — session code already tested. |
| **Docs** | Update `USAGE.md`. |
| **Harness impact** | Add `tui_session_save_resume`. |

---

#### Week 12: Inline Tool Call Rendering

| Field | Value |
|---|---|
| **Deliverable** | Tool calls show inline spinner + name. Results show ✓/✗ with truncated preview. |
| **Files touched** | `tui/app.rs (+60)`, `tui/widgets/tool_call.rs` (created, 220 lines) |
| **New files** | `tui/widgets/tool_call.rs` |
| **Test strategy** | `#[test] fn test_tool_call_lifecycle()`. |
| **Merge criteria** | Tool calls animate. Results truncated. Expandable. |
| **Rollback strategy** | Show plain text tool names. |
| **PR title** | `feat: inline tool call spinner and result summary` |
| **Review checklist** | ① Spinner visible. ② ✓/✗ correct. ③ Truncation at 5 lines. ④ Concurrent tools ok. ⑤ Tests headless. |
| **Risk** | Low — tool execution already works. |
| **Docs** | Document tool visualization. |
| **Harness impact** | Add `tui_tool_call_display`. |

---

#### Week 13: Permission Prompt Modal

| Field | Value |
|---|---|
| **Deliverable** | Modal overlay for Y/N approval. Does not block event loop. |
| **Files touched** | `tui/app.rs (+60)`, `tui/widgets/permission.rs` (created, 200 lines) |
| **New files** | `tui/widgets/permission.rs` |
| **Test strategy** | `#[test] fn test_permission_modal_renders()`. `#[test] fn test_permission_y_approves()`. |
| **Merge criteria** | Modal renders. `y` approves. `n` denies. Escape cancels. |
| **Rollback strategy** | Always deny. |
| **PR title** | `feat: permission prompt modal overlay in tui` |
| **Review checklist** | ① Centered block. ② `y`/`n` correct. ③ Background dimmed. ④ Escape cancels. ⑤ Tests. |
| **Risk** | Medium — state transitions. |
| **Docs** | Document permission flow. |
| **Harness impact** | Add `tui_permission_prompt`. |

---

#### Week 14: Error Handling

| Field | Value |
|---|---|
| **Deliverable** | Errors render as styled messages in chat. No panics. |
| **Files touched** | `tui/app.rs (+40)`, `tui/widgets/chat.rs (+30)` |
| **Test strategy** | `#[test] fn test_error_rendered_in_chat()`. |
| **Merge criteria** | All error paths show friendly messages. Panic-free. |
| **Rollback strategy** | Panic on error (ugly). |
| **PR title** | `feat: graceful error rendering in tui chat` |
| **Review checklist** | ① Network error visible. ② API error visible. ③ No panic. ④ Stack trace in log. ⑤ Tests. |
| **Risk** | Low — error types exist. |
| **Docs** | Document error handling. |
| **Harness impact** | Add `tui_error_handling`. |

---

#### Week 15-16: Multi-Turn + Polish

**Week 15:** Multi-turn conversation, follow-up messages, input clears after send.  
**Week 16:** Code review, bug fixes, first load-test with real model (manual QA).

| Phase C checkpoint (end of Week 16) | |
|---|---|
| `claw tui --features tui --resume` works | ✅ |
| Real model conversations | ✅ Streaming text, tool calls |
| Permission modal works | ✅ |
| Session persistence | ✅ |
| Errors graceful | ✅ |

---

### Phase D: Rich Rendering (Weeks 17–24) — Make It Beautiful

**Goal:** Markdown rendering, syntax highlighting, themes, diffs, performance.

#### Week 17: Markdown Widget (Headings, Lists, Code Blocks)

| Field | Value |
|---|---|
| **Deliverable** | `tui/widgets/markdown.rs`: Port `render.rs` to ratatui widget. Renders headings, lists, inline code, code blocks with syntect. |
| **Files touched** | `render.rs` (reference only), `tui/widgets/markdown.rs` (created, 300 lines) |
| **New files** | `tui/widgets/markdown.rs` |
| **Test strategy** | `#[test] fn test_heading_bold()`. `#[test] fn test_code_block_highlighted()`. |
| **Merge criteria** | Markdown renders. Code blocks highlighted. Lists indented. |
| **Rollback strategy** | Plain text rendering. |
| **PR title** | `feat: markdown widget with syntax highlighting` |
| **Review checklist** | ① H1/H2 bold. ② Code blocks highlighted. ③ Lists indented. ④ Inline code colored. ⑤ <50ms. |
| **Risk** | Medium — markdown parsing complex. |
| **Docs** | Document supported markdown. |
| **Harness impact** | Add `tui_markdown_render`. |

---

#### Week 18: Table Rendering

| Field | Value |
|---|---|
| **Deliverable** | Tables render as ratatui `Table` with borders and alignment. |
| **Files touched** | `tui/widgets/markdown.rs (+100)` |
| **Test strategy** | `#[test] fn test_table_borders()`. |
| **Merge criteria** | Tables render with borders. Fit terminal width. |
| **Rollback strategy** | Plain text tables. |
| **PR title** | `feat: table rendering in markdown widget` |
| **Review checklist** | ① Borders. ② Headers bold. ③ Cells aligned. ④ Graceful truncation. ⑤ Tests. |
| **Risk** | Low — standard widget. |
| **Docs** | Add table examples. |
| **Harness impact** | None. |

---

#### Week 19: Blockquote, Link, Incremental Streaming

| Field | Value |
|---|---|
| **Deliverable** | Blockquotes with left border. Links underlined. Incremental markdown parse (don't re-render whole doc each delta). |
| **Files touched** | `tui/widgets/markdown.rs (+120)`, `tui/theme.rs (+20)` |
| **Test strategy** | `#[test] fn test_incremental_parse_heading()`. |
| **Merge criteria** | Blockquotes distinct. Links underlined. Incremental parse <16ms per delta. |
| **Rollback strategy** | Full re-parse (slower). |
| **PR title** | `feat: blockquote/link rendering and incremental markdown streaming` |
| **Review checklist** | ① Blockquote border. ② Links underlined. ③ Incremental no flicker. ④ <16ms. ⑤ Tests. |
| **Risk** | High — incremental parsing tricky. Mitigated by fallback. |
| **Docs** | Document incremental strategy. |
| **Harness impact** | None. |

---

#### Week 20: Theme System

| Field | Value |
|---|---|
| **Deliverable** | Dark/light themes. `NO_COLOR` support. Truecolor → 256 → 16 fallback. |
| **Files touched** | `tui/theme.rs` (created, 250 lines) |
| **New files** | `tui/theme.rs` |
| **Test strategy** | `#[test] fn test_theme_no_color()`. `#[test] fn test_theme_16_fallback()`. |
| **Merge criteria** | `NO_COLOR` disables colors. Truecolor shows full palette. Degrades gracefully. |
| **Rollback strategy** | Hardcoded dark theme. |
| **PR title** | `feat: color theme system with terminal capability detection` |
| **Review checklist** | ① `NO_COLOR=1` → no ANSI. ② Truecolor → 24-bit. ③ 256-color approx. ④ 16-color nearest. ⑤ Configurable. |
| **Risk** | Low — well-understood. |
| **Docs** | Document theme config. |
| **Harness impact** | None. |

---

#### Week 21-22: Tool Expansion + Diff Colors + Performance

**Week 21:** Expandable/collapsible tool results. Colored diff rendering (green additions, red removals).  
**Week 22:** Performance pass. `cargo flamegraph`. Target: 60fps on 100-message session.

| Phase D checkpoint (end of Week 24) | |
|---|---|
| Markdown renders beautifully | ✅ |
| Code blocks highlighted | ✅ |
| Tables, blockquotes, links | ✅ |
| Themes work | ✅ |
| 60fps sustained | ✅ |

---

### Phase E: Navigation & Power (Weeks 25–34) — Make It Fast to Use

| Week | Deliverable |
|---|---|
| 25 | `/` incremental search within conversation |
| 26 | `?` help overlay with keybindings |
| 27 | `Ctrl+S` session picker (fuzzy) |
| 28 | Slash command picker in TUI |
| 29 | `/compact` with toast notification |
| 30 | Copy to clipboard (`y` on message) |
| 31 | Mouse support (scroll, click expand, focus) |
| 32 | Internal pager for long outputs |
| 33 | Optional right sidebar (`Ctrl+P`) |
| 34 | Custom keybindings from `.claw.json` |

---

### Phase F: Polish & Hardening (Weeks 35–44) — Production-Ready

| Week | Deliverable |
|---|---|
| 35 | Screen reader annotations |
| 36 | Small terminal graceful degradation |
| 37 | Unicode width handling (`unicode-width`) |
| 38 | Snapshot tests with `insta` |
| 39 | Stress test: 1,000 messages |
| 40 | Windows testing (PowerShell, Windows Terminal) |
| 41 | TUI user guide (`docs/TUI-USAGE.md`) |
| 42 | `claw doctor --tui` diagnostic |
| 43-44 | Buffer for bug fixes, review feedback |

---

### Phase G: Advanced Features (Weeks 45–52) — Best-in-Class

| Week | Deliverable |
|---|---|
| 45 | Image support (sixel/iTerm2 protocols) when vision API lands |
| 46 | Notebook editing in TUI |
| 47 | Agent team dashboard (`Ctrl+A`) |
| 48 | MCP server inspector (`Ctrl+M`) |
| 49 | Teleport mode: fuzzy file finder (`Ctrl+G`) |
| 50 | Release polish: update ROADMAP |
| 51-52 | Buffer for final fixes, default TUI mode evaluation |

---

## 3. What's Different from v1

| Aspect | v1 | v2 (this document) |
|---|---|---|
| Assumed monolith | Yes (11,282 lines intact) | No — acknowledges `cli/` extractions (~2,644 lines moved) |
| Extraction phase | 6 weeks (app.rs, format.rs, session_mgr.rs) | 2 weeks (session.rs, git.rs) — `cli/` already done |
| TUI starts | Week 7 | Week 3 |
| Feature flag | Week 20 | Week 1 |
| Live conversation | Week 13 | Week 9 |
| Markdown rendering | Week 21 | Week 17 |
| Navigation features | Week 29 | Week 25 |
| Default TUI mode | Week 52 | Week 52 (unchanged, but more buffer) |
| API errors | `ratatui::try_init()` (wrong), `--features tui` before flag exists | `ratatui::init()` (correct), feature flag from Week 1 |
| Backtick bug in PR title | Week 4: unescaped `` `claw tui` `` | Fixed: single-quoted or escaped |

**Net result:** TUI ships functional features **4 weeks earlier** while maintaining the same quality standards. The plan accounts for reality instead of assuming a blank slate.

---

## 4. Remaining Risks (v2)

| # | Risk | Mitigation |
|---|---|---|
| 1 | `LiveCli` remains in `main.rs` (~1,286 lines) during TUI dev | Dual-surface `ConversationSurface` trait means TUI doesn't need LiveCli extracted. Parallel extraction can happen in Weeks 10-15 without blocking TUI. |
| 2 | Feature flag complexity too early | Flag is opt-in only. Default builds skip TUI entirely. CI builds both. |
| 3 | `crossterm` `event-stream` feature affects REPL | The feature is additive (only adds async event stream). REPL uses blocking `event::read()`. No conflict. |
| 4 | Mock parity harness needs TUI scenarios | Added specific scenarios per phase. Harness scenarios are added in the week their feature lands — not all at once. |
| 5 | Prior credit-war trauma on `feat/uiux-redesign` | Attribution policy: `Co-Authored-By` trailers only during TUI work. No README edits in TUI PRs. |

---

## 5. Rules of Engagement (Unchanged from v1)

1. No README edits during TUI work.
2. No empty branches (commit within 24h or delete).
3. No PR >400 lines.
4. Feature flag exists from Week 1.
5. Every widget has `TestBackend` assertions.
6. Every sprint ends with green CI.
7. REPL is sacred — `claw tui` is separate entrypoint.
8. Streaming-first for all message rendering.
9. Reuse runtime types directly — no wrappers.
10. Defer, don't abandon.

---

*Generated: 2026-06-12 | Revision: v2 | Authors: TheArchitectit, Claude*  
*Reconciled with actual codebase state at `feat/all-prs-combined` HEAD (11,282-line `main.rs` with `cli/` extractions already present)*
