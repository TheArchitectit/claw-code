# TUI Sprint Plan — Proposal for Upstream Review (Revision v3)

**Repository:** `ultraworkers/claw-code`  
**Proposed Branch:** `feat/tui` → `main`  
**Status:** Proposal for upstream maintainers  
**Date:** 2026-06-12  
**Author(s):** TheArchitectit  

---

## Important Note on Scope

This is a **proposal** for a downstream contributor planning to land TUI support in `ultraworkers/claw-code`. The maintainers of the upstream repository retain full authority over merge criteria, PR size limits, release scheduling, and feature priorities. This document is written as a *request for comment* — not as a set of mandates.

The suggested practices below (e.g., "aim for ≤400-line PRs") are **recommendations** for reviewability. They are presented as trade-offs, not rules. The upstream maintainer may accept, modify, or reject any part of this proposal.

---

## 1. Current Baseline (As-Is)

The `rusty-claude-cli/src/main.rs` file is **~11,282 lines**. Prior TUI attempts (`feat/ui-hardening`, `fix/ui-parity`, `feat/uiux-redesign`, `rcc/ui-polish`) all stalled. Post-mortem analysis identified three common causes:

1. **Attempting to build a TUI without first modularizing the CLI surface.** The REPL, formatting, diagnostics, and session logic were deeply interleaved.
2. **Credit-warfare on `feat/uiux-redesign`** (18 README-only commits) distracted from feature work.
3. **The TUI plan (`rust/TUI-ENHANCEMENT-PLAN.md`) had no owner or branch** — it was a wishlist, not a tracked effort.

**Partial relief already exists:** `cli/parse.rs`, `cli/doctor.rs`, `cli/format.rs`, `cli/permission.rs`, and `cli/model.rs` have already been extracted from `main.rs` on the base branch. This removes ~2,644 lines of complexity from the problem.

**What remains in `main.rs`:**
- `LiveCli` struct + impl (~1,286 lines of dispatch, streaming display, tool rendering, REPL loop)
- Session lifecycle helpers (~400 lines)
- Git workspace helpers (~500 lines)
- `run()` dispatcher and subcommand routing (~200 lines)
- Small helpers for errors, stdin, model resolution, etc.

---

## 2. Proposed Approach

Rather than demanding full extraction before TUI work begins (the trap that killed prior attempts), we propose a **dual-surface trait approach**:

1. **Extract remaining helpers** (session, git) into their own modules. These are mechanical moves with zero behavioral change.
2. **Define a `ConversationSurface` trait** that both the existing REPL (`LiveCli`) and the new TUI can implement. This allows the TUI to reuse the runtime without needing `main.rs` to be fully gutted first.
3. **Build the TUI as an opt-in feature** behind a `tui` Cargo feature flag. Without the flag, no TUI code is compiled — the REPL and all existing surfaces are untouched.
4. **Extract `LiveCli` to `repl.rs` in parallel** with TUI development, not as a prerequisite. The trait boundary means neither surface blocks the other.

### Why this is different from prior attempts

| Prior Attempt | Why It Failed | How This Proposal Avoids It |
|---|---|---|
| `feat/ui-hardening` | Patched behavior inside `main.rs` without structural change | TUI is a separate module, `main.rs` untouched except for trait import |
| `fix/ui-parity` | Scoped DOWN from TUI; never started extraction | TUI skeleton starts in Week 3; extraction happens in parallel |
| `feat/uiux-redesign` | README credit wars consumed branch history | All TUI PRs keep attribution in commit trailers only |
| `rcc/ui-polish` | Empty branch; waited for another branch | Every branch has a visible change within 24 hours |
| `TUI-ENHANCEMENT-PLAN.md` | Plan without owner or branch | Tracked on `feat/tui` branch, weekly deliverables |

---

## 3. Proposed Crate Additions

| Crate | Version | Feature-Gated? | Purpose |
|---|---|---|---|
| `ratatui` | `0.29` | Yes (`tui` feature) | TUI framework — layout, widgets, rendering |
| `tui-textarea` | `0.7` | Yes (`tui` feature) | Multi-line text input with history and wrapping |
| `unicode-width` | `0.2` | Yes (`tui` feature) | Correct column counting for CJK and emoji |
| `arboard` | `3` | Yes (`tui` feature, Phase F) | Cross-platform clipboard for copy-to-clipboard |
| `insta` | `1` | Dev-only (`dev-dependencies`) | Snapshot testing for rendered TUI output |

