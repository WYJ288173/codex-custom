# Quiet Startup Notices Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `codex-custom` startup match Claude Code’s quieter customization UX, verify it through the remote GitHub build/package workflow, and install the resulting release locally only after tests/build/release pass.

**Architecture:** Implement this in two phases. Phase 1 adds a small TUI startup-notice policy layer and routes skill-load/MCP-startup warning rendering through it. Phase 2 productizes the existing `McpStartupPolicy::LazyWhenCached` path for root sessions: optional remote MCP servers with cached visible tools are published immediately from cache and connected in the background/on first use, while required servers, stdio servers, selected-plugin MCP servers, and uncached servers remain eager.

**Tech Stack:** Rust, `codex-rs/config`, `codex-rs/core`, `codex-rs/tui`, `nextest`, existing config TOML serde path.

## Global Constraints

- Default behavior for `codex-custom`: startup skill-load errors and MCP startup failures are quiet in the transcript.
- Default root-session MCP behavior for `codex-custom`: optional remote MCP servers with cached visible tools use lazy/cached startup.
- Diagnostic behavior must remain available via logs, existing MCP status surfaces, and explicit verbose config.
- Final delivery must use the GitHub workflow to compile, test, package, and publish the release artifact; do not treat local build success alone as deliverable.
- Local install must happen only after the GitHub workflow completes successfully and the release exists.
- Remote build/package takes a long time; after triggering it, do not poll frequently. Use long waits, with a default first wait of about 30 minutes, then 20-30 minute polling intervals unless the user explicitly asks for an immediate status check.
- Do not suppress fatal errors that prevent Codex from starting.
- Do not skip loading skills or plugins.
- Do not lazy-load required MCP servers.
- Do not lazy-load stdio MCP servers in this iteration; local process startup semantics are different and failures are more likely to represent local config breakage.
- Do not lazy-load selected-plugin MCP servers in this iteration; selected plugin tool availability should stay deterministic.
- Do not expose stale cached tools when the server config, environment, auth identity, protocol mode, elicitation capability, or client MCP extensions changed.
- If no cached visible tools exist for a server, start it eagerly as today.
- Keep the implementation easy to rebase onto upstream: prefer new config fields and local TUI gates over invasive MCP runtime changes.
- Claude Code reference points: `claude mcp list` exposes health states such as connected, needs-authentication, failed, or pending approval; Agent SDK MCP status may be `pending` when tool lists are loaded from cache and the server connects only when a tool is first used. Treat remote MCP startup problems as status/diagnostic state, not main transcript content.

---

## Target UX

Default `codex-custom` startup should not print:

```text
⚠ Skipped loading 21 skill(s) due to invalid SKILL.md files.
⚠ /path/to/SKILL.md: failed to read file: Too many open files (os error 24)
⚠ MCP client for `openaiDeveloperDocs` failed to start: ...
⚠ MCP client for `codex_apps` timed out after 30 seconds. ...
⚠ MCP startup incomplete (failed: codex_apps, openaiDeveloperDocs)
```

Recommended default replacement: print nothing for these non-fatal startup diagnostics.

Optional verbose mode should preserve current behavior:

```toml
[tui.startup_notices]
skill_load_errors = "verbose"
mcp_startup_errors = "verbose"
```

Optional summary mode can be supported at the same time:

```toml
[tui.startup_notices]
skill_load_errors = "summary"
mcp_startup_errors = "summary"
```

Summary copy:

```text
⚠ Startup diagnostics: 21 skills skipped. Run `codex doctor` for details.
⚠ MCP startup incomplete: 2 servers failed. Run `/mcp` or `codex mcp list` for details.
```

## Target Lazy/Cached MCP Behavior

For root interactive sessions, `codex-custom` should behave like this:

| Server kind | Cache state | Startup behavior |
|---|---:|---|
| Required MCP server | any | eager, blocks startup summary as today |
| Stdio MCP server | any | eager, no behavior change |
| Selected-plugin MCP server | any | eager, no behavior change |
| Optional remote Streamable HTTP MCP server | no cached visible tools | eager, no behavior change |
| Optional remote Streamable HTTP MCP server | cached visible tools | publish cached tools immediately; connect in background/on first tool call; do not hold startup open |

The user-facing result should be:

- Main transcript is not flooded by stale remote MCP startup failures.
- Tools from cached optional remote MCP servers can still be offered quickly.
- A real tool call still forces a live connection before execution.
- If the background reconnect fails, existing cached tools remain available for tool discovery but execution reports a normal tool-call/connect error if the user actually invokes that server.
- `/mcp`, `codex mcp list`, and logs remain the places to inspect remote MCP status.

## File Structure

- Modify `codex-rs/config/src/types.rs`
  - Add serde-backed config structs/enums under existing `Tui`.
  - Responsibility: parse `[tui.startup_notices]`.

- Modify `codex-rs/core/src/config/mod.rs`
  - Add effective config fields to `Config`.
  - Responsibility: expose resolved startup notice policy to the TUI.

