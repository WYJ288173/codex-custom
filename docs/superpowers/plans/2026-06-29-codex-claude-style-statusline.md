# Codex CLI Claude-Style Status Line Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build, install, and publish a custom Codex CLI with the approved Claude-style colored, responsive status line and account-wide token statistics.

**Architecture:** Extend Codex's existing status-surface pipeline with an opt-in `claude` layout. Reuse in-memory thread usage, context, Git, and rate-limit state; asynchronously reuse the existing `account/tokenUsage/read` app-server endpoint for Daily, Weekly, Monthly, and Total values. Generalize the footer from one `Line` to multiple lines while preserving all existing single-line behavior.

**Tech Stack:** Rust, Tokio, Ratatui, Chrono, Cargo/Just, zsh wrappers, GitHub CLI.

## Global Constraints

- Base the custom branch on official tag `rust-v0.142.0`, matching the installed `0.142.x` CLI line.
- Source checkout: `/Users/huayang/developer/codex-custom`.
- Remote repository name: `codex-custom`; create it as private in the active personal GitHub account.
- Default `codex` command must invoke the custom build; `codex-official` must remain available.
- The custom layout is opt-in through `[tui] status_line_layout = "claude"`; upstream single-line behavior remains the default.
- Wide terminals (`>= 140` columns) use two lines; `80–139` columns use three lines; narrower terminals truncate safely without panicking.
- Account statistics come from `account/tokenUsage/read`; do not scan session transcript files.
- Do not persist account usage to disk in the first version.
- Account usage refresh is asynchronous and runs at startup and no more frequently than every five minutes.
- Missing or failed account data renders `—` or retains the last successful in-process snapshot.
- Use Ratatui `Span` styles; do not embed ANSI escape sequences in status text.
- Parse no conversation content and change no model, sandbox, approval, or networking behavior.

---

### Task 1: Bootstrap the Personal Fork and Add the Layout Configuration

**Files:**
- Modify: `codex-rs/config/src/types.rs`
- Modify: `codex-rs/core/src/config/mod.rs`
- Modify: `codex-rs/core/src/config/config_tests.rs`
- Modify: `codex-rs/Cargo.toml`
- Modify: `codex-rs/Cargo.lock`
- Regenerate: `codex-rs/core/config.schema.json`
- Create: `docs/superpowers/specs/2026-06-29-codex-claude-style-statusline-design.md`
- Create: `docs/superpowers/plans/2026-06-29-codex-claude-style-statusline.md`

**Interfaces:**
- Produces: `codex_config::types::StatusLineLayout::{SingleLine, Claude}`
- Produces: `Config::tui_status_line_layout: StatusLineLayout`

- [ ] **Step 1: Restore GitHub authentication**

Run:

```bash
gh auth login -h github.com -p https -w
gh auth status
```

Expected: the browser authorization completes and `gh auth status` reports one active authenticated personal account.

- [ ] **Step 2: Create the source checkout and remotes**

Run:

```bash
git clone --branch rust-v0.142.0 https://github.com/openai/codex.git /Users/huayang/developer/codex-custom
cd /Users/huayang/developer/codex-custom
git remote rename origin upstream
git switch -c feature/claude-style-statusline
gh repo create codex-custom --private --source=. --remote=origin
```

Expected: `upstream` points to `openai/codex`, `origin` points to the authenticated user's private `codex-custom` repository, and the active branch is `feature/claude-style-statusline`.

- [ ] **Step 3: Add the approved spec and this implementation plan**

Use `apply_patch` to add the two approved Markdown files under the checkout's `docs/superpowers/specs/` and `docs/superpowers/plans/` paths with content identical to the reviewed files in `/Users/huayang/docs/superpowers/`.

- [ ] **Step 4: Mark the source build as custom**

Use `apply_patch` to change `[workspace.package]` in `codex-rs/Cargo.toml`:

```toml
version = "0.142.0-custom.1"
```

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo check -p codex-cli
cargo run -p codex-cli -- --version
```

Expected: Cargo updates `Cargo.lock`, the check exits zero, and version output contains `0.142.0-custom.1`.

- [ ] **Step 5: Write failing configuration tests**

Add to `codex-rs/core/src/config/config_tests.rs`:

```rust
use codex_config::types::StatusLineLayout;