**Proposed `Cargo.toml` addition:**

```toml
[features]
default = []
tui = ["dep:ratatui", "dep:tui-textarea"]

[dependencies]
# ... existing deps ...
ratatui = { version = "0.29", optional = true }
tui-textarea = { version = "0.7", optional = true }
unicode-width = { version = "0.2", optional = true }

[dev-dependencies]
insta = "1"
```

Without `--features tui`, the binary size, compile time, and dependency footprint are **unchanged**.

---

## 4. Proposed Module Layout

```
rust/crates/rusty-claude-cli/src/
├── main.rs              # Entrypoint (~200 lines). Imports cli, session, git.
│                        # Dispatches to REPL or TUI based on args and feature flag.
│
├── cli/                 # ALREADY MODULARIZED — do not touch
│   ├── mod.rs
│   ├── doctor.rs
│   ├── format.rs
│   ├── model.rs
│   ├── parse.rs
│   └── permission.rs
│
├── session.rs           # NEW: Extracted from main.rs. Session CRUD.
├── git.rs               # NEW: Extracted from main.rs. Git helpers.
├── repl.rs              # NEW (Week 10+): LiveCli moves here. REPL surface.
│
├── init.rs              # Repo initialization (existing, unchanged)
├── input.rs             # rustyline editor (existing, unchanged)
├── render.rs            # Markdown rendering (existing, extended)
├── setup_wizard.rs      # Provider wizard (existing, unchanged)
│
└── tui/                 # NEW when `tui` feature enabled
    ├── mod.rs           # run() entrypoint, terminal init/cleanup
    ├── app.rs           # State machine, message list, input, scroll
    ├── event.rs         # EventBroker — crossterm → AppEvent translation
    ├── theme.rs         # Color themes, terminal capability detection
    ├── surface.rs       # impl ConversationSurface for Tui (trait bridge)
    └── widgets/
        ├── chat.rs      # Scrollable message list
        ├── input.rs     # Composer input (tui-textarea wrapper)
        ├── markdown.rs  # Markdown → ratatui Paragraph
        ├── status_bar.rs # Bottom status line
        ├── tool_call.rs  # Inline tool spinner + result
        ├── permission.rs # Modal Y/N overlay
        └── sidebar.rs    # Optional right sidebar (Phase E)
```

The `ConversationSurface` trait is the key architectural decision:

```rust
/// Shared contract between REPL and TUI. Both surfaces consume the same
/// runtime types (ConversationRuntime, Session, ToolExecutor) through this trait.
pub trait ConversationSurface {
    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    fn run_turn(&mut self, prompt: &str) -> Result<TurnResult, RuntimeError>;
    fn session_id(&self) -> Option<&str>;
    fn model_name(&self) -> &str;
}
```

`LiveCli` (REPL) and `TuiApp` (TUI) both implement this trait. The TUI does not depend on `LiveCli`'s internals.

---

## 5. Proposed 52-Week Timeline

Sprints are 2 weeks. Every sprint proposes a mergeable increment. The timeline assumes the upstream maintainer approves the general direction; individual sprints can be accepted, rejected, or reordered.

### Phase A: Foundation (Weeks 1–2)

| Week | Proposal | Files | Estimated Lines (net) |
|---|---|---|---|
| **1** | Extract `session.rs` and `git.rs` from `main.rs`. Add `tui` feature flag to `Cargo.toml`. Add `claw tui` stub command (prints "Coming soon", exits). | `main.rs` (reduced), `session.rs` (new), `git.rs` (new), `Cargo.toml` | −400/+850 |
| **2** | Define `ConversationSurface` trait. Add `tui/mod.rs` and `tui/app.rs` stubs behind `#[cfg(feature = "tui")]`. Add `TestBackend` smoke test. | `tui/mod.rs`, `tui/app.rs` | +180 |

**After Phase A:** `main.rs` is smaller. `claw tui --features tui` exists but is a stub. All existing functionality unchanged.

### Phase B: Core TUI Shell (Weeks 3–8)

