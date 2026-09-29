# Interactive MCP Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan.

**Goal:** Replace bare `/mcp` transcript dumping with a keyboard-driven, layered MCP server manager that supports OAuth Authenticate/Re-authenticate, while preserving `/mcp verbose` and installing only a GitHub-validated `codex-custom` artifact.

**Architecture:** Keep the app-server v2 protocol unchanged. `ChatWidget` owns popup presentation in a focused `mcp_picker` module; `App` owns the one-at-a-time OAuth state machine, typed app-server requests, global completion notification handling, and secret-safe browser side effect. Existing `ListSelectionView`, paginated status inventory, OAuth login, MCP refresh, and verbose history renderer are reused.

**Tech Stack:** Rust, Ratatui TUI, Tokio, Codex app-server v2 typed requests, `insta` snapshots, `just`, GitHub Actions, `gh` CLI.

## Global Constraints

- Implement the approved spec in `docs/superpowers/specs/2026-07-17-interactive-mcp-manager-design.md` without adding project-level `.mcp.json` trust/enable approval.
- Do not add logout, credential clearing, token deletion, or MCP tool-call permission changes.
- Do not add or modify app-server protocol/schema types.
- Never put an OAuth authorization URL into history cells, rollout data, user-facing error text, or tracing fields.
- Accept at most one pending OAuth operation; correlate internal async results by operation ID and app-server completion by server name.
- Preserve `/mcp verbose` through the existing `FetchMcpInventory`/history-renderer path.
- A late inventory or OAuth result must update an existing MCP view only; it must never reopen a view the user closed.
- Follow the repository order at the end: tests, `just fix -p codex-tui`, then `just fmt`; do not claim success from stale output.
- Do not install a locally built binary. Install only the checksummed artifact from a fully green GitHub workflow run.

---

## Task 1: Prepare the Isolated, Up-to-Date Implementation Branch

**Files:**

- Read: `AGENTS.md`
- Read: `codex-rs/tui/AGENTS.md` if present
- Carry forward: `docs/superpowers/specs/2026-07-17-interactive-mcp-manager-design.md`
- Carry forward: `docs/superpowers/plans/2026-07-17-interactive-mcp-manager.md`

**Step 1: Re-read repository instructions and inspect local state**

Run:

```bash
git -C /Users/huayang/developer/codex-custom status --short --branch
git -C /Users/huayang/developer/codex-custom fetch origin upstream
git -C /Users/huayang/developer/codex-custom rev-parse origin/feature/claude-style-statusline
git -C /Users/huayang/developer/codex-custom rev-parse upstream/main
git -C /Users/huayang/developer/codex-custom branch backup/interactive-mcp-manager-origin origin/feature/claude-style-statusline
```

Expected: only the known untracked `.superpowers/` helper directory is unrelated. The local backup ref records the exact remote feature OID for a later lease check; if that backup ref already exists, verify it still points to the intended pre-change remote commit instead of moving it silently.

**Step 2: Use the worktree skill and create the implementation worktree**

Invoke `superpowers:using-git-worktrees`, then create:

```text
/Users/huayang/developer/codex-custom/.worktrees/interactive-mcp-manager
```

on a local branch named `feature/interactive-mcp-manager`, starting at `origin/feature/claude-style-statusline`.

Expected: the main working tree remains on `setup/worktree-ignore` and its unrelated files remain untouched.

**Step 3: Rebase the custom line onto current upstream before feature edits**

Run in the new worktree:

```bash
git rebase upstream/main
```

Expected: the existing custom commits replay cleanly. If conflicts occur, resolve only after reading both the custom intent and upstream change; run the existing custom sync contract before proceeding:

```bash
python3 -m unittest scripts/test_codex_custom_sync.py
```

**Step 4: Bring the approved design and plan commits onto the implementation branch**

Cherry-pick the design and plan commits from `setup/worktree-ignore`, then verify:

```bash
git log -5 --oneline
git status --short
```

Expected: both documents are tracked on the implementation branch and the worktree is clean.

---

## Task 2: Add View-Stack Presence Support for Safe Async Replacement