- Modify `codex-rs/tui/src/app/startup_prompts.rs`
  - Route skill load warning rendering through config policy.
  - Responsibility: no transcript spam for invalid skills unless policy says summary/verbose.

- Modify `codex-rs/tui/src/app/thread_routing.rs`
  - Pass `self.config` into skill warning emission.
  - Responsibility: preserve dedupe state while respecting quiet mode.

- Modify `codex-rs/tui/src/chatwidget/mcp_startup.rs`
  - Route MCP startup failure rendering through config policy.
  - Responsibility: keep progress/status lifecycle, suppress final warning history cells by default.

- Modify `codex-rs/tui/src/chatwidget.rs` and `codex-rs/tui/src/chatwidget/constructor.rs` if `ChatWidget` does not already have access to full `Config`.
  - Responsibility: make MCP startup renderer able to read startup notice policy.

- Modify `codex-rs/config/src/types.rs`
  - Add `mcp_startup_mode` under `[tui.startup_notices]` or a dedicated `[mcp.startup]` config, depending on existing config layout.
  - Responsibility: configure eager vs lazy-cached behavior without changing per-server config.

- Modify `codex-rs/core/src/config/mod.rs`
  - Expose effective `mcp_startup_mode`.
  - Responsibility: make session MCP runtime selection configurable.

- Modify `codex-rs/core/src/session/mcp_runtime.rs`
  - Select `McpStartupPolicy::LazyWhenCached` for root sessions when config enables it.
  - Responsibility: switch policy without changing lower-level connection logic.

- Modify `codex-rs/codex-mcp/src/connection_manager.rs`
  - Tighten lazy eligibility to optional remote cached servers only.
  - Responsibility: preserve eager behavior for required, stdio, selected-plugin, uncached, and changed-identity servers.

- Modify `codex-rs/codex-mcp/src/connection_manager/tool_catalog.rs` only if tests show cached tool listing waits for background startup.
  - Responsibility: ensure cached visible tools can be listed immediately.

- Modify tests:
  - `codex-rs/tui/src/app/startup_prompts.rs`
  - `codex-rs/tui/src/chatwidget/tests/mcp_startup.rs`
  - `codex-rs/tui/src/app/tests.rs` only where existing assertions expect the old warning transcript.
  - `codex-rs/core/src/config/config_tests.rs` or `codex-rs/config/src/types_tests.rs` for config parsing.
  - `codex-rs/core/src/session/mcp_runtime.rs` tests for policy selection.
  - `codex-rs/codex-mcp/src/connection_manager_tests.rs` for lazy/cached root behavior.

---

### Task 1: Add startup notice and MCP startup mode config

**Files:**
- Modify: `codex-rs/config/src/types.rs`
- Modify: `codex-rs/core/src/config/mod.rs`
- Test: `codex-rs/core/src/config/config_tests.rs`

**Interfaces:**
- Produces:
  - `StartupNoticeLevel::{Quiet, Summary, Verbose}`
  - `McpStartupMode::{Eager, LazyCachedRemote}`
  - `TuiStartupNotices { skill_load_errors, mcp_startup_errors }`
  - `Config::mcp_startup_mode: McpStartupMode`
  - `Config::tui_startup_notices: TuiStartupNotices`
- Consumes: existing `Tui` config and `Config::load_from_base_config_with_overrides` path.

- [ ] **Step 1: Add failing config parse test**

Add to `codex-rs/core/src/config/config_tests.rs` near existing TUI config tests:

```rust
#[test]
fn loads_tui_startup_notice_levels() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let config_toml = r#"
[tui.startup_notices]
skill_load_errors = "summary"
mcp_startup_errors = "verbose"
mcp_startup_mode = "eager"
"#;
    write_config_toml(codex_home.path(), config_toml)?;

    let config = load_config_for_test(codex_home.path())?;

    assert_eq!(
        config.tui_startup_notices.skill_load_errors,
        StartupNoticeLevel::Summary
    );
    assert_eq!(
        config.tui_startup_notices.mcp_startup_errors,
        StartupNoticeLevel::Verbose
    );
    assert_eq!(config.mcp_startup_mode, McpStartupMode::Eager);
    Ok(())
}

#[test]
fn startup_customization_defaults_match_claude_code_style() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    write_config_toml(codex_home.path(), "")?;

    let config = load_config_for_test(codex_home.path())?;

    assert_eq!(
        config.tui_startup_notices.skill_load_errors,
        StartupNoticeLevel::Quiet
    );
    assert_eq!(
        config.tui_startup_notices.mcp_startup_errors,
        StartupNoticeLevel::Quiet
    );
    assert_eq!(config.mcp_startup_mode, McpStartupMode::LazyCachedRemote);
    Ok(())
}
```

If this test module does not already import `StartupNoticeLevel` and `McpStartupMode`, add:

