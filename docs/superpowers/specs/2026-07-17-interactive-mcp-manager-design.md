# Interactive MCP Manager Design

**Date:** 2026-07-17

**Status:** Approved for implementation planning

**Target:** `codex-custom` TUI on top of `feature/claude-style-statusline`

## Context

The current `/mcp` command renders every configured server and all of its tool names into chat history. With several MCP servers, this creates a long, hard-to-scan transcript. OAuth authentication is only practical through the separate `codex mcp login <server>` command even though the app-server already exposes OAuth login and completion APIs.

Claude Code uses a layered MCP manager: the first screen lists servers, status, and tool counts; Enter opens one server; authentication is an action in that server's detail view. This design adopts that information hierarchy without copying Claude Code's unrelated project-level `.mcp.json` trust system.

Reference: <https://code.claude.com/docs/en/mcp>

## Goals

1. Make bare `/mcp` open a keyboard-driven server picker.
2. Keep the first screen compact: server name, state, authentication state when relevant, and tool count only.
3. Let users select a server with Up/Down and Enter.
4. Support `Authenticate` for a server that is not logged in and `Re-authenticate` for an OAuth-authenticated server.
5. Open the OAuth authorization URL in the browser, track completion, refresh MCP connections, and update the open panel.
6. Put tool names behind an explicit `View tools (N)` action for one server.
7. Preserve `/mcp verbose` as the existing transcript-oriented diagnostic output.
8. Build and test on GitHub before installing the release artifact locally.

## Non-goals

- Do not add project-level MCP configuration trust or approval.
- Do not add `Clear authentication`, logout, or credential deletion.
- Do not change OAuth token storage, refresh semantics, or provider discovery.
- Do not change MCP tool-call permissions.
- Do not redesign `codex mcp list/add/remove/login` CLI commands.
- Do not expand the app-server v2 protocol solely to show richer connection errors.
- Do not change the official `codex` binary or replace it with this fork.

## User Experience

### Level 1: server list

Bare `/mcp` opens a modal selection view:

```text
MCP servers

› ✓ flyeye-log-query       4 tools
  ! sentry                 Needs authentication
  ✓ openaiDeveloperDocs    3 tools
  × unavailable-server     Unavailable

↑↓ Navigate  Enter Details  Esc Close
```

The list never renders tool names, resource names, resource-template names, commands, URLs, headers, or environment values. Server rows are sorted by name, matching the current inventory behavior.

The initial state is derived without changing the app-server protocol:

- `auth_status == NotLoggedIn` → `Needs authentication`.
- `server_info.is_some()` → `Connected`.
- otherwise → `Unavailable`.

The UI does not claim to know an exact startup failure reason when the current status RPC does not provide one.

If no servers are configured, the view shows a compact empty state and an MCP documentation hint. If inventory loading fails, it shows the error and a `Retry` action.

### Level 2: server detail

Enter opens a second selection view for the highlighted server. It shows:

- server name;
- derived connection state;
- authentication state;
- available tool count;
- `View tools (N)` when tools exist;
- the applicable OAuth action;
- `Back` through Esc and the normal selection-view stack.

OAuth actions are determined only from `McpAuthStatus`:

| Auth status | Action |
| --- | --- |
| `NotLoggedIn` | `Authenticate` |
| `OAuth` | `Re-authenticate` |
| `BearerToken` | No OAuth action |
| `Unsupported` | No OAuth action |

Re-authentication does not clear existing credentials before starting. A failed re-authentication therefore does not intentionally delete the last stored credentials.

### Level 3: tool list

`View tools (N)` opens a third, scrollable selection view containing only the selected server's sorted tool names. The list is informational; Enter does not call tools. Esc returns to the server detail.

### OAuth progress

Selecting `Authenticate` or `Re-authenticate` replaces the detail view with an in-place progress view:

```text
Authenticating sentry…

Complete sign-in in your browser, then return here.
› Open browser again
  Close
```

The authorization URL remains available to the progress view for browser-open retry, but must not be persisted to transcript history or logs. This avoids retaining OAuth state-bearing URLs in rollout history.

The progress view's cancel callback dismisses the MCP view stack, so pressing Esc closes the whole MCP panel rather than returning to a detail page that could start a second login. It does not cancel the app-server OAuth listener. When the flow completes after the panel is closed, Codex adds one short success or failure history message and does not reopen a modal unexpectedly.

## Command Compatibility

- `/mcp` opens the interactive manager.
- `/mcp verbose` preserves the current `McpServerStatusDetail::Full` history output, including tools, resources, and resource templates.
- Other `/mcp` arguments continue to produce the usage error.

This preserves a verbose diagnostic path while changing the default from transcript output to interaction.

## Architecture

### TUI module boundary

Add a focused `codex-rs/tui/src/chatwidget/mcp_picker.rs` module. It owns:

- view IDs and view-state builders;
- compact server-row formatting;
- server-detail action construction;
- tool-list construction;
- loading, OAuth progress, and error views;
- replacement rules for asynchronous results.

`chatwidget.rs` remains orchestration-only. Existing generic `ListSelectionView`, the bottom-pane view stack, and selection-view replacement helpers provide Up/Down, Enter, Esc, scrolling, and nested navigation.

### Inventory flow

1. `SlashCommand::Mcp` sends an event requesting an interactive MCP inventory.
2. `App` uses the existing paginated `mcpServerStatus/list` request with `ToolsAndAuthOnly`.
3. While the request is pending, a modal loading view is active.
4. The result replaces the loading view only if that view is still present. A late result must not reopen a picker the user already dismissed.
5. The picker retains the returned `McpServerStatus` values, so the third-level tool list requires no additional request.

The existing `/mcp verbose` path continues through the transcript inventory renderer.