#[test]
fn config_toml_status_line_layout_defaults_to_single_line() {
    let cfg: ConfigToml = toml::from_str("[tui]\n")
        .expect("TOML deserialization should succeed for TUI config");
    assert_eq!(
        cfg.tui.expect("tui config should deserialize").status_line_layout,
        StatusLineLayout::SingleLine
    );
}

#[test]
fn config_toml_deserializes_claude_status_line_layout() {
    let cfg: ConfigToml = toml::from_str(
        r#"
[tui]
status_line_layout = "claude"
"#,
    )
    .expect("TOML deserialization should succeed for TUI config");
    assert_eq!(
        cfg.tui.expect("tui config should deserialize").status_line_layout,
        StatusLineLayout::Claude
    );
}
```

- [ ] **Step 6: Run the focused tests and verify failure**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo test -p codex-core config_toml_status_line_layout -- --nocapture
```

Expected: compilation fails because `StatusLineLayout` and `tui_status_line_layout` do not exist.

- [ ] **Step 7: Implement the configuration type and mapping**

Add to `codex-rs/config/src/types.rs`:

```rust
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum StatusLineLayout {
    #[default]
    SingleLine,
    Claude,
}
```

Add this field to `Tui`:

```rust
/// Selects the footer status-line layout.
#[serde(default)]
pub status_line_layout: StatusLineLayout,
```

Import `StatusLineLayout` in `codex-rs/core/src/config/mod.rs`, add this field to `Config`:

```rust
pub tui_status_line_layout: StatusLineLayout,
```

Map it when resolving configuration:

```rust
tui_status_line_layout: cfg
    .tui
    .as_ref()
    .map(|t| t.status_line_layout)
    .unwrap_or_default(),
```

Update every explicit `Tui { ... }` test fixture with:

```rust
status_line_layout: StatusLineLayout::SingleLine,
```

Update explicit `Config { ... }` fixtures, including `codex-rs/thread-manager-sample/src/main.rs`, with:

```rust
tui_status_line_layout: StatusLineLayout::SingleLine,
```

- [ ] **Step 8: Regenerate schema and run focused tests**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
just write-config-schema
cargo test -p codex-core config_toml_status_line_layout -- --nocapture
```

Expected: both configuration tests pass and `config.schema.json` documents `single-line` and `claude`.

- [ ] **Step 9: Commit and publish the bootstrap**

Run:

```bash
cd /Users/huayang/developer/codex-custom
git add codex-rs/Cargo.toml codex-rs/Cargo.lock codex-rs/config/src/types.rs codex-rs/core/src/config/mod.rs codex-rs/core/src/config/config_tests.rs codex-rs/core/config.schema.json docs/superpowers
git commit -m "feat(tui): add Claude status line layout option"
git push -u origin feature/claude-style-statusline
```

Expected: the first commit is visible on the personal remote branch.

---

### Task 2: Summarize Account Token Usage

**Files:**
- Create: `codex-rs/tui/src/status_line_account_usage.rs`
- Modify: `codex-rs/tui/src/lib.rs`

**Interfaces:**
- Consumes: `codex_app_server_protocol::GetAccountTokenUsageResponse`
- Produces: `AccountUsageSummary { daily, weekly, monthly, total }`
- Produces: `summarize_account_usage(response, today) -> AccountUsageSummary`

- [ ] **Step 1: Add the module with failing tests**

Create `codex-rs/tui/src/status_line_account_usage.rs` with the public data shape and tests:

```rust
use chrono::Datelike;
use chrono::Duration;
use chrono::NaiveDate;
use codex_app_server_protocol::GetAccountTokenUsageResponse;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AccountUsageSummary {
    pub(crate) daily: Option<i64>,
    pub(crate) weekly: Option<i64>,
    pub(crate) monthly: Option<i64>,
    pub(crate) total: Option<i64>,
}