```rust
use codex_config::McpStartupMode;
use codex_config::StartupNoticeLevel;
```

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-core loads_tui_startup_notice_levels startup_customization_defaults_match_claude_code_style
```

Expected: compile failure because `StartupNoticeLevel`, `McpStartupMode`, `mcp_startup_mode`, and `tui_startup_notices` do not exist.

- [ ] **Step 3: Implement config types**

In `codex-rs/config/src/types.rs`, below `StatusLineLayout`, add:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StartupNoticeLevel {
    #[default]
    Quiet,
    Summary,
    Verbose,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum McpStartupMode {
    Eager,
    #[default]
    LazyCachedRemote,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiStartupNotices {
    #[serde(default)]
    pub skill_load_errors: StartupNoticeLevel,

    #[serde(default)]
    pub mcp_startup_errors: StartupNoticeLevel,

    #[serde(default)]
    pub mcp_startup_mode: McpStartupMode,
}
```

Inside `pub struct Tui`, after `show_tooltips`, add:

```rust
    /// Controls how non-fatal startup diagnostics are rendered in the transcript.
    /// Defaults to quiet for codex-custom so broken customizations do not flood every session.
    #[serde(default)]
    pub startup_notices: TuiStartupNotices,
```

Ensure the crate publicly re-exports these types if `codex_config::StartupNoticeLevel` / `codex_config::McpStartupMode` are not already visible through the existing module exports.

- [ ] **Step 4: Expose effective config**

In `codex-rs/core/src/config/mod.rs`, import or re-export `StartupNoticeLevel` and `TuiStartupNotices` consistently with other `codex_config` types.

In `pub struct Config`, near `show_tooltips`, add:

```rust
    /// Controls how non-fatal skill/MCP startup diagnostics are rendered in the TUI transcript.
    pub tui_startup_notices: TuiStartupNotices,

    /// Controls whether optional remote MCP servers with cached tools are started lazily.
    pub mcp_startup_mode: McpStartupMode,
```

In the `Config` construction block near `show_tooltips`, add:

```rust
            tui_startup_notices: cfg
                .tui
                .as_ref()
                .map(|t| t.startup_notices)
                .unwrap_or_default(),
            mcp_startup_mode: cfg
                .tui
                .as_ref()
                .map(|t| t.startup_notices.mcp_startup_mode)
                .unwrap_or_default(),
```

- [ ] **Step 5: Run config tests**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-core loads_tui_startup_notice_levels startup_customization_defaults_match_claude_code_style
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add codex-rs/config/src/types.rs codex-rs/core/src/config/mod.rs codex-rs/core/src/config/config_tests.rs
git commit -m "feat(tui): add startup customization policy config"
```

---

### Task 2: Quiet skill-load warnings by default

**Files:**
- Modify: `codex-rs/tui/src/app/startup_prompts.rs`
- Modify: `codex-rs/tui/src/app/thread_routing.rs`
- Test: `codex-rs/tui/src/app/startup_prompts.rs`

**Interfaces:**
- Consumes: `Config::tui_startup_notices.skill_load_errors`
- Produces: `emit_skill_load_warnings(app_event_tx, config, errors)`

- [ ] **Step 1: Update failing tests**

In `codex-rs/tui/src/app/startup_prompts.rs`, replace the existing helper test body around `render_skill_load_warning_cells` with three policy-specific tests:

```rust
fn render_skill_load_warning_cells_with_level(
    errors: &[SkillErrorInfo],
    level: StartupNoticeLevel,
) -> String {
    let (app_event_tx, mut app_event_rx) = AppEventSender::new_test();
    let mut config = test_config();
    config.tui_startup_notices.skill_load_errors = level;
    emit_skill_load_warnings(&app_event_tx, &config, errors);

    let mut rendered = String::new();
    while let Ok(AppEvent::InsertHistoryCell(cell)) = app_event_rx.try_recv() {
        rendered.push_str(&cell.display_text());
    }
    rendered
}

#[test]
fn skill_load_warning_quiet_renders_nothing() {
    let errors = vec![SkillErrorInfo {
        path: PathBuf::from("/tmp/bad/SKILL.md"),
        message: "failed to read file: Too many open files (os error 24)".to_string(),
    }];

    assert_eq!(
        render_skill_load_warning_cells_with_level(&errors, StartupNoticeLevel::Quiet),
        ""
    );
}

#[test]
fn skill_load_warning_summary_renders_single_diagnostic_hint() {
    let errors = vec![SkillErrorInfo {
        path: PathBuf::from("/tmp/bad/SKILL.md"),
        message: "failed to read file: Too many open files (os error 24)".to_string(),
    }];

    assert_eq!(
        render_skill_load_warning_cells_with_level(&errors, StartupNoticeLevel::Summary),
        "⚠ Startup diagnostics: 1 skill skipped. Run `codex doctor` for details.\n"
    );
}