**Files:**

- Modify: `codex-rs/tui/src/bottom_pane/mod.rs`

**Step 1: Write the failing unit test**

Add a focused test beside the existing view-stack tests proving that a view can be found when it is active or covered by another view, and is absent after dismissal:

```rust
assert!(pane.has_view_id("mcp-list"));
assert!(pane.has_view_id("mcp-list")); // still true under an overlay
assert!(!pane.has_view_id("missing"));
```

**Step 2: Run the focused test and confirm RED**

Run:

```bash
just test -p codex-tui bottom_pane::tests::has_view_id_finds_active_and_covered_views -- --exact
```

Expected: compile failure because `has_view_id` does not exist.

**Step 3: Implement the minimal helper**

Add:

```rust
pub(crate) fn has_view_id(&self, view_id: &'static str) -> bool {
    self.view_stack
        .iter()
        .any(|view| view.view_id() == Some(view_id))
}
```

This helper is the late-result guard used by the MCP picker; do not expose the entire stack.

**Step 4: Run the focused test and confirm GREEN**

Run the same `just test` command. Expected: pass.

**Step 5: Commit**

```bash
git add codex-rs/tui/src/bottom_pane/mod.rs
git commit -m "tui: expose safe popup presence check"
```

---

## Task 3: Build the Compact MCP List and Change Bare `/mcp`

**Files:**

- Create: `codex-rs/tui/src/chatwidget/mcp_picker.rs`
- Create: `codex-rs/tui/src/chatwidget/tests/mcp_picker.rs`
- Modify: `codex-rs/tui/src/chatwidget.rs`
- Modify: `codex-rs/tui/src/chatwidget/tests.rs`
- Modify: `codex-rs/tui/src/chatwidget/slash_dispatch.rs`
- Modify: `codex-rs/tui/src/chatwidget/tests/slash_commands.rs`
- Modify: `codex-rs/tui/src/app_event.rs`
- Modify: `codex-rs/tui/src/app/background_requests.rs`
- Modify: `codex-rs/tui/src/app/event_dispatch.rs`

**Step 1: Add failing slash-command and popup tests**

Change `slash_mcp_requests_inventory_via_app_server` so bare `/mcp` expects a modal and a new interactive event rather than an active transcript spinner:

```rust
assert_eq!(chat.bottom_pane.active_view_id(), Some(MCP_LIST_VIEW_ID));
assert_matches!(
    rx.try_recv(),
    Ok(AppEvent::FetchMcpPickerInventory {
        thread_id: Some(actual_thread_id),
        focus_server: None,
    }) if actual_thread_id == thread_id
);
```

In `tests/mcp_picker.rs`, add snapshot tests for:

- loading;
- a sorted mixed-state list;
- empty inventory;
- inventory error with `Retry`;
- narrow width.

Use fixture statuses containing distinctive tool names such as `secret_tool_name`, and assert the rendered level-1 list does **not** contain them.

**Step 2: Run the focused tests and confirm RED**

Run:

```bash
just test -p codex-tui slash_mcp_requests_inventory_via_app_server -- --exact
just test -p codex-tui mcp_picker -- --nocapture
```

Expected: missing module/event/method failures.

**Step 3: Add the interactive inventory event contract**

Keep the legacy variants unchanged and add separate interactive variants to `AppEvent`:

```rust
FetchMcpPickerInventory {
    thread_id: Option<ThreadId>,
    focus_server: Option<String>,
},
McpPickerInventoryLoaded {
    result: Result<Vec<McpServerStatus>, String>,
    thread_id: Option<ThreadId>,
    focus_server: Option<String>,
},
```

The separate contract prevents `/mcp verbose` from accidentally acquiring modal semantics.

**Step 4: Implement compact state derivation and list builders**

In `mcp_picker.rs`, define stable view IDs and pure state derivation:

```rust
pub(super) const MCP_LIST_VIEW_ID: &str = "mcp-list";
pub(super) const MCP_DETAIL_VIEW_ID: &str = "mcp-detail";
pub(super) const MCP_TOOLS_VIEW_ID: &str = "mcp-tools";
pub(super) const MCP_OAUTH_VIEW_ID: &str = "mcp-oauth";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpConnectionState {
    Connected,
    NeedsAuthentication,
    Unavailable,
}

fn connection_state(status: &McpServerStatus) -> McpConnectionState {
    if status.auth_status == McpAuthStatus::NotLoggedIn {
        McpConnectionState::NeedsAuthentication
    } else if status.server_info.is_some() {
        McpConnectionState::Connected
    } else {
        McpConnectionState::Unavailable
    }
}
```

Implement `open_mcp_picker_loading`, `on_mcp_picker_inventory_loaded`, and list/empty/error `SelectionViewParams`. Sort by server name. Rows contain only name, derived state, relevant auth label, and `N tool(s)`. Use `bottom_pane.has_view_id(MCP_LIST_VIEW_ID)` before replacing a late result.

**Step 5: Route inventory without transcript output**

- Bare `SlashCommand::Mcp` calls `open_mcp_picker_loading()` and sends `FetchMcpPickerInventory`.
- `App::fetch_mcp_picker_inventory` reuses `fetch_all_mcp_server_statuses(..., ToolsAndAuthOnly, ...)` and sends `McpPickerInventoryLoaded`.
- The result handler keeps the existing thread identity guard, then delegates to the picker.
- `Retry` replaces the error with loading and sends the same fetch event. Initial and ordinary retries use `focus_server: None`; a post-authentication reconnect retry uses `Some(server_name)` so a successful retry can return directly to that server's detail.
- Do not add or clear `McpInventoryLoadingCell` for interactive inventory.

**Step 6: Run tests and review snapshots**

Run:

```bash
INSTA_UPDATE=new just test -p codex-tui mcp_picker -- --nocapture
just test -p codex-tui slash_mcp_requests_inventory_via_app_server -- --exact
```

Review every `.snap.new`, confirm no tool names appear on level 1, then accept only the intended snapshots with the repository's normal `insta` workflow.

**Step 7: Commit**

```bash
git add codex-rs/tui/src/chatwidget.rs codex-rs/tui/src/chatwidget/mcp_picker.rs codex-rs/tui/src/chatwidget/tests.rs codex-rs/tui/src/chatwidget/tests/mcp_picker.rs codex-rs/tui/src/chatwidget/slash_dispatch.rs codex-rs/tui/src/chatwidget/tests/slash_commands.rs codex-rs/tui/src/chatwidget/snapshots codex-rs/tui/src/app_event.rs codex-rs/tui/src/app/background_requests.rs codex-rs/tui/src/app/event_dispatch.rs
git commit -m "tui: make mcp command open compact server picker"
```

---

## Task 4: Add Server Detail and Tool-List Navigation

**Files:**

- Modify: `codex-rs/tui/src/chatwidget/mcp_picker.rs`
- Modify: `codex-rs/tui/src/chatwidget/tests/mcp_picker.rs`
- Modify: `codex-rs/tui/src/app_event.rs`
- Modify: `codex-rs/tui/src/app/event_dispatch.rs`
- Modify: `codex-rs/tui/src/chatwidget/snapshots/*.snap`

**Step 1: Write failing navigation and authorization-label tests**

Add tests that drive real keys through the popup:

1. Down selects the second server.
2. Enter emits `OpenMcpServerDetail` for that exact server.
3. The detail snapshot shows state/auth/count.
4. `NotLoggedIn` shows `Authenticate`.
5. `OAuth` shows `Re-authenticate` and contains no clear/logout action.
6. `BearerToken` and `Unsupported` show no OAuth action.
7. `View tools (N)` opens the sorted informational tool list.
8. Esc returns tools → detail → list.

**Step 2: Run the focused tests and confirm RED**

```bash
just test -p codex-tui mcp_picker -- --nocapture
```

Expected: missing detail/tool events and builders.

**Step 3: Add navigation events**

Add:

```rust
OpenMcpServerDetail { server: McpServerStatus },
OpenMcpServerTools { server: McpServerStatus },
```