pub(crate) fn summarize_account_usage(
    response: &GetAccountTokenUsageResponse,
    today: NaiveDate,
) -> AccountUsageSummary {
    let _ = (response, today);
    AccountUsageSummary::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AccountTokenUsageDailyBucket;
    use codex_app_server_protocol::AccountTokenUsageSummary;

    fn response() -> GetAccountTokenUsageResponse {
        GetAccountTokenUsageResponse {
            summary: AccountTokenUsageSummary {
                lifetime_tokens: Some(42_100_000),
                peak_daily_tokens: None,
                longest_running_turn_sec: None,
                current_streak_days: None,
                longest_streak_days: None,
            },
            daily_usage_buckets: Some(vec![
                AccountTokenUsageDailyBucket {
                    start_date: "2026-05-31".into(),
                    tokens: 100,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-01".into(),
                    tokens: 200,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-22".into(),
                    tokens: 300,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-28".into(),
                    tokens: 400,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-29".into(),
                    tokens: 500,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "invalid".into(),
                    tokens: 99_999,
                },
            ]),
        }
    }

    #[test]
    fn summarizes_today_monday_week_month_and_lifetime() {
        let result = summarize_account_usage(
            &response(),
            NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
        );
        assert_eq!(
            result,
            AccountUsageSummary {
                daily: Some(500),
                weekly: Some(500),
                monthly: Some(1_400),
                total: Some(42_100_000),
            }
        );
    }

    #[test]
    fn missing_buckets_preserves_lifetime_only() {
        let mut response = response();
        response.daily_usage_buckets = None;
        let result = summarize_account_usage(
            &response,
            NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
        );
        assert_eq!(result.daily, None);
        assert_eq!(result.weekly, None);
        assert_eq!(result.monthly, None);
        assert_eq!(result.total, Some(42_100_000));
    }
}
```

Register it in `codex-rs/tui/src/lib.rs`:

```rust
mod status_line_account_usage;
```

- [ ] **Step 2: Run tests and verify the red state**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo test -p codex-tui status_line_account_usage -- --nocapture
```

Expected: `summarizes_today_monday_week_month_and_lifetime` fails because the actual summary contains only `None` values.

- [ ] **Step 3: Implement date-bucket aggregation**

Replace the unimplemented function with:

```rust
pub(crate) fn summarize_account_usage(
    response: &GetAccountTokenUsageResponse,
    today: NaiveDate,
) -> AccountUsageSummary {
    let Some(buckets) = response.daily_usage_buckets.as_ref() else {
        return AccountUsageSummary {
            total: response.summary.lifetime_tokens,
            ..Default::default()
        };
    };

    let week_start = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let month_start = today.with_day(1).expect("day one exists");
    let parsed = buckets.iter().filter_map(|bucket| {
        NaiveDate::parse_from_str(&bucket.start_date, "%Y-%m-%d")
            .ok()
            .map(|date| (date, bucket.tokens.max(0)))
    });

    let mut daily = 0;
    let mut weekly = 0;
    let mut monthly = 0;
    for (date, tokens) in parsed {
        if date == today {
            daily += tokens;
        }
        if (week_start..=today).contains(&date) {
            weekly += tokens;
        }
        if (month_start..=today).contains(&date) {
            monthly += tokens;
        }
    }

    AccountUsageSummary {
        daily: Some(daily),
        weekly: Some(weekly),
        monthly: Some(monthly),
        total: response.summary.lifetime_tokens.map(|value| value.max(0)),
    }
}
```

- [ ] **Step 4: Run tests and commit**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo test -p codex-tui status_line_account_usage -- --nocapture
cd ..
git add codex-rs/tui/src/status_line_account_usage.rs codex-rs/tui/src/lib.rs
git commit -m "feat(tui): summarize account token usage"
```

Expected: focused tests pass and the commit contains only the pure aggregation module.

---

### Task 3: Fetch and Retain Account Usage Asynchronously

**Files:**
- Modify: `codex-rs/tui/src/app_event.rs`
- Modify: `codex-rs/tui/src/app/background_requests.rs`
- Modify: `codex-rs/tui/src/app/event_dispatch.rs`
- Modify: `codex-rs/tui/src/chatwidget.rs`
- Modify: `codex-rs/tui/src/chatwidget/status_controls.rs`
- Test: `codex-rs/tui/src/app/tests.rs`

**Interfaces:**
- Produces: `AppEvent::RefreshStatusLineAccountUsage { request_id }`
- Produces: `AppEvent::StatusLineAccountUsageLoaded { request_id, result }`
- Produces: `ChatWidget::finish_status_line_account_usage_refresh(...) -> bool`
- Maintains: last successful `AccountUsageSummary` and a five-minute refresh deadline

- [ ] **Step 1: Add a failing app-level lifecycle test**

In `codex-rs/tui/src/app/tests.rs`, construct a chat widget configured with `StatusLineLayout::Claude`, trigger status-line setup, capture `RefreshStatusLineAccountUsage`, then finish with a fixed `GetAccountTokenUsageResponse`. Assert that:

```rust
assert_matches!(
    rx.try_recv(),
    Ok(AppEvent::RefreshStatusLineAccountUsage { .. })
);
assert!(app.chat_widget.status_line_text().is_some());
assert!(app.chat_widget.status_line_text().unwrap().contains("Today"));
```

Also add a second test that finishes a successful response, then an error, and asserts the previous numeric summary remains visible.

- [ ] **Step 2: Run the lifecycle tests and verify failure**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo test -p codex-tui status_line_account_usage_refresh -- --nocapture
```