#[test]
fn skill_load_warning_verbose_preserves_existing_details() {
    let errors = vec![SkillErrorInfo {
        path: PathBuf::from("/tmp/bad/SKILL.md"),
        message: "failed to read file: Too many open files (os error 24)".to_string(),
    }];

    let rendered = render_skill_load_warning_cells_with_level(&errors, StartupNoticeLevel::Verbose);

    assert!(rendered.contains("Skipped loading 1 skill(s) due to invalid SKILL.md files."));
    assert!(rendered.contains("/tmp/bad/SKILL.md: failed to read file"));
}
```

If exact helper names differ, preserve the current test harness and only change the expected rendering.

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-tui skill_load_warning_quiet_renders_nothing skill_load_warning_summary_renders_single_diagnostic_hint skill_load_warning_verbose_preserves_existing_details
```

Expected: compile failure or assertion failure because `emit_skill_load_warnings` does not accept `Config`.

- [ ] **Step 3: Implement policy gate**

In `codex-rs/tui/src/app/startup_prompts.rs`, change:

```rust
pub(super) fn emit_skill_load_warnings(app_event_tx: &AppEventSender, errors: &[SkillErrorInfo]) {
```

to:

```rust
pub(super) fn emit_skill_load_warnings(
    app_event_tx: &AppEventSender,
    config: &Config,
    errors: &[SkillErrorInfo],
) {
```

At the top of the function, after the empty check, add:

```rust
    match config.tui_startup_notices.skill_load_errors {
        StartupNoticeLevel::Quiet => return,
        StartupNoticeLevel::Summary => {
            let noun = if errors.len() == 1 { "skill" } else { "skills" };
            app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                crate::history_cell::new_warning_event(format!(
                    "Startup diagnostics: {} {noun} skipped. Run `codex doctor` for details.",
                    errors.len()
                )),
            )));
            return;
        }
        StartupNoticeLevel::Verbose => {}
    }
```

Import `StartupNoticeLevel` in this file if needed:

```rust
use codex_config::StartupNoticeLevel;
```

- [ ] **Step 4: Update callers**

In `codex-rs/tui/src/app/thread_routing.rs`, change:

```rust
emit_skill_load_warnings(&self.app_event_tx, &errors);
```

to:

```rust
emit_skill_load_warnings(&self.app_event_tx, &self.config, &errors);
```

Find any other callers:

```bash
rg -n "emit_skill_load_warnings\\(" codex-rs/tui/src
```

Update tests/helpers to pass a test config.

- [ ] **Step 5: Run skill warning tests**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-tui skill_load_warning
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add codex-rs/tui/src/app/startup_prompts.rs codex-rs/tui/src/app/thread_routing.rs
git commit -m "feat(tui): quiet skill startup diagnostics"
```

---

### Task 3: Quiet MCP startup failure transcript by default

**Files:**
- Modify: `codex-rs/tui/src/chatwidget/mcp_startup.rs`
- Modify: `codex-rs/tui/src/chatwidget.rs`
- Modify: `codex-rs/tui/src/chatwidget/constructor.rs`
- Test: `codex-rs/tui/src/chatwidget/tests/mcp_startup.rs`

**Interfaces:**
- Consumes: `Config::tui_startup_notices.mcp_startup_errors`
- Produces:
  - Progress/status behavior unchanged while MCP startup is running.
  - `finish_mcp_startup()` inserts no warning in quiet mode.
  - `finish_mcp_startup()` inserts one summary in summary mode.
  - `finish_mcp_startup()` preserves current detailed warnings in verbose mode.

- [ ] **Step 1: Add failing MCP tests**

In `codex-rs/tui/src/chatwidget/tests/mcp_startup.rs`, add:

```rust
#[tokio::test]
async fn mcp_startup_quiet_mode_suppresses_failure_history() {
    let mut chat = make_chatwidget().await;
    chat.config.tui_startup_notices.mcp_startup_errors = StartupNoticeLevel::Quiet;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);

    chat.on_mcp_server_status_updated(McpServerStatusUpdatedNotification {
        name: "alpha".to_string(),
        status: McpServerStartupState::Failed,
        error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        thread_id: None,
    });

    let rendered = drain_history_text(&mut chat);
    assert!(!rendered.contains("MCP client for `alpha` failed to start"));
    assert!(!rendered.contains("MCP startup incomplete"));
}

#[tokio::test]
async fn mcp_startup_summary_mode_renders_single_hint() {
    let mut chat = make_chatwidget().await;
    chat.config.tui_startup_notices.mcp_startup_errors = StartupNoticeLevel::Summary;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);

    chat.on_mcp_server_status_updated(McpServerStatusUpdatedNotification {
        name: "alpha".to_string(),
        status: McpServerStartupState::Failed,
        error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        thread_id: None,
    });

    let rendered = drain_history_text(&mut chat);
    assert_eq!(
        rendered,
        "⚠ MCP startup incomplete: 1 server failed. Run `/mcp` or `codex mcp list` for details.\n"
    );
}

