# Bold Model Status-Line Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render the Claude-style status line's model name in green bold text while keeping the reasoning suffix green and normal weight.

**Architecture:** Split the current combined model/reasoning string into independently styled Ratatui spans. Preserve all visible text and responsive layout behavior.

**Tech Stack:** Rust, Ratatui, Insta, Cargo/Just

## Global Constraints

- Do not change status-line text, colors, line counts, truncation, or metrics.
- The model name is green and bold.
- The optional ` · reasoning` suffix is green and not bold.

---

### Task 1: Style the Model Name Independently

**Files:**
- Modify: `codex-rs/tui/src/bottom_pane/claude_status_line.rs`
- Verify: `codex-rs/tui/src/bottom_pane/snapshots/codex_tui__bottom_pane__claude_status_line__tests__claude_status_line_narrow.snap`

**Interfaces:**
- Consumes: `ClaudeStatusLineData { model, reasoning, .. }`
- Produces: `model_and_reasoning_spans(&ClaudeStatusLineData) -> Vec<Span<'static>>`

- [ ] **Step 1: Write the failing style assertions**

Import `Modifier` and replace the first assertions in `semantic_fields_use_approved_colors` with:

```rust
assert_eq!(lines[0].spans[0].content, "GPT-5.4");
assert_eq!(lines[0].spans[0].style.fg, Some(MODEL));
assert!(
    lines[0].spans[0]
        .style
        .add_modifier
        .contains(Modifier::BOLD)
);
assert_eq!(lines[0].spans[1].content, " · high");
assert_eq!(lines[0].spans[1].style.fg, Some(MODEL));
assert!(
    !lines[0].spans[1]
        .style
        .add_modifier
        .contains(Modifier::BOLD)
);
assert_eq!(lines[0].spans[3].style.fg, Some(PATH));
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
cd codex-rs
just test -p codex-tui semantic_fields_use_approved_colors -- --nocapture
```

Expected: FAIL because the model and reasoning currently share one non-bold span.

- [ ] **Step 3: Split and style the model/reasoning spans**

Replace the identity construction with:

```rust
let mut identity = model_and_reasoning_spans(data);
```

Replace `model_and_reasoning` with:

```rust
fn model_and_reasoning_spans(data: &ClaudeStatusLineData) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(
        data.model.clone(),
        Style::default().fg(MODEL).add_modifier(Modifier::BOLD),
    )];
    if let Some(reasoning) = data.reasoning.as_deref().filter(|value| !value.is_empty()) {
        spans.push(Span::styled(
            format!(" · {reasoning}"),
            Style::default().fg(MODEL),
        ));
    }
    spans
}
```

- [ ] **Step 4: Verify GREEN and unchanged text snapshot**

Run:

```bash
cd codex-rs
just test -p codex-tui claude_status_line -- --nocapture
```

Expected: all Claude status-line tests pass and the existing snapshot has no textual diff.

- [ ] **Step 5: Format and lint**

Run:

```bash
cd codex-rs
cargo fmt -- --config imports_granularity=Item
just fix -p codex-tui
```

Expected: both commands exit zero and Clippy reports no new warnings.

- [ ] **Step 6: Commit, install, and publish**

Run:

```bash
git add codex-rs/tui/src/bottom_pane/claude_status_line.rs docs/superpowers
git commit -m "feat(tui): emphasize Claude status model"
scripts/install-custom-codex.sh
git push origin feature/claude-style-statusline
```

Expected: `codex --version` reports `0.142.0-custom.1`, the installed TUI shows a bold green model name, and the remote branch matches local HEAD.