| Week | Proposal | Files | Est. Lines |
|---|---|---|---|
| **3** | Add `ratatui` and `tui-textarea` as optional deps. Minimal alternate-screen demo: "Hello, claw" + exit on `q`. | `Cargo.toml`, `tui/mod.rs` | +150 |
| **4** | `EventBroker`: crossterm events → `AppEvent`. `App` renders static mock messages. Arrow keys scroll. | `tui/event.rs`, `tui/app.rs` | +330 |
| **5** | `ComposerWidget` (tui-textarea). Enter sends, Shift+Enter newline, up/down history. | `tui/widgets/input.rs` | +160 |
| **6** | `ChatWidget` scrollable message list. `StatusBarWidget` (static model name, permission mode). Resize handling. | `tui/widgets/chat.rs`, `tui/widgets/status_bar.rs` | +300 |
| **7-8** | Integration + bug fixes. `claw tui --features tui` opens full mock chat. First `docs/TUI-USAGE.md` draft. | `tui/` integration tests | +200 |

### Phase C: Live Conversation (Weeks 9–16)

| Week | Proposal | Est. Lines |
|---|---|---|
| **9** | Wire `ConversationRuntime` via `ConversationSurface`. `TuiProgressReporter` sends deltas to TUI channel. | +250 |
| **10** | Streaming assistant text appends in real time. No markdown — raw text. Auto-scroll. | +180 |
| **11** | Session auto-save on exit. `claw tui --resume --features tui` loads session. | +200 |
| **12** | Inline tool call rendering: spinner → ✓/✗ with truncated result. | +220 |
| **13** | Permission prompt modal overlay (centered, Y/N, non-blocking). | +200 |
| **14** | Error handling: styled error messages in chat. No panics. | +150 |
| **15** | Multi-turn conversation: follow-up messages, input clears, full scrollback. | +100 |
| **16** | Buffer week: bug fixes, manual QA with real model, performance baseline. | +50 |

### Phase D: Rich Rendering (Weeks 17–24)

| Week | Proposal | Est. Lines |
|---|---|---|
| **17** | `MarkdownWidget`: port `render.rs` (pulldown-cmark + syntect) to ratatui. Headings, lists, code blocks. | +300 |
| **18** | Table rendering with ratatui `Table` widget. | +150 |
| **19** | Blockquotes, links, incremental markdown streaming (no full re-parse per delta). | +200 |
| **20** | Dark/light theme system. `NO_COLOR` support. Truecolor → 256 → 16 fallback. | +250 |
| **21** | Collapsible tool results (Enter toggles expand/collapse). | +180 |
| **22** | Colored diff rendering (green additions, red removals). | +200 |
| **23-24** | Performance pass. `cargo flamegraph`. Target: 60fps on 100-message session. | +100 |

### Phase E: Navigation & Power (Weeks 25–34)

| Week | Proposal | Est. Lines |
|---|---|---|
| **25** | `/` incremental search within conversation. | +200 |
| **26** | `?` help overlay with keybindings panel. | +150 |
| **27** | `Ctrl+S` fuzzy session picker. | +200 |
| **28** | Slash command picker in TUI (`/` in input). | +250 |
| **29** | `/compact` with toast notification. | +100 |
| **30** | Copy to clipboard (`arboard`). | +120 |
| **31** | Mouse support: scroll, click expand, focus input. | +150 |
| **32** | Internal pager for long outputs (`j`/`k`/`q`). | +180 |
| **33** | Optional right sidebar (`Ctrl+P`) for tool status. | +250 |
| **34** | Custom keybindings from `.claw.json`. | +150 |

### Phase F: Polish & Hardening (Weeks 35–44)

| Week | Proposal | Est. Lines |
|---|---|---|
| **35** | Screen reader annotations (terminal title updates). | +100 |
| **36** | Small-terminal graceful degradation (<30 rows or <80 cols). | +120 |
| **37** | Unicode width handling (`unicode-width` crate). | +100 |
| **38** | Snapshot tests with `insta`. | +150 |
| **39** | Stress test: 1,000 messages. | +50 |
| **40** | Windows testing (PowerShell, Windows Terminal). | +100 |
| **41** | Complete `docs/TUI-USAGE.md` user guide. | +300 |
| **42** | `claw doctor --tui` diagnostic mode. | +150 |
| **43-44** | Buffer: bug fixes, review feedback, CI tuning. | +100 |

### Phase G: Advanced Features (Weeks 45–52)