#[tokio::test]
async fn mcp_startup_verbose_mode_preserves_existing_failure_history() {
    let mut chat = make_chatwidget().await;
    chat.config.tui_startup_notices.mcp_startup_errors = StartupNoticeLevel::Verbose;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);

    chat.on_mcp_server_status_updated(McpServerStatusUpdatedNotification {
        name: "alpha".to_string(),
        status: McpServerStartupState::Failed,
        error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        thread_id: None,
    });

    let rendered = drain_history_text(&mut chat);
    assert!(rendered.contains("MCP client for `alpha` failed to start: handshake failed"));
    assert!(rendered.contains("MCP startup incomplete (failed: alpha)"));
}
```

If the existing test utilities use different names, adapt only `make_chatwidget()` and `drain_history_text()` to the current helpers in the same file. Do not create a separate harness.

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-tui mcp_startup_quiet_mode_suppresses_failure_history mcp_startup_summary_mode_renders_single_hint mcp_startup_verbose_mode_preserves_existing_failure_history
```

Expected: failures because current code always renders warnings.

- [ ] **Step 3: Ensure ChatWidget has config access**

If `ChatWidget` already has `config: Config`, use it directly. If not, add:

In `codex-rs/tui/src/chatwidget.rs`:

```rust
    pub(crate) config: Config,
```

In `codex-rs/tui/src/chatwidget/constructor.rs`, initialize from constructor argument:

```rust
            config,
```

If `ChatWidget` already stores a narrower config copy, prefer extending that existing config copy instead of adding a second full `Config`.

- [ ] **Step 4: Implement MCP policy gate**

In `codex-rs/tui/src/chatwidget/mcp_startup.rs`, import:

```rust
use codex_config::StartupNoticeLevel;
```

Change `finish_mcp_startup` warning logic to:

```rust
        match self.config.tui_startup_notices.mcp_startup_errors {
            StartupNoticeLevel::Quiet => {}
            StartupNoticeLevel::Summary => {
                let failed_count = failed.len();
                let cancelled_count = cancelled.len();
                if failed_count > 0 || cancelled_count > 0 {
                    let mut clauses = Vec::new();
                    if failed_count > 0 {
                        let noun = if failed_count == 1 { "server" } else { "servers" };
                        clauses.push(format!("{failed_count} {noun} failed"));
                    }
                    if cancelled_count > 0 {
                        let noun = if cancelled_count == 1 { "server" } else { "servers" };
                        clauses.push(format!("{cancelled_count} {noun} interrupted"));
                    }
                    self.on_warning(format!(
                        "MCP startup incomplete: {}. Run `/mcp` or `codex mcp list` for details.",
                        clauses.join(", ")
                    ));
                }
            }
            StartupNoticeLevel::Verbose => {
                if !cancelled.is_empty() {
                    self.on_warning(format!(
                        "MCP startup interrupted. The following servers were not initialized: {}",
                        cancelled.join(", ")
                    ));
                }
                let mut parts = Vec::new();
                if !failed.is_empty() {
                    parts.push(format!("failed: {}", failed.join(", ")));
                }
                if !parts.is_empty() {
                    self.on_warning(format!("MCP startup incomplete ({})", parts.join("; ")));
                }
            }
        }
```

Important: do not suppress the transient status header while startup is running unless the user later asks for that too. This plan suppresses transcript noise only.

- [ ] **Step 5: Run MCP startup tests**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-tui mcp_startup
```

Expected: pass after updating old tests that expected verbose behavior by explicitly setting `StartupNoticeLevel::Verbose`.

- [ ] **Step 6: Commit**

```bash
git add codex-rs/tui/src/chatwidget.rs codex-rs/tui/src/chatwidget/constructor.rs codex-rs/tui/src/chatwidget/mcp_startup.rs codex-rs/tui/src/chatwidget/tests/mcp_startup.rs
git commit -m "feat(tui): quiet mcp startup diagnostics"
```

---

### Task 4: Enable lazy/cached startup for optional remote MCP servers in root sessions

**Files:**
- Modify: `codex-rs/core/src/session/mcp_runtime.rs`
- Modify: `codex-rs/codex-mcp/src/connection_manager.rs`
- Test: `codex-rs/codex-mcp/src/connection_manager_tests.rs`
- Test: existing `codex-rs/core/src/session` MCP runtime tests if present.

**Interfaces:**
- Consumes:
  - `Config::mcp_startup_mode`
  - existing `McpStartupPolicy::{Eager, LazyWhenCached}`
  - existing `McpToolCatalogCacheContext::current_tools`
- Produces:
  - Root sessions use `LazyWhenCached` when `mcp_startup_mode == LazyCachedRemote`.
  - Only optional remote Streamable HTTP servers with cached visible tools are deferred.
  - Required, stdio, selected-plugin, uncached, auth/config-changed servers remain eager.

- [ ] **Step 1: Add failing policy-selection test**

In the nearest existing test module for `codex-rs/core/src/session/mcp_runtime.rs`, add a test that constructs a root-session desired thread config with default config and asserts the selected startup policy is `LazyWhenCached`.

Use the existing helper that builds `McpRuntimeInput`; the assertion should be:

```rust
assert_eq!(input.startup_policy, McpStartupPolicy::LazyWhenCached);
```

Also add explicit eager override coverage:

```rust
config.mcp_startup_mode = McpStartupMode::Eager;
let input = build_mcp_runtime_input_for_root_session(config).await;
assert_eq!(input.startup_policy, McpStartupPolicy::Eager);
```

If this code path is hard to unit test because helpers are private, add the assertion to the closest existing session suite test that starts a root thread and observes the built runtime input.

- [ ] **Step 2: Run policy-selection test and verify failure**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-core mcp_startup_policy
```