Each list item clones one status into its action. Event dispatch calls the corresponding `ChatWidget` method. Do not perform another inventory request for the tool view.

**Step 4: Implement the detail and tools views**

Add a pure OAuth action mapping:

```rust
fn oauth_action(auth_status: &McpAuthStatus) -> Option<&'static str> {
    match auth_status {
        McpAuthStatus::NotLoggedIn => Some("Authenticate"),
        McpAuthStatus::OAuth => Some("Re-authenticate"),
        McpAuthStatus::BearerToken | McpAuthStatus::Unsupported => None,
    }
}
```

The detail view pushes above the list. `View tools (N)` pushes a third view with sorted names, disabled informational rows, and no tool-call actions. Rely on the generic stack for ordinary Esc back navigation.

**Step 5: Run tests, review/accept snapshots, and commit**

```bash
INSTA_UPDATE=new just test -p codex-tui mcp_picker -- --nocapture
git add codex-rs/tui/src/chatwidget/mcp_picker.rs codex-rs/tui/src/chatwidget/tests/mcp_picker.rs codex-rs/tui/src/chatwidget/snapshots codex-rs/tui/src/app_event.rs codex-rs/tui/src/app/event_dispatch.rs
git commit -m "tui: add mcp detail and tool navigation"
```

---

## Task 5: Add the Single-Operation OAuth State Machine

**Files:**

- Create: `codex-rs/tui/src/app/mcp_oauth.rs`
- Create: `codex-rs/tui/src/app/mcp_oauth_tests.rs`
- Modify: `codex-rs/tui/src/app.rs`
- Modify: `codex-rs/tui/src/app/test_support.rs`
- Modify: `codex-rs/tui/src/app/tests.rs`
- Modify: `codex-rs/tui/src/app_event.rs`
- Modify: `codex-rs/tui/src/app/event_dispatch.rs`
- Modify: `codex-rs/tui/src/chatwidget/mcp_picker.rs`
- Modify: `codex-rs/tui/src/chatwidget/tests/mcp_picker.rs`

**Step 1: Write failing state-machine and progress-view tests**

Add tests for:

- building `ClientRequest::McpServerOauthLogin` with the selected server, `scopes: None`, and `timeout_secs: None`;
- formatting the internal authorization-URL wrapper with `Debug` produces `[REDACTED]`, not its URL or state parameter;
- a first operation entering `RequestingUrl`;
- a second start being rejected without replacing the pending operation;
- an operation-ID mismatch being ignored;
- progress snapshot with `Open browser again` and `Close`;
- Esc from starting/progress sending `DismissMcpViews` and closing the whole MCP stack;
- no URL string appearing in rendered progress/error output.

**Step 2: Run the focused tests and confirm RED**

```bash
just test -p codex-tui mcp_oauth -- --nocapture
just test -p codex-tui mcp_picker -- --nocapture
```

Expected: missing state, events, and views.

**Step 3: Define app-owned pending state**

In `app_event.rs`, add a narrow in-memory wrapper whose `Debug` implementation is always redacted and whose inner string is exposed only to `app/mcp_oauth.rs`:

```rust
#[derive(Clone)]
pub(crate) struct McpOauthAuthorizationUrl(String);

impl std::fmt::Debug for McpOauthAuthorizationUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("McpOauthAuthorizationUrl([REDACTED])")
    }
}
```

Provide crate-private `new` and `as_str` methods. Wrap `McpServerOauthLoginResponse::authorization_url` immediately inside the spawned task, before sending it through `AppEvent`.

In `app/mcp_oauth.rs` add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PendingMcpOauthPhase {
    RequestingUrl,
    WaitingForCompletion,
    Refreshing,
}