Expected: compilation fails because the new events and completion method are missing.

- [ ] **Step 3: Add events and the background request**

Add to `AppEvent`:

```rust
RefreshStatusLineAccountUsage {
    request_id: u64,
},
StatusLineAccountUsageLoaded {
    request_id: u64,
    result: Result<GetAccountTokenUsageResponse, String>,
},
```

Add `App::refresh_status_line_account_usage` beside `refresh_token_activity` in `background_requests.rs`. Reuse `TOKEN_ACTIVITY_FETCH_TIMEOUT` and `fetch_account_token_activity`, but send `StatusLineAccountUsageLoaded`.

Add both dispatch arms in `event_dispatch.rs`: the request arm starts the task; the loaded arm logs failures and calls `finish_status_line_account_usage_refresh`.

- [ ] **Step 4: Add ChatWidget state and refresh gating**

Add fields to `ChatWidget` and its construction sites:

```rust
status_line_account_usage: Option<AccountUsageSummary>,
status_line_account_usage_pending_request_id: Option<u64>,
status_line_account_usage_last_requested_at: Option<Instant>,
next_status_line_account_usage_request_id: u64,
```

Use:

```rust
const STATUS_LINE_ACCOUNT_USAGE_REFRESH_INTERVAL: Duration =
    Duration::from_secs(5 * 60);
```

Implement request gating so Claude layout requests immediately when no request has succeeded, then at most once every five minutes. On success, replace the snapshot; on failure, retain the prior snapshot. Every completion calls `refresh_status_line()` and `request_redraw()`.

- [ ] **Step 5: Run focused and adjacent tests**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo test -p codex-tui status_line_account_usage_refresh -- --nocapture
cargo test -p codex-tui token_activity -- --nocapture
```

Expected: new lifecycle tests pass and existing `/usage` token activity tests remain green.

- [ ] **Step 6: Commit**

Run:

```bash
cd /Users/huayang/developer/codex-custom
git add codex-rs/tui/src/app_event.rs codex-rs/tui/src/app/background_requests.rs codex-rs/tui/src/app/event_dispatch.rs codex-rs/tui/src/chatwidget.rs codex-rs/tui/src/chatwidget/status_controls.rs codex-rs/tui/src/app/tests.rs
git commit -m "feat(tui): refresh status line account usage"
```

---

### Task 4: Generalize the Footer to Multiple Status Lines

**Files:**
- Modify: `codex-rs/tui/src/bottom_pane/chat_composer/footer_state.rs`
- Modify: `codex-rs/tui/src/bottom_pane/chat_composer.rs`
- Modify: `codex-rs/tui/src/bottom_pane/footer.rs`
- Modify: `codex-rs/tui/src/bottom_pane/mod.rs`
- Test: `codex-rs/tui/src/bottom_pane/footer.rs`

**Interfaces:**
- Produces: `type StatusLineValue = Vec<Line<'static>>`
- Changes: `set_status_line(Option<StatusLineValue>) -> bool`
- Produces: `passive_footer_status_lines(&FooterProps) -> Vec<Line<'static>>`

- [ ] **Step 1: Add failing two- and three-line footer snapshot tests**

Add footer test fixtures with:

```rust
status_line_value: Some(vec![
    Line::from("model · cwd · branch"),
    Line::from("context · session · account"),
]),
```

Assert `footer_height(&props) == 2` and snapshot both two-line and three-line cases. Add a regression case with one line and assert the existing snapshot stays unchanged.

- [ ] **Step 2: Run footer tests and verify failure**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo test -p codex-tui footer_status_line -- --nocapture
```

Expected: compilation fails because `status_line_value` still accepts one `Line`.

- [ ] **Step 3: Convert footer storage and passive rendering to vectors**

Define in `bottom_pane/mod.rs`:

```rust
pub(crate) type StatusLineValue = Vec<ratatui::text::Line<'static>>;
```

Replace `Option<Line<'static>>` status-line fields and setters with `Option<StatusLineValue>`. Replace `passive_footer_status_line` with `passive_footer_status_lines`.

When an active agent label exists, append it to the first status line only. Return all status lines from `footer_from_props_lines`; `footer_height` then naturally returns two or three.

- [ ] **Step 4: Update width-collapse behavior**

For one-line status values, preserve existing right-indicator and ellipsis behavior byte-for-byte. For multi-line values:

- Render all status lines through `render_footer_from_props`.
- Truncate each line independently with `truncate_line_with_ellipsis_if_overflow`.
- Render collaboration/goal context only when it fits on the first line; never overwrite lower metric lines.

- [ ] **Step 5: Run footer and composer tests**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo test -p codex-tui footer_status_line -- --nocapture
cargo test -p codex-tui chat_composer -- --nocapture
```

Expected: new multi-line snapshots pass and existing single-line snapshots remain unchanged.

- [ ] **Step 6: Commit**

Run:

```bash
cd /Users/huayang/developer/codex-custom
git add codex-rs/tui/src/bottom_pane
git commit -m "refactor(tui): support multiline status values"
```

---

### Task 5: Render the Approved Claude-Style Layout

**Files:**
- Create: `codex-rs/tui/src/bottom_pane/claude_status_line.rs`
- Modify: `codex-rs/tui/src/bottom_pane/mod.rs`
- Modify: `codex-rs/tui/src/chatwidget/status_surfaces.rs`
- Modify: `codex-rs/tui/src/chatwidget.rs`
- Test: `codex-rs/tui/src/bottom_pane/claude_status_line.rs`

**Interfaces:**
- Produces: `ClaudeStatusLineData`
- Produces: `render_claude_status_line(data, width) -> StatusLineValue`
- Consumes: model, reasoning, cwd, branch, context used, thread input/output, account summary, five-hour limit, weekly limit

- [ ] **Step 1: Add formatter and layout tests before implementation**

Create `claude_status_line.rs` with:

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ClaudeStatusLineData {
    pub(crate) model: String,
    pub(crate) reasoning: Option<String>,
    pub(crate) current_dir: String,
    pub(crate) git_branch: Option<String>,
    pub(crate) context_used_percent: Option<i64>,
    pub(crate) session_input_tokens: i64,
    pub(crate) session_output_tokens: i64,
    pub(crate) account_usage: Option<AccountUsageSummary>,
    pub(crate) five_hour_limit: Option<String>,
    pub(crate) weekly_limit: Option<String>,
}

pub(crate) fn render_claude_status_line(
    data: &ClaudeStatusLineData,
    width: u16,
) -> StatusLineValue {
    let _ = (data, width);
    Vec::new()
}
```

Add tests that assert:

- Width `180` returns two lines with model/path/branch on line one and all metrics on line two.
- Width `100` returns three lines and retains Today, Week, Month, and Total.
- Width `60` returns three lines, each has width at most 60, and no panic occurs.
- Context `31` renders exactly ten cells: `▓▓▓░░░░░░░`.
- Missing account usage renders `Today —`, `Week —`, `Month —`, `Total —`.
- Model is green, context purple, path cyan, input blue, output/weekly yellow, and total red.