Expected: failure because root sessions still use `Eager`.

- [ ] **Step 3: Implement root policy selection**

In `codex-rs/core/src/session/mcp_runtime.rs`, replace:

```rust
            startup_policy: if matches!(desired.session_source, SessionSource::SubAgent(_)) {
                McpStartupPolicy::LazyWhenCached
            } else {
                McpStartupPolicy::Eager
            },
```

with:

```rust
            startup_policy: if matches!(desired.session_source, SessionSource::SubAgent(_))
                || matches!(
                    desired.config.mcp_startup_mode,
                    codex_config::McpStartupMode::LazyCachedRemote
                )
            {
                McpStartupPolicy::LazyWhenCached
            } else {
                McpStartupPolicy::Eager
            },
```

This keeps subagent behavior unchanged and extends lazy/cached behavior to root sessions by config.

- [ ] **Step 4: Add failing connection-manager eligibility tests**

In `codex-rs/codex-mcp/src/connection_manager_tests.rs`, add or adapt tests around existing cached-tool tests:

```rust
#[tokio::test]
async fn root_lazy_cached_remote_defers_optional_streamable_http_with_cached_visible_tools() {
    let cache_context = create_test_tool_catalog_cache_context("remote");
    store_current_tools(&cache_context, vec![create_test_tool("remote", "cached_search")]);

    let manager = create_connection_set_with_policy_and_server(
        McpStartupPolicy::LazyWhenCached,
        optional_streamable_http_server("remote"),
        Some(cache_context),
    )
    .await;

    let cached = tokio::time::timeout(Duration::from_millis(50), capture_binding(&manager))
        .await
        .expect("cached tools should be available without waiting for startup")
        .expect("binding should be available");

    assert_eq!(tool_names(&cached), vec!["cached_search"]);
    assert!(
        startup_barrier_was_not_released("remote"),
        "remote startup should be deferred until first use/background trigger"
    );
}

#[tokio::test]
async fn lazy_cached_mode_keeps_required_servers_eager() {
    let cache_context = create_test_tool_catalog_cache_context("required_remote");
    store_current_tools(&cache_context, vec![create_test_tool("required_remote", "cached_tool")]);

    let manager = create_connection_set_with_policy_and_server(
        McpStartupPolicy::LazyWhenCached,
        required_streamable_http_server("required_remote"),
        Some(cache_context),
    )
    .await;

    assert!(
        startup_barrier_was_reached("required_remote"),
        "required server must start eagerly even with cached tools"
    );
}

#[tokio::test]
async fn lazy_cached_mode_keeps_stdio_servers_eager() {
    let cache_context = create_test_tool_catalog_cache_context("local_stdio");
    store_current_tools(&cache_context, vec![create_test_tool("local_stdio", "cached_tool")]);

    let manager = create_connection_set_with_policy_and_server(
        McpStartupPolicy::LazyWhenCached,
        optional_stdio_server("local_stdio"),
        Some(cache_context),
    )
    .await;

    assert!(
        startup_barrier_was_reached("local_stdio"),
        "stdio server must remain eager"
    );
}
```

Use existing helper names if they already exist. The assertions above are the required behavior; do not duplicate helper scaffolding if current tests already have blocked startup servers and cached catalog helpers.

- [ ] **Step 5: Run connection-manager tests and verify failure**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-mcp lazy_cached
```

Expected: at least the root/remote eligibility test fails or cannot compile until helper wiring is added.

- [ ] **Step 6: Tighten defer eligibility**

In `codex-rs/codex-mcp/src/connection_manager.rs`, replace the existing `defer_startup` predicate:

```rust
            let defer_startup = allow_deferred_startup
                && !configured_config.required
                && !tool_plugin_provenance.is_selected_plugin_mcp_server(&server_name)
                && async_managed_client
                    .tool_catalog_cache_context
                    .as_ref()
                    .and_then(McpToolCatalogCacheContext::current_tools)
                    .is_some_and(|tools| {
                        tools.into_iter().any(|tool| {
                            configured_tool_filter.allows(&tool.tool.name)
                                && tool_is_model_visible(&tool)
                        })
                    });
```

with:

```rust
            let is_remote_streamable_http = matches!(
                configured_config.transport,
                McpServerTransportConfig::StreamableHttp { .. }
            );
            let has_cached_visible_tools = async_managed_client
                .tool_catalog_cache_context
                .as_ref()
                .and_then(McpToolCatalogCacheContext::current_tools)
                .is_some_and(|tools| {
                    tools.into_iter().any(|tool| {
                        configured_tool_filter.allows(&tool.tool.name)
                            && tool_is_model_visible(&tool)
                    })
                });
            let defer_startup = allow_deferred_startup
                && is_remote_streamable_http
                && !configured_config.required
                && !tool_plugin_provenance.is_selected_plugin_mcp_server(&server_name)
                && has_cached_visible_tools;