#[derive(Clone)]
pub(super) struct PendingMcpOauth {
    pub(super) operation_id: String,
    pub(super) server: McpServerStatus,
    pub(super) authorization_url: Option<McpOauthAuthorizationUrl>,
    pub(super) phase: PendingMcpOauthPhase,
}
```

Add `pending_mcp_oauth: Option<PendingMcpOauth>` to `App` and initialize it to `None` in the production constructor and every test fixture constructor found by `rg "pending_hook_enabled_writes" codex-rs/tui/src/app*`.

**Step 4: Add OAuth events and start request**

Add events carrying only the minimum data:

```rust
StartMcpOauth { server: McpServerStatus },
McpOauthLoginStarted {
    operation_id: String,
    result: Result<McpOauthAuthorizationUrl, String>,
},
OpenPendingMcpOauthUrl { operation_id: String },
DismissMcpViews,
```

`StartMcpOauth` must record the pending operation **before** spawning the typed request. Use the same operation ID as the JSON-RPC `RequestId::String`, show the starting view, and request:

```rust
ClientRequest::McpServerOauthLogin {
    request_id: RequestId::String(operation_id.clone()),
    params: McpServerOauthLoginParams {
        name: server.name.clone(),
        scopes: None,
        timeout_secs: None,
    },
}
```

On request failure, clear only the matching operation. On success, retain the redacted URL wrapper only in `PendingMcpOauth`, move to `WaitingForCompletion`, show progress, and trigger the secret-safe browser path in Task 6.

**Step 5: Implement MCP stack dismissal**

The starting/progress/error `SelectionViewParams` use `on_cancel` to send `DismissMcpViews`. Its handler repeatedly dismisses the four known MCP view IDs without touching unrelated overlays. Closing views never clears `pending_mcp_oauth` and never cancels the app-server listener.

**Step 6: Run tests and commit**

```bash
INSTA_UPDATE=new just test -p codex-tui mcp_oauth -- --nocapture
INSTA_UPDATE=new just test -p codex-tui mcp_picker -- --nocapture
git add codex-rs/tui/src/app.rs codex-rs/tui/src/app/mcp_oauth.rs codex-rs/tui/src/app/mcp_oauth_tests.rs codex-rs/tui/src/app/test_support.rs codex-rs/tui/src/app/tests.rs codex-rs/tui/src/app_event.rs codex-rs/tui/src/app/event_dispatch.rs codex-rs/tui/src/chatwidget/mcp_picker.rs codex-rs/tui/src/chatwidget/tests/mcp_picker.rs codex-rs/tui/src/chatwidget/snapshots
git commit -m "tui: add mcp oauth flow state"
```

---

## Task 6: Open OAuth URLs Without Persisting Them

**Files:**

- Modify: `codex-rs/tui/src/app/mcp_oauth.rs`
- Modify: `codex-rs/tui/src/app/mcp_oauth_tests.rs`
- Modify: `codex-rs/tui/src/chatwidget/mcp_picker.rs`
- Modify: `codex-rs/tui/src/chatwidget/tests/mcp_picker.rs`

**Step 1: Write failing secret-safety tests**

Use a conspicuous URL such as `https://oauth.example/authorize?state=DO_NOT_PERSIST` and an injected test opener. Prove:

- the opener receives the exact in-memory URL;
- successful open adds no history cell/message;
- failed open renders only `Failed to open browser: simulated opener failure` and never renders the URL/state;
- `Open browser again` uses the same pending operation and URL;
- a stale operation ID cannot open another operation's URL.

**Step 2: Run and confirm RED**

```bash
just test -p codex-tui mcp_oauth_url -- --nocapture
```

**Step 3: Implement a testable MCP-specific opener**

Keep the existing general `OpenUrlInBrowser` behavior unchanged. In `mcp_oauth.rs`, implement a production wrapper over `webbrowser::open` and a private generic/helper that accepts an opener for tests. Look up the URL from `pending_mcp_oauth` by operation ID; never put it in `AppEvent` beyond the internal login-result event and never format it into a message.

On browser failure, update the existing OAuth view with a sanitized error and retain `Open browser again`. If the MCP panel was closed, add only a short sanitized failure message; do not reopen it.

**Step 4: Run tests and commit**

```bash
INSTA_UPDATE=new just test -p codex-tui mcp_oauth_url -- --nocapture
git add codex-rs/tui/src/app/mcp_oauth.rs codex-rs/tui/src/app/mcp_oauth_tests.rs codex-rs/tui/src/chatwidget/mcp_picker.rs codex-rs/tui/src/chatwidget/tests/mcp_picker.rs codex-rs/tui/src/chatwidget/snapshots
git commit -m "tui: keep mcp oauth urls out of history"
```