### OAuth flow

1. A server-detail action sends a TUI app event with the server name and whether the label is `Authenticate` or `Re-authenticate`.
2. `App` records one pending MCP OAuth operation. Additional login actions are disabled or rejected while it is pending.
3. A background request calls the existing `mcpServer/oauth/login` RPC with the server name.
4. The response returns `authorization_url`.
5. The TUI opens the URL using the existing browser-opening implementation, but the MCP-specific path does not write the full URL to history.
6. The detail view is replaced with the OAuth progress view, which retains the URL in memory for `Open browser again`.
7. `App` handles the existing global `McpServerOauthLoginCompleted` notification instead of forwarding it to the current no-op ChatWidget branch.
8. Only a completion notification matching the pending server is accepted; stale or mismatched notifications are ignored with tracing.
9. On success, the TUI calls the existing `mcpServer/refresh` RPC, then fetches a fresh inventory and returns to the selected server's detail view.
10. On failure or timeout, the progress view becomes an error view with `Retry` and `Close`.

No core MCP connection manager, OAuth store, app-server OAuth implementation, or wire type needs to change.

### State ownership

`App` owns the pending asynchronous OAuth operation because it owns app-server requests and global notifications. The picker module owns presentational state. The pending record contains only the server name and authorization URL needed by the active flow; it contains no token or credential.

The UI supports at most one pending MCP OAuth operation. This matches the single active selection flow and avoids ambiguous completion notifications, whose protocol payload identifies the server but has no client-generated operation ID.

## Error Handling

| Scenario | Behavior |
| --- | --- |
| Inventory request fails | Replace loading with error plus `Retry`; do not leave a spinner or blank view. |
| OAuth RPC fails before returning a URL | Stay in the server flow, show the error, and offer `Retry`. |
| Browser launch fails | Keep the progress view and authorization URL; offer `Open browser again`. |
| OAuth succeeds | Refresh MCP servers, reload inventory, and reopen the selected server detail. |
| Refresh fails after OAuth succeeds | Report that authentication succeeded but refresh failed; offer inventory retry and mention that a restart may be needed. |
| OAuth fails or times out | Show the app-server error with `Retry` and `Close`. |
| User closes the progress view | Let the background flow finish; publish a non-blocking result message instead of reopening the panel. |
| Completion is stale or for another server | Ignore it and preserve the current pending flow. |
| Thread changes while inventory is loading | Reuse the existing thread identity guard and discard the stale result. |

Errors are rendered as user-facing text without OAuth URLs, tokens, headers, or environment secrets.

## Testing

All user-visible views require `insta` snapshot coverage.

### View and formatting tests

- compact list renders server names, states, and counts but no tool names;
- empty and inventory-error views;
- unauthenticated detail with `Authenticate`;
- OAuth-authenticated detail with `Re-authenticate` and no clear/logout action;
- bearer-token and unsupported details with no OAuth action;
- sorted third-level tool list;
- OAuth progress, success-refresh failure, and OAuth failure views;
- narrow-terminal wrapping and footer hints.

### Interaction and app tests

- bare `/mcp` requests `ToolsAndAuthOnly` and opens the picker;
- `/mcp verbose` retains the current full history path;
- Up/Down, Enter, Esc, and view-stack back navigation;
- dismissed loading view is not reopened by a late inventory result;
- OAuth action sends the typed login request for the selected server;
- returned URL opens through the MCP-specific non-persisting browser path;
- matching completion succeeds, refreshes, reloads, and returns to detail;
- OAuth failure exposes retry;
- completion after the view is closed produces a history message only;
- stale or mismatched completion notification is ignored;
- a second OAuth action cannot replace an active pending operation.

### Validation commands

Follow repository instructions and use scoped commands:

```text
just test -p codex-tui
just fix -p codex-tui
just fmt
```

The existing custom GitHub workflow additionally runs focused `codex-tui` and `codex-cli` tests before the release build. No app-server schema regeneration is required because the v2 protocol remains unchanged.

## Branch, Remote Build, and Installation

Implementation is based on the fork's `feature/claude-style-statusline` line, rebased onto current `upstream/main`, and developed in an isolated worktree so existing local branches and changes remain untouched.

Release sequence:

1. Complete local focused tests and snapshots.
2. Push the updated custom feature branch.
3. Trigger the existing `codex-custom-sync` GitHub Actions workflow with release publishing enabled.
4. Let GitHub rebase onto the latest upstream, run formatting and focused tests, build the Apple Silicon release binary, upload artifacts, and publish a prerelease with SHA256.
5. If any remote job fails, stop; do not update the local executable.
6. After the workflow is fully green, install by explicit release tag using `scripts/install-codex-custom.sh`.
7. Verify the downloaded archive checksum, `codex-custom --version`, the release commit/version, and coexistence with the official `codex` binary.

The GitHub-built artifact is the only binary installed locally for this change.

## Acceptance Criteria

1. `/mcp` opens a navigable server list instead of appending every tool to history.
2. No tool name appears on the first screen.
3. Enter opens the selected server's detail view.
4. Tool names are available only through `View tools (N)`.
5. `NotLoggedIn` servers offer `Authenticate`.
6. OAuth-authenticated servers offer `Re-authenticate` and no credential-clear action.
7. OAuth can complete from the interactive TUI without requiring `codex mcp login`.
8. Successful OAuth refreshes MCP state and updates the detail view.
9. Closing the OAuth panel never causes a surprise modal reopen.
10. `/mcp verbose` retains the full diagnostic output.
11. Relevant local tests and snapshots pass.
12. The GitHub workflow passes and publishes a checksummed artifact before local installation.
13. The installed `codex-custom` matches the published release; official Codex remains unchanged.