```

Rationale: the old predicate could defer any optional cached server. The Claude-like behavior should be limited to remote MCPs in this iteration.

- [ ] **Step 7: Ensure first tool call forces live connection**

Find `prepare_call` behavior:

```bash
rg -n "prepare_call|startup_trigger|startup_receiver|client\\(\\)" codex-rs/codex-mcp/src/connection_manager.rs codex-rs/codex-mcp/src/connection_manager -g '*.rs'
```

Verify existing behavior sends the startup trigger before returning a callable client. If it does not, add a test:

```rust
#[tokio::test]
async fn lazy_cached_tool_call_triggers_live_startup_before_execution() {
    let manager = create_lazy_cached_remote_manager_with_blocked_startup().await;
    let binding = capture_binding(&manager).await.expect("cached binding");

    let call = binding.prepare_call("remote", "cached_search").await;

    assert!(
        startup_barrier_was_reached("remote"),
        "preparing a cached remote tool call must trigger live startup"
    );
    assert!(call.is_pending_or_waiting_for_live_client());
}
```

Use existing call/result types in the assertion; the intent is that cached discovery does not bypass live execution.

- [ ] **Step 8: Run MCP tests**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-mcp lazy_cached
```

Expected: pass.

- [ ] **Step 9: Commit**

```bash
git add codex-rs/core/src/session/mcp_runtime.rs codex-rs/codex-mcp/src/connection_manager.rs codex-rs/codex-mcp/src/connection_manager_tests.rs
git commit -m "feat(mcp): lazy start cached remote servers"
```

---

### Task 5: Preserve explicit diagnostics and document config

**Files:**
- Modify: `codex-rs/cli/src/doctor.rs` only if `doctor` currently depends on startup warning history rather than direct diagnostics.
- Modify: docs/config reference if the repo has one; otherwise add a short note to `docs/superpowers/plans/2026-08-12-quiet-startup-notices.md` execution result section after implementation.
- Test: existing `codex doctor` tests and MCP list tests.

**Interfaces:**
- Consumes: existing `codex doctor`, `/mcp`, and `codex mcp list`.
- Produces: confidence that quiet mode does not remove diagnostic access.

- [ ] **Step 1: Check doctor does not regress**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-cli doctor
```

Expected: pass. If failing assertions are only about startup transcript wording, update them to align with quiet transcript and keep diagnostic checks intact.

- [ ] **Step 2: Check MCP CLI diagnostics do not regress**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo test -p codex-cli mcp_list
```

Expected: pass.

- [ ] **Step 3: Add user-facing config note**

If there is a config reference doc, add:

````markdown
### TUI startup notices

`codex-custom` keeps non-fatal startup customization diagnostics quiet by default.
To show summaries or full details in the transcript:

```toml
[tui.startup_notices]
skill_load_errors = "summary" # quiet | summary | verbose
mcp_startup_errors = "summary" # quiet | summary | verbose
mcp_startup_mode = "lazy-cached-remote" # eager | lazy-cached-remote
```

Use `verbose` to restore the previous startup behavior.
Use `mcp_startup_mode = "eager"` to restore the previous eager root-session MCP startup behavior.
````

If no config reference exists, include the same snippet in the final handoff instead of adding a new doc.

- [ ] **Step 4: Commit docs/test updates**

```bash
git add <changed-doc-or-test-files>
git commit -m "docs: document startup notice controls"
```

If no files changed in this task, do not create an empty commit.

---

### Task 6: Full verification, remote build/package, release, local install

**Files:**
- No planned source edits unless tests expose regressions.

**Interfaces:**
- Consumes: all previous commits.
- Produces: locally smoke-tested branch, successful GitHub workflow run, published release artifact, and installed local `codex-custom`.

- [ ] **Step 1: Format check**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo fmt -- --config imports_granularity=Item --check
```

Expected: pass. Existing stable-toolchain warnings about `imports_granularity` are acceptable if exit code is 0.

- [ ] **Step 2: Focused TUI test**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
just test -p codex-tui
```

Expected: all pass.

- [ ] **Step 3: Focused CLI test**

Run:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
just test -p codex-cli
```

Expected: all pass. If known flaky doctor HTTP probe retries and passes, record it as flaky but acceptable.

- [ ] **Step 4: Manual startup smoke test**

Build local debug binary:

```bash
PATH=/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
cargo build -p codex-cli
```

Start the TUI with the same user config that previously produced the warning flood:

```bash
PATH=/private/tmp/codex-custom-linear-fix/codex-rs/target/debug:/Users/huayang/.cargo/bin:/Users/huayang/.local/bin:/opt/homebrew/opt/rustup/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin \
codex
```

Expected:

- No skill-load warning list appears in the initial transcript.
- No MCP failure stack/details appear in the initial transcript.
- If MCP startup progress header appears briefly, it clears without adding failure history cells.
- Normal prompt input remains usable.

- [ ] **Step 5: Push branch**

```bash
git push origin HEAD:feature/claude-style-statusline --force-with-lease
```

Expected: branch updates successfully.

- [ ] **Step 6: Trigger remote GitHub build/package workflow**

Run:

```bash
gh workflow run codex-custom-sync.yml \
  --repo WYJ288173/codex-custom \
  --ref feature/claude-style-statusline \
  -f publish_release=true