---

## Task 7: Handle OAuth Completion, Refresh, and Late Results

**Files:**

- Modify: `codex-rs/tui/src/app/app_server_events.rs`
- Modify: `codex-rs/tui/src/app/mcp_oauth.rs`
- Modify: `codex-rs/tui/src/app/mcp_oauth_tests.rs`
- Modify: `codex-rs/tui/src/app_event.rs`
- Modify: `codex-rs/tui/src/app/event_dispatch.rs`
- Modify: `codex-rs/tui/src/chatwidget/mcp_picker.rs`
- Modify: `codex-rs/tui/src/chatwidget/tests/mcp_picker.rs`

**Step 1: Write failing completion-transition tests**

Cover:

- a completion for another server is ignored and leaves pending state unchanged;
- matching failure clears pending and shows Retry/Close when visible;
- matching failure after close adds one sanitized history message and opens no modal;
- matching success moves to `Refreshing` and constructs `ClientRequest::McpServerRefresh { params: None }`;
- a late login-URL response after completion is ignored by operation/phase checks;
- refresh/inventory success selects the freshly returned matching server and restores detail only when an MCP view still exists;
- completion after close writes one success message only;
- refresh failure says authentication succeeded but reconnection failed, without losing retry/close behavior.

**Step 2: Run and confirm RED**

```bash
just test -p codex-tui mcp_oauth_completion -- --nocapture
```

**Step 3: Intercept the existing global notification**

In `handle_server_notification_event`, add a dedicated branch before generic routing:

```rust
ServerNotification::McpServerOauthLoginCompleted(notification) => {
    self.handle_mcp_oauth_login_completed(app_server_client, notification.clone());
    return;
}
```

Do not change protocol routing or the ChatWidget's exhaustive no-op arm; the app-level branch owns this global state transition.

**Step 4: Refresh and reload inventory on success**

Spawn a typed request:

```rust
let _: McpServerRefreshResponse = request_handle
    .request_typed(ClientRequest::McpServerRefresh {
        request_id: RequestId::String(format!("{operation_id}-refresh")),
        params: None,
    })
    .await?;
```

Then call the existing paginated `fetch_all_mcp_server_statuses` with `ToolsAndAuthOnly` and the current guarded thread ID. Send a correlated result event:

```rust
McpOauthRefreshFinished {
    operation_id: String,
    result: Result<Vec<McpServerStatus>, String>,
}
```

On success, find the original server by exact name. If an MCP OAuth/list view is still present, replace the OAuth view with fresh detail; otherwise add a short message such as `Authenticated MCP server 'sentry'.` to history and leave all modals closed. Clear pending only after processing the matching refresh result.

**Step 5: Implement Retry semantics**

- OAuth failure Retry starts a new operation after the old pending state is cleared.
- Refresh failure Retry re-fetches inventory; it does not delete credentials or automatically run OAuth again.
- `Close` and Esc dismiss all MCP views but allow any already-running operation to complete.

**Step 6: Run tests, accept snapshots, and commit**

```bash
INSTA_UPDATE=new just test -p codex-tui mcp_oauth_completion -- --nocapture
INSTA_UPDATE=new just test -p codex-tui mcp_picker -- --nocapture
git add codex-rs/tui/src/app/app_server_events.rs codex-rs/tui/src/app/mcp_oauth.rs codex-rs/tui/src/app/mcp_oauth_tests.rs codex-rs/tui/src/app_event.rs codex-rs/tui/src/app/event_dispatch.rs codex-rs/tui/src/chatwidget/mcp_picker.rs codex-rs/tui/src/chatwidget/tests/mcp_picker.rs codex-rs/tui/src/chatwidget/snapshots
git commit -m "tui: complete and refresh mcp oauth login"
```

---

## Task 8: Preserve Verbose Compatibility and Run Full Local Verification

**Files:**