| Week | Proposal | Est. Lines |
|---|---|---|
| **45** | Image support (sixel/iTerm2 protocols) when vision API lands. Deferred to upstream API readiness. | +200 |
| **46** | Notebook editing in TUI. | +250 |
| **47** | Agent team dashboard (`Ctrl+A`). | +250 |
| **48** | MCP server inspector (`Ctrl+M`). | +200 |
| **49** | Teleport mode: fuzzy file finder (`Ctrl+G`). | +180 |
| **50** | Release polish: update `ROADMAP.md`, `CHANGELOG.md`. | +100 |
| **51-52** | Buffer: final fixes, evaluate default-TUI mode. | +100 |

---

## 6. Testing Proposal

### Unit Tests
Every new widget module should include at least one test using ratatui's `TestBackend` (an in-memory buffer). This allows headless CI testing without requiring a real terminal.

```rust
#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_widget_renders() {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        // render widget, assert buffer content
    }
}
```

### Integration Tests
- `tests/tui_smoke.rs`: Verify `claw tui --help` works and exits with feature flag.
- `tests/tui_mock_conversation.rs`: Mock `ConversationSurface` that emits canned deltas; assert rendered output.

### Mock Parity Harness
Proposed new scenarios (added incrementally as features land):

| Scenario | Tests |
|---|---|
| `tui_stub` | `claw tui --features tui` exits cleanly |
| `tui_smoke` | Full layout renders with mock data |
| `tui_streaming` | Streaming text appears in chat |
| `tui_session` | Save + resume round-trip |
| `tui_tool_call` | Inline tool rendering |
| `tui_permission` | Modal overlay |
| `tui_markdown` | Markdown widget rendering |

These are **proposed** additions to `mock_parity_scenarios.json` and would require upstream maintainer approval.

---

## 7. Risk Disclosure

| Risk | Likelihood | Impact | Proposed Mitigation |
|---|---|---|---|
| `main.rs` refactoring conflicts with other in-flight PRs | Medium | High | Propose extraction PRs early; merge conflicts resolved after each extraction |
| Feature flag complexity adds friction | Low | Low | Default OFF; only CI and interested users compile with it |
| TUI rendering performance poor on large sessions | Medium | Medium | Performance budget (Week 23-24); TestBackend benchmarks |
| Terminal compatibility (tmux, SSH, Windows) | Medium | Medium | Rely on `crossterm` abstraction; Windows testing in Week 40 |
| Prior credit-warfare trauma | Low | Medium | All TUI attribution via commit trailers; no README edits in TUI PRs |
| Upstream maintainer rejects large chunks of the plan | Medium | High | Plan is modular — any phase can be accepted or rejected independently |

---

## 8. What the Upstream Maintainer Needs to Decide

1. **Is the `ConversationSurface` trait approach acceptable?** This is the architectural linchpin.
2. **Is a `tui` feature flag the right integration strategy?** Or would a separate binary (`claw-tui`) or runtime detection be preferred?
3. **Should extraction PRs be merged before TUI PRs?** Or are you comfortable with parallel development?
4. **What is the preferred PR size?** The proposal suggests ≤400-line PRs for reviewability, but the maintainer may prefer larger chunks or smaller ones.
5. **Should any phases be cut entirely?** For example, mouse support (Week 31) or image rendering (Week 45) may be out of scope.

---

## 9. Rollback Strategy (If the TUI is Rejected)

If the upstream maintainer decides the TUI is not wanted:

```bash
# Remove TUI code, keep all extractions
git checkout main -- rust/crates/rusty-claude-cli/src/tui/
git revert <tui-feature-flag-commit>

# Result:
# - session.rs and git.rs extractions remain (pure wins)
# - cli/ module remains (already done)
# - REPL untouched
# - All TUI code gone
# - Binary size unchanged (feature flag default OFF)
```

Because the TUI is behind a feature flag, it can be disabled without touching a single line of REPL code.

---

## 10. Prior Documents

This proposal supersedes:
- `docs/TUI-52-WEEK-SPRINT.md` (v1) — had API errors, assumed unbroken monolith, presented guardrails as mandates
- `docs/TUI.md` — research and crate stack (still valid)
- `docs/TUI-52-WEEK-PLAN.md` — failure analysis (still valid)

---

*Generated: 2026-06-12 | Revision: v3 | Author: TheArchitectit*  
*Intended for upstream PR review and maintainer comment.*