gh run list \
  --repo WYJ288173/codex-custom \
  --workflow codex-custom-sync.yml \
  --branch feature/claude-style-statusline \
  --limit 1 \
  --json databaseId,status,conclusion,createdAt,url,headSha
```

Record:

```text
run_id=<databaseId>
run_url=<url>
head_sha=<headSha>
```

Expected: workflow is `queued` or `in_progress`.

- [ ] **Step 7: Wait with long polling cadence**

Do not keep a tight watch loop. The workflow compiles, tests, packages, uploads artifacts, pushes the rebased branch, and creates a GitHub release; previous successful release builds took roughly 90 minutes.

Polling rule:

- Wait about 30 minutes before the first status query unless the user explicitly asks for an immediate check.
- If still running, wait another 20-30 minutes before the next query.
- Each status query should be one `gh run view`, not a continuous `gh run watch`.

First query command:

```bash
gh run view "$run_id" \
  --repo WYJ288173/codex-custom \
  --json status,conclusion,url,createdAt,updatedAt,jobs
```

Expected while running: `status` is `queued` or `in_progress`; report current job/step briefly and stop polling.

- [ ] **Step 8: Verify remote success**

When the run reports `completed`, inspect the conclusion:

```bash
gh run view "$run_id" \
  --repo WYJ288173/codex-custom \
  --json status,conclusion,url,createdAt,updatedAt,jobs
```

Expected:

```text
status=completed
conclusion=success
```

If conclusion is not `success`, do not install locally. Fetch the failed job logs, diagnose, fix, rerun local focused tests, push, and trigger a new workflow run.

- [ ] **Step 9: Resolve release tag from successful remote build**

After remote success:

```bash
git fetch origin feature/claude-style-statusline
remote_short_sha="$(git rev-parse --short=12 origin/feature/claude-style-statusline)"
gh release list --repo WYJ288173/codex-custom --limit 10
```

Select the newest release tag that ends with:

```text
-$remote_short_sha
```

Expected example:

```text
codex-custom-0.147.0-custom.1-aa50ed1b3546
```

- [ ] **Step 10: Download and install the successful release locally**

Install exactly the release produced by the successful workflow:

```bash
scripts/install-codex-custom.sh \
  --repo WYJ288173/codex-custom \
  --release "$release_tag" \
  --target aarch64-apple-darwin
```

Do not use `latest` for this install step; use the explicit release tag tied to the successful run.

- [ ] **Step 11: Verify local binary and shortcut**

Run:

```bash
command -v codex-custom
codex-custom --version
command -v codex-custom-update
codex-custom-update --help | sed -n '1,80p'
```

Expected:

- `codex-custom` resolves to `/Users/huayang/.local/bin/codex-custom`.
- Version matches the remote release version.
- `codex-custom-update` remains available.

- [ ] **Step 12: Final startup smoke test with installed binary**

Run the installed binary in the same environment that previously produced the startup warning flood:

```bash
codex-custom
```

Expected:

- No long invalid-skill warning list in the initial transcript.
- No MCP startup failure stack/details in the initial transcript.
- Optional remote cached MCPs do not block normal startup.
- If the user explicitly sets `verbose`, old diagnostic detail can still be shown.

---

## Self-Review

**Spec coverage**

- User wants no large startup warning flood: covered by Task 2 and Task 3 quiet defaults.
- User wants Claude Code as reference: plan adopts Claude Code’s customization-diagnostics approach and cached/pending MCP startup model.
- User wants optional remote MCP lazy/cached startup in this same goal: covered by Task 4 with conservative eligibility gates.
- User requires remote GitHub compile/package/test and local install only after success: covered by Task 6 Steps 5-12.
- User requires long build polling intervals: covered by Task 6 Step 7.
- User wants a goal-executable plan: tasks are sequenced, testable, and include commands/commit points.

**Placeholder scan**

- No TBD/TODO placeholders remain.
- The only adaptive wording is constrained to existing test helper names where the local test harness may differ after upstream changes.

**Type consistency**

- `StartupNoticeLevel` is defined in Task 1 and consumed by Tasks 2 and 3.
- `Config::tui_startup_notices` is defined in Task 1 and consumed by TUI functions in Tasks 2 and 3.
- `McpStartupMode` and `Config::mcp_startup_mode` are defined in Task 1 and consumed by Task 4.
- Remote release tag resolution in Task 6 is tied to `origin/feature/claude-style-statusline` short SHA, preventing accidental install of an unrelated latest release.