- Modify if needed: `codex-rs/tui/src/chatwidget/tests/slash_commands.rs`
- Modify if needed: `codex-rs/tui/src/history_cell/tests.rs`
- Modify if needed: affected `codex-rs/tui/src/**/snapshots/*.snap`

**Step 1: Add/retain explicit regression tests**

Prove:

- `/mcp verbose` still emits `FetchMcpInventory { detail: Full, ... }` and uses the transcript loading cell;
- invalid `/mcp` arguments retain the usage error;
- the legacy history renderer still lists tools/resources/templates;
- bare `/mcp` creates no history loading/output cells;
- first-level interactive snapshots contain no tool names, OAuth URLs, headers, or environment values.

**Step 2: Run focused suites**

```bash
just test -p codex-tui mcp_picker -- --nocapture
just test -p codex-tui mcp_oauth -- --nocapture
just test -p codex-tui slash_mcp -- --nocapture
```

Expected: all pass with no unreviewed `.snap.new` files.

**Step 3: Run the full scoped crate test**

```bash
just test -p codex-tui
```

Expected: pass.

**Step 4: Apply repository fix/format commands last**

```bash
just fix -p codex-tui
just fmt
```

Per repository instructions, inspect the resulting diff rather than rerunning tests after these commands.

**Step 5: Audit secrets, scope, and diff**

Run:

```bash
rg -n "authorization_url|Opened .*browser|DO_NOT_PERSIST" codex-rs/tui/src
git diff --check
git status --short
git diff --stat upstream/main...HEAD
```

Expected: authorization URLs exist only in in-memory state/request handling and test fixtures; no production formatting/logging includes them; no `.snap.new` or unrelated files remain.

**Step 6: Commit any verification-only adjustments**

```bash
git add codex-rs/tui
git commit -m "test: cover interactive mcp manager regressions"
```

Skip this commit if there are no post-verification changes.

---

## Task 9: Review, Push Safely, and Require a Green GitHub Release Build

**Files:**

- Verify: `.github/workflows/codex-custom-sync.yml`
- Verify: `scripts/test_codex_custom_sync.py`
- Verify: `scripts/install-codex-custom.sh`

**Step 1: Invoke completion verification and code-review skills**

Use `superpowers:verification-before-completion`, then `superpowers:requesting-code-review`. Resolve any real findings with new focused tests and commits; do not weaken assertions to make review pass.

**Step 2: Re-run the custom workflow contract locally**

```bash
python3 -m unittest scripts/test_codex_custom_sync.py
git status --short
```

Expected: contract passes and branch is clean.

**Step 3: Check the remote lease and push the reviewed branch**

Re-read `origin/feature/claude-style-statusline` and compare it with the backup ref recorded in Task 1. If unchanged, push with an exact lease:

```bash
git fetch origin feature/claude-style-statusline
mcp_expected_remote_oid="$(git rev-parse backup/interactive-mcp-manager-origin)"
test "$(git rev-parse origin/feature/claude-style-statusline)" = "$mcp_expected_remote_oid"
git push --force-with-lease=refs/heads/feature/claude-style-statusline:"$mcp_expected_remote_oid" origin HEAD:feature/claude-style-statusline
```

If the remote changed, stop and inspect/reconcile it; never overwrite unknown work.

**Step 4: Trigger the release-enabled workflow**

```bash
mcp_release_head="$(git rev-parse HEAD)"
gh workflow run codex-custom-sync.yml --repo WYJ288173/codex-custom --ref feature/claude-style-statusline -f publish_release=true
for mcp_lookup_attempt in {1..12}; do
  mcp_run_id="$(gh run list --repo WYJ288173/codex-custom --workflow codex-custom-sync.yml --branch feature/claude-style-statusline --event workflow_dispatch --commit "$mcp_release_head" --limit 1 --json databaseId --jq '.[0].databaseId')"
  test -n "$mcp_run_id" && break
  sleep 5
done
test -n "$mcp_run_id"
gh run view "$mcp_run_id" --repo WYJ288173/codex-custom --json databaseId,headSha,event,status,url
```