- [ ] **Step 2: Run renderer tests and verify failure**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo test -p codex-tui claude_status_line -- --nocapture
```

Expected: layout assertions fail because the renderer returns zero lines.

- [ ] **Step 3: Implement compact formatting and semantic spans**

Implement:

```rust
fn compact_tokens(value: Option<i64>) -> String
fn context_bar(percent: i64) -> String
fn abbreviate_home_and_parents(path: &str, width: usize) -> String
fn truncate_styled_line(line: Line<'static>, width: usize) -> Line<'static>
```

Use these semantic styles:

```rust
const MODEL: Color = Color::Green;
const CONTEXT: Color = Color::Magenta;
const PATH: Color = Color::Cyan;
const INPUT: Color = Color::Blue;
const OUTPUT: Color = Color::Yellow;
const TOTAL: Color = Color::Red;
const MUTED: Color = Color::DarkGray;
```

Build the approved two-line and three-line forms using styled `Span`s and `│` separators. K/M formatting uses one decimal only when needed (`842K`, `1.2M`, `42.1M`).

- [ ] **Step 4: Wire ChatWidget state into the renderer**

In `refresh_status_line_from_selections`, branch on `self.config.tui_status_line_layout`:

```rust
StatusLineLayout::SingleLine => {
    // existing status_line_from_segments path unchanged
}
StatusLineLayout::Claude => {
    let data = self.claude_status_line_data();
    self.set_status_line(Some(render_claude_status_line(
        &data,
        self.bottom_pane.available_footer_width(),
    )));
}
```

Build `ClaudeStatusLineData` using existing helpers:

- `model_display_name`
- `reasoning_display_name`
- `format_directory_display`
- `status_line_branch`
- `status_line_context_used_percent`
- `status_line_total_usage`
- `status_line_limit_display`
- the new account usage snapshot

Keep `status_line = []` as the global hide switch.

- [ ] **Step 5: Run renderer, status-line, and app tests**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo test -p codex-tui claude_status_line -- --nocapture
cargo test -p codex-tui status_line -- --nocapture
cargo test -p codex-tui token_usage_update_refreshes_status_line -- --nocapture
```

Expected: all focused suites pass.

- [ ] **Step 6: Commit**

Run:

```bash
cd /Users/huayang/developer/codex-custom
git add codex-rs/tui/src/bottom_pane/claude_status_line.rs codex-rs/tui/src/bottom_pane/mod.rs codex-rs/tui/src/chatwidget/status_surfaces.rs codex-rs/tui/src/chatwidget.rs
git commit -m "feat(tui): render Claude-style status dashboard"
```

---

### Task 6: Install, Configure, and Preserve the Official Fallback

**Files:**
- Create: `scripts/install-custom-codex.sh`
- Create: `scripts/update-custom-codex.sh`
- Create: `scripts/codex-official`
- Modify: `/Users/huayang/.codex/config.toml`
- Modify: `/Users/huayang/.zshrc`
- Create: `docs/custom-statusline.md`

**Interfaces:**
- Produces: `/Users/huayang/.local/bin/codex`
- Produces: `/Users/huayang/.local/bin/codex-official`
- Produces: repeatable build/install/update workflow

- [ ] **Step 1: Write failing installer smoke checks**

Before installation, run:

```bash
test -x /Users/huayang/.local/bin/codex
test -x /Users/huayang/.local/bin/codex-official
```

Expected: at least the custom `codex` check fails.

- [ ] **Step 2: Add the official wrapper**

Create `scripts/codex-official` as an executable POSIX shell script that resolves the current FNM global npm root, executes `@openai/codex/bin/codex.js` with Node, and falls back to `/Applications/Codex.app/Contents/Resources/codex` when the npm entry point is absent. Forward arguments with `"$@"` and emit a clear error when neither source exists.

- [ ] **Step 3: Add an atomic installer**

Create `scripts/install-custom-codex.sh` to:

1. Run `cargo build --release -p codex-cli`.
2. Locate `codex-rs/target/release/codex`.
3. Install to `~/.local/bin/codex.new`.
4. Verify `codex.new --version`.
5. Atomically rename it to `~/.local/bin/codex`.
6. Install `scripts/codex-official` to `~/.local/bin/codex-official`.

The script must use `set -eu`, quote all paths, and leave the previous binary untouched if build or verification fails.

- [ ] **Step 4: Add the controlled update script**

Create `scripts/update-custom-codex.sh` to:

1. Require a clean working tree.
2. Fetch `upstream --tags`.
3. Accept an explicit tag argument.
4. Create a temporary update branch.
5. Rebase the custom commits onto that tag.
6. Run `just fmt`, `cargo test -p codex-tui`, and the installer.
7. Fast-forward the feature branch only after success.
8. Delete the temporary branch on failure and preserve the installed binary.

- [ ] **Step 5: Configure the custom layout**

Use `apply_patch` to add under `[tui]` in `/Users/huayang/.codex/config.toml`:

```toml
status_line_layout = "claude"
status_line_use_colors = true
```

Retain the existing `status_line` list and all unrelated user configuration.

Use `apply_patch` to ensure `/Users/huayang/.zshrc` prepends `~/.local/bin` once:

```zsh
export PATH="$HOME/.local/bin:$PATH"
```

- [ ] **Step 6: Build and install**

Run:

```bash
cd /Users/huayang/developer/codex-custom
bash scripts/install-custom-codex.sh
zsh -lic 'command -v codex; codex --version; command -v codex-official; codex-official --version'
```

Expected: `codex` resolves to `~/.local/bin/codex`, the custom version runs, and `codex-official` reports the official version.

- [ ] **Step 7: Write usage and rollback documentation**

Document:

- Required `config.toml` keys.
- Wide/narrow layout examples.
- `codex` versus `codex-official`.
- How to update to an explicit upstream tag.
- How to roll back by removing `~/.local/bin/codex` or moving `~/.local/bin` later in `PATH`.
- The account usage API dependency and offline fallback behavior.

- [ ] **Step 8: Commit**

Run:

```bash
cd /Users/huayang/developer/codex-custom
git add scripts docs/custom-statusline.md
git commit -m "build: install and update custom Codex CLI"
```

Do not commit `/Users/huayang/.codex/config.toml` or `/Users/huayang/.zshrc`.

---

### Task 7: Full Verification and Remote Delivery

**Files:**
- Verify all changed Rust, scripts, docs, and generated schema files.
- No new implementation files unless verification reveals a defect.

**Interfaces:**
- Produces: a tested custom binary installed locally
- Produces: a pushed personal GitHub repository branch

- [ ] **Step 1: Run formatting and static checks**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
just fmt
just fix -p codex-tui
cargo test -p codex-core config_toml_status_line_layout -- --nocapture
cargo test -p codex-tui
```

Expected: formatting produces no remaining diff after a second `just fmt`; all tests pass with zero failures.

- [ ] **Step 2: Verify release build**

Run:

```bash
cd /Users/huayang/developer/codex-custom/codex-rs
cargo build --release -p codex-cli
```

Expected: release build exits zero and produces `target/release/codex`.

- [ ] **Step 3: Run command-entry smoke tests**

Run:

```bash
zsh -lic 'test "$(command -v codex)" = "$HOME/.local/bin/codex"'
zsh -lic 'codex --version'
zsh -lic 'codex-official --version'
```

Expected: all three commands exit zero and the two commands identify custom and official binaries respectively.

- [ ] **Step 4: Perform interactive TUI acceptance**

Start the custom CLI at widths 80, 100, 140, and 180. Confirm:

- 140 and 180 columns render two lines.
- 80 and 100 columns render three lines.
- Model is green; context is purple; path is cyan; session input is blue; output/weekly is yellow; total is red.
- Context bar contains ten cells.
- Session, Daily, Weekly, Monthly, Total, 5-hour, and weekly-limit data appear.
- Account usage loading or failure does not block typing or streaming.
- Git field disappears cleanly outside a repository.

Capture screenshots under `docs/screenshots/` and add them to the final documentation commit.

- [ ] **Step 5: Verify the working tree and commit final artifacts**

Run:

```bash
cd /Users/huayang/developer/codex-custom
git status --short
git diff --check
git add docs/screenshots
git commit -m "docs: add custom status line verification"
```

Expected: `git diff --check` emits no errors. If no screenshot changes exist, skip only the empty commit.

- [ ] **Step 6: Push all code and verify the remote**

Run:

```bash
cd /Users/huayang/developer/codex-custom
git push origin feature/claude-style-statusline
gh repo view --web
git status --short --branch
```

Expected: the browser opens the personal `codex-custom` repository, the remote branch contains all commits, and the local working tree is clean.