Confirm the displayed `headSha` equals `mcp_release_head`, then watch that exact run:

```bash
gh run watch "$mcp_run_id" --repo WYJ288173/codex-custom --exit-status
```

Expected: rebase, sync contract, format check, `codex-tui` tests, `codex-cli` tests, Apple Silicon release build, packaging, branch push, and prerelease creation all succeed.

**Step 5: Stop on any remote failure**

If the run fails:

```bash
gh run view "$mcp_run_id" --repo WYJ288173/codex-custom --log-failed
```

Diagnose with `superpowers:systematic-debugging`, fix locally with a regression test, push again with a refreshed exact lease, and trigger a new run. Do **not** install anything until one exact run is fully green.

**Step 6: Verify release identity and assets**

From the successful run, fetch the workflow-updated branch and derive the unique release tag suffix from its 12-character commit ID. Verify:

```bash
git fetch origin feature/claude-style-statusline
mcp_release_short_sha="$(git rev-parse --short=12 origin/feature/claude-style-statusline)"
mcp_release_tag="$(gh release list --repo WYJ288173/codex-custom --limit 30 --json tagName,isPrerelease --jq ".[] | select(.isPrerelease and (.tagName | endswith(\"-$mcp_release_short_sha\"))) | .tagName")"
test -n "$mcp_release_tag"
gh release view "$mcp_release_tag" --repo WYJ288173/codex-custom --json tagName,isPrerelease,targetCommitish,assets,url
git ls-remote origin refs/heads/feature/claude-style-statusline
```

Expected: prerelease is checksummed, includes both the `aarch64-apple-darwin.tar.gz` archive and `.sha256`, and targets the workflow-updated custom branch.

---

## Task 10: Install the Exact GitHub Artifact and Verify Coexistence

**Files:**

- Execute: `scripts/install-codex-custom.sh`
- Verify only: `~/.local/bin/codex-custom`
- Do not modify: the official `codex` executable

**Step 1: Record the current binaries before installation**

Run:

```bash
zsh -lic 'command -v codex; codex --version; command -v codex-custom; codex-custom --version'
```

Expected: capture both paths and versions for comparison.

**Step 2: Install by explicit release tag**

From the verified implementation worktree, run:

```bash
mcp_release_short_sha="$(git rev-parse --short=12 origin/feature/claude-style-statusline)"
mcp_release_tag="$(gh release list --repo WYJ288173/codex-custom --limit 30 --json tagName,isPrerelease --jq ".[] | select(.isPrerelease and (.tagName | endswith(\"-$mcp_release_short_sha\"))) | .tagName")"
test -n "$mcp_release_tag"
scripts/install-codex-custom.sh --repo WYJ288173/codex-custom --release "$mcp_release_tag"
```

The installer must download both assets, verify SHA256, and install only `~/.local/bin/codex-custom`.

**Step 3: Verify installed artifact and official Codex coexistence**

Run:

```bash
zsh -lic 'command -v codex-custom; codex-custom --version; command -v codex; codex --version'
shasum -a 256 ~/.local/bin/codex-custom
```

Expected:

- `codex-custom --version` matches the release's custom version;
- the custom binary path is `~/.local/bin/codex-custom`;
- official `codex` path/version are unchanged from Step 1;
- the installed binary is the unpacked GitHub artifact, not a local `target/` build.

**Step 4: Perform a user-level smoke check**

Launch `codex-custom`, run `/mcp`, and verify:

- server list is compact and navigable with Up/Down;
- Enter opens detail;
- tool names appear only under `View tools (N)`;
- applicable servers show Authenticate/Re-authenticate;
- Esc behavior matches the approved design;
- `/mcp verbose` still prints the diagnostic history output.

Do not force a real re-authentication of a production MCP solely for smoke testing; use an already-safe test server or stop after confirming action availability unless the user explicitly selects a server.

**Step 5: Final handoff evidence**

Report the implementation commit, successful GitHub run URL/ID, release tag, checksum verification, installed `codex-custom` path/version, unchanged official `codex` path/version, and any OAuth provider that was intentionally not exercised end-to-end.
