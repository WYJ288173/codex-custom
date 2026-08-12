use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionRowDisplay;
use crate::bottom_pane::SelectionViewParams;
use codex_app_server_protocol::McpAuthStatus;
use codex_app_server_protocol::McpServerStatus;
use ratatui::style::Stylize;
use ratatui::text::Line;

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

fn tool_count_label(count: usize) -> String {
    if count == 1 {
        "1 tool".to_string()
    } else {
        format!("{count} tools")
    }
}

fn oauth_action(auth_status: McpAuthStatus) -> Option<&'static str> {
    match auth_status {
        McpAuthStatus::NotLoggedIn => Some("Authenticate"),
        McpAuthStatus::OAuth => Some("Re-authenticate"),
        McpAuthStatus::BearerToken | McpAuthStatus::Unknown | McpAuthStatus::Unsupported => None,
    }
}

fn loading_params() -> SelectionViewParams {
    SelectionViewParams {
        view_id: Some(MCP_LIST_VIEW_ID),
        title: Some("MCP servers".to_string()),
        items: vec![SelectionItem {
            name: "Loading MCP inventory…".to_string(),
            is_disabled: true,
            ..Default::default()
        }],
        footer_hint: Some(Line::from("Esc Close")),
        ..Default::default()
    }
}

fn list_params(mut statuses: Vec<McpServerStatus>) -> SelectionViewParams {
    statuses.sort_by(|left, right| left.name.cmp(&right.name));
    let items = statuses
        .into_iter()
        .map(|status| {
            let tool_count = tool_count_label(status.tools.len());
            let (state_prefix, description) = match connection_state(&status) {
                McpConnectionState::Connected => ("✓ ".green(), tool_count),
                McpConnectionState::NeedsAuthentication => {
                    ("! ".cyan(), format!("Needs authentication · {tool_count}"))
                }
                McpConnectionState::Unavailable => {
                    ("× ".red(), format!("Unavailable · {tool_count}"))
                }
            };
            let name = status.name.clone();
            SelectionItem {
                name,
                name_prefix_spans: vec![state_prefix],
                description: Some(description),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenMcpServerDetail {
                        server: status.clone(),
                    });
                })],
                ..Default::default()
            }
        })
        .collect();

    SelectionViewParams {
        view_id: Some(MCP_LIST_VIEW_ID),
        title: Some("MCP servers".to_string()),
        items,
        footer_hint: Some(Line::from("↑↓ Navigate  Enter Details  Esc Close")),
        col_width_mode: ColumnWidthMode::AutoAllRows,
        row_display: SelectionRowDisplay::SingleLine,
        ..Default::default()
    }
}

fn detail_params(server: McpServerStatus) -> SelectionViewParams {
    let tool_count = server.tools.len();
    let state_label = match connection_state(&server) {
        McpConnectionState::Connected => "Connected",
        McpConnectionState::NeedsAuthentication => "Needs authentication",
        McpConnectionState::Unavailable => "Unavailable",
    };
    let auth_label = match server.auth_status {
        McpAuthStatus::Unsupported => "Unsupported",
        McpAuthStatus::Unknown => "Unknown",
        McpAuthStatus::NotLoggedIn => "Not logged in",
        McpAuthStatus::BearerToken => "Bearer token",
        McpAuthStatus::OAuth => "OAuth",
    };
    let mut items = Vec::new();
    if tool_count > 0 {
        let tools_server = server.clone();
        items.push(SelectionItem {
            name: format!("View tools ({tool_count})"),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenMcpServerTools {
                    server: tools_server.clone(),
                });
            })],
            ..Default::default()
        });
    }
    if let Some(action) = oauth_action(server.auth_status) {
        let oauth_server = server.clone();
        items.push(SelectionItem {
            name: action.to_string(),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::StartMcpOauth {
                    server: oauth_server.clone(),
                });
            })],
            ..Default::default()
        });
    }
    let has_actions = !items.is_empty();

    SelectionViewParams {
        view_id: Some(MCP_DETAIL_VIEW_ID),
        title: Some(format!("MCP server · {}", server.name)),
        subtitle: Some(format!(
            "State: {} · Authentication: {} · {}",
            state_label,
            auth_label,
            tool_count_label(tool_count),
        )),
        items,
        footer_hint: Some(if has_actions {
            Line::from("↑↓ Navigate  Enter Select  Esc Back")
        } else {
            Line::from("Esc Back")
        }),
        col_width_mode: ColumnWidthMode::AutoAllRows,
        row_display: SelectionRowDisplay::SingleLine,
        ..Default::default()
    }
}

fn tools_params(server: McpServerStatus) -> SelectionViewParams {
    let mut tool_names = server.tools.into_keys().collect::<Vec<_>>();
    tool_names.sort();
    let tool_count = tool_names.len();
    let items = tool_names
        .into_iter()
        .map(|name| SelectionItem {
            name,
            is_disabled: true,
            ..Default::default()
        })
        .collect();

    SelectionViewParams {
        view_id: Some(MCP_TOOLS_VIEW_ID),
        title: Some(format!("MCP tools · {}", server.name)),
        subtitle: Some(tool_count_label(tool_count)),
        items,
        footer_hint: Some(Line::from("Esc Back")),
        col_width_mode: ColumnWidthMode::AutoAllRows,
        row_display: SelectionRowDisplay::SingleLine,
        ..Default::default()
    }
}

fn empty_params() -> SelectionViewParams {
    SelectionViewParams {
        view_id: Some(MCP_LIST_VIEW_ID),
        title: Some("MCP servers".to_string()),
        subtitle: Some("No MCP servers configured.".to_string()),
        items: vec![SelectionItem {
            name: "See the Codex documentation to configure MCP servers.".to_string(),
            is_disabled: true,
            ..Default::default()
        }],
        footer_hint: Some(Line::from("Esc Close")),
        ..Default::default()
    }
}

fn error_params(
    error: String,
    thread_id: Option<codex_protocol::ThreadId>,
    focus_server: Option<String>,
) -> SelectionViewParams {
    let error = sanitize_mcp_oauth_message(&error);
    SelectionViewParams {
        view_id: Some(MCP_LIST_VIEW_ID),
        title: Some("MCP servers".to_string()),
        subtitle: Some(format!("Failed to load MCP inventory: {error}")),
        items: vec![SelectionItem {
            name: "Retry".to_string(),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::FetchMcpPickerInventory {
                    thread_id,
                    focus_server: focus_server.clone(),
                });
            })],
            ..Default::default()
        }],
        footer_hint: Some(Line::from("Enter Retry  Esc Close")),
        ..Default::default()
    }
}

fn oauth_refresh_error_params(
    server: McpServerStatus,
    message: String,
    thread_id: Option<codex_protocol::ThreadId>,
) -> SelectionViewParams {
    let focus_server = server.name.clone();
    SelectionViewParams {
        view_id: Some(MCP_OAUTH_VIEW_ID),
        title: Some(format!("MCP reconnect failed · {}", server.name)),
        subtitle: Some(sanitize_mcp_oauth_message(&message)),
        items: vec![
            SelectionItem {
                name: "Retry".to_string(),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::FetchMcpPickerInventory {
                        thread_id,
                        focus_server: Some(focus_server.clone()),
                    });
                })],
                ..Default::default()
            },
            SelectionItem {
                name: "Close".to_string(),
                actions: vec![Box::new(|tx| tx.send(AppEvent::DismissMcpViews))],
                ..Default::default()
            },
        ],
        footer_hint: Some(Line::from("Enter Select  Esc Close")),
        on_cancel: Some(Box::new(|tx| tx.send(AppEvent::DismissMcpViews))),
        ..Default::default()
    }
}

fn oauth_starting_params(server: McpServerStatus) -> SelectionViewParams {
    SelectionViewParams {
        view_id: Some(MCP_OAUTH_VIEW_ID),
        title: Some(format!("Authenticating {}…", server.name)),
        subtitle: Some("Requesting a browser sign-in link…".to_string()),
        items: vec![SelectionItem {
            name: "Starting authentication…".to_string(),
            is_disabled: true,
            ..Default::default()
        }],
        footer_hint: Some(Line::from("Esc Close")),
        on_cancel: Some(Box::new(|tx| tx.send(AppEvent::DismissMcpViews))),
        ..Default::default()
    }
}

fn oauth_progress_params(
    server: McpServerStatus,
    operation_id: String,
    subtitle: String,
) -> SelectionViewParams {
    SelectionViewParams {
        view_id: Some(MCP_OAUTH_VIEW_ID),
        title: Some(format!("Authenticating {}…", server.name)),
        subtitle: Some(subtitle),
        items: vec![
            SelectionItem {
                name: "Open browser again".to_string(),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenPendingMcpOauthUrl {
                        operation_id: operation_id.clone(),
                    });
                })],
                ..Default::default()
            },
            SelectionItem {
                name: "Close".to_string(),
                actions: vec![Box::new(|tx| tx.send(AppEvent::DismissMcpViews))],
                ..Default::default()
            },
        ],
        footer_hint: Some(Line::from("Enter Select  Esc Close")),
        on_cancel: Some(Box::new(|tx| tx.send(AppEvent::DismissMcpViews))),
        ..Default::default()
    }
}

fn oauth_busy_params(server: McpServerStatus) -> SelectionViewParams {
    SelectionViewParams {
        view_id: Some(MCP_OAUTH_VIEW_ID),
        title: Some(format!("Already authenticating {}…", server.name)),
        subtitle: Some("Complete the current sign-in before starting another.".to_string()),
        items: vec![SelectionItem {
            name: "Close".to_string(),
            actions: vec![Box::new(|tx| tx.send(AppEvent::DismissMcpViews))],
            ..Default::default()
        }],
        footer_hint: Some(Line::from("Enter Close  Esc Close")),
        on_cancel: Some(Box::new(|tx| tx.send(AppEvent::DismissMcpViews))),
        ..Default::default()
    }
}

fn oauth_error_params(server: McpServerStatus, error: String) -> SelectionViewParams {
    let retry_server = server.clone();
    SelectionViewParams {
        view_id: Some(MCP_OAUTH_VIEW_ID),
        title: Some(format!("Authentication failed · {}", server.name)),
        subtitle: Some(sanitize_mcp_oauth_message(&error)),
        items: vec![
            SelectionItem {
                name: "Retry".to_string(),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::StartMcpOauth {
                        server: retry_server.clone(),
                    });
                })],
                ..Default::default()
            },
            SelectionItem {
                name: "Close".to_string(),
                actions: vec![Box::new(|tx| tx.send(AppEvent::DismissMcpViews))],
                ..Default::default()
            },
        ],
        footer_hint: Some(Line::from("Enter Select  Esc Close")),
        on_cancel: Some(Box::new(|tx| tx.send(AppEvent::DismissMcpViews))),
        ..Default::default()
    }
}

fn redact_urls(error: &str) -> String {
    let mut redacted = String::new();
    let mut remaining = error;
    loop {
        let lowercase = remaining.to_ascii_lowercase();
        let http = lowercase.find("http://");
        let https = lowercase.find("https://");
        let start = match (http, https) {
            (Some(http), Some(https)) => Some(http.min(https)),
            (Some(http), None) => Some(http),
            (None, Some(https)) => Some(https),
            (None, None) => None,
        };
        let Some(start) = start else {
            redacted.push_str(remaining);
            return redacted;
        };
        redacted.push_str(&remaining[..start]);
        redacted.push_str("[REDACTED]");
        let url = &remaining[start..];
        let end = url.find(char::is_whitespace).unwrap_or(url.len());
        remaining = &url[end..];
    }
}

pub(crate) fn sanitize_mcp_oauth_message(message: &str) -> String {
    let sanitized = redact_urls(message);
    let lowercase = sanitized.to_ascii_lowercase();
    if [
        "access_token",
        "refresh_token",
        "authorization",
        "credential",
        "environment",
        "token",
        "state",
        "headers",
        "header",
        "env",
    ]
    .iter()
    .any(|marker| lowercase.contains(marker))
    {
        return "[REDACTED]".to_string();
    }
    sanitized
}

impl ChatWidget {
    pub(crate) fn open_mcp_picker_loading(&mut self) {
        let params = loading_params();
        if self.bottom_pane.has_view_id(MCP_LIST_VIEW_ID) {
            let _ = self
                .bottom_pane
                .replace_selection_view_if_present(MCP_LIST_VIEW_ID, params);
        } else {
            self.bottom_pane.show_selection_view(params);
        }
    }

    pub(crate) fn on_mcp_picker_inventory_loaded(
        &mut self,
        result: Result<Vec<McpServerStatus>, String>,
        focus_server: Option<String>,
    ) {
        if !self.bottom_pane.has_view_id(MCP_LIST_VIEW_ID) {
            return;
        }

        let params = match (result, focus_server) {
            (Ok(statuses), None) if statuses.is_empty() => empty_params(),
            (Ok(statuses), Some(focus_server)) => {
                match statuses
                    .iter()
                    .find(|status| status.name == focus_server)
                    .cloned()
                {
                    Some(server) => detail_params(server),
                    None => oauth_refresh_error_params(
                        McpServerStatus {
                            name: focus_server,
                            plugin_id: None,
                            server_info: None,
                            tools: Default::default(),
                            resources: Vec::new(),
                            resource_templates: Vec::new(),
                            auth_status: McpAuthStatus::Unsupported,
                        },
                        "Authentication succeeded, but reconnection failed. Restart Codex if needed; the server was absent from the refreshed MCP inventory.".to_string(),
                        self.thread_id(),
                    ),
                }
            }
            (Ok(statuses), None) => list_params(statuses),
            (Err(error), Some(focus_server)) => oauth_refresh_error_params(
                McpServerStatus {
                    name: focus_server,
                    plugin_id: None,
                    server_info: None,
                    tools: Default::default(),
                    resources: Vec::new(),
                    resource_templates: Vec::new(),
                    auth_status: McpAuthStatus::Unsupported,
                },
                format!(
                    "Authentication succeeded, but reconnection failed. Restart Codex if needed. Details: {}",
                    sanitize_mcp_oauth_message(&error)
                ),
                self.thread_id(),
            ),
            (Err(error), None) => error_params(error, self.thread_id(), /*focus_server*/ None),
        };
        let _ = self
            .bottom_pane
            .replace_selection_view_if_present(MCP_LIST_VIEW_ID, params);
    }

    pub(crate) fn open_mcp_server_detail(&mut self, server: McpServerStatus) {
        self.bottom_pane.show_selection_view(detail_params(server));
    }

    pub(crate) fn open_mcp_server_tools(&mut self, server: McpServerStatus) {
        self.bottom_pane.show_selection_view(tools_params(server));
    }

    pub(crate) fn open_mcp_oauth_starting(&mut self, server: McpServerStatus) {
        let params = oauth_starting_params(server.clone());
        if self
            .bottom_pane
            .replace_selection_view_if_present(MCP_OAUTH_VIEW_ID, params)
        {
            return;
        }
        let _ = self
            .bottom_pane
            .replace_selection_view_if_present(MCP_DETAIL_VIEW_ID, oauth_starting_params(server));
    }

    pub(crate) fn show_mcp_oauth_progress(
        &mut self,
        server: McpServerStatus,
        operation_id: String,
    ) {
        let _ = self.bottom_pane.replace_selection_view_if_present(
            MCP_OAUTH_VIEW_ID,
            oauth_progress_params(
                server,
                operation_id,
                "Complete sign-in in your browser, then return here.".to_string(),
            ),
        );
    }

    pub(crate) fn mcp_oauth_browser_error_message(error: &str) -> String {
        format!(
            "Failed to open browser: {}",
            sanitize_mcp_oauth_message(error)
        )
    }

    pub(crate) fn show_mcp_oauth_browser_error(
        &mut self,
        server: McpServerStatus,
        operation_id: String,
        message: String,
    ) -> bool {
        self.bottom_pane.replace_selection_view_if_present(
            MCP_OAUTH_VIEW_ID,
            oauth_progress_params(server, operation_id, sanitize_mcp_oauth_message(&message)),
        )
    }

    pub(crate) fn show_mcp_oauth_busy(&mut self, server: McpServerStatus) {
        let params = oauth_busy_params(server.clone());
        if self
            .bottom_pane
            .replace_selection_view_if_present(MCP_OAUTH_VIEW_ID, params)
        {
            return;
        }
        let _ = self
            .bottom_pane
            .replace_selection_view_if_present(MCP_DETAIL_VIEW_ID, oauth_busy_params(server));
    }

    pub(crate) fn show_mcp_oauth_error(&mut self, server: McpServerStatus, error: String) -> bool {
        let params = oauth_error_params(server.clone(), error.clone());
        if self
            .bottom_pane
            .replace_selection_view_if_present(MCP_OAUTH_VIEW_ID, params)
        {
            return true;
        }
        self.bottom_pane
            .replace_selection_view_if_present(MCP_LIST_VIEW_ID, oauth_error_params(server, error))
    }

    pub(crate) fn show_mcp_oauth_refresh_error(
        &mut self,
        server: McpServerStatus,
        message: String,
    ) -> bool {
        let params = oauth_refresh_error_params(server.clone(), message.clone(), self.thread_id());
        if self
            .bottom_pane
            .replace_selection_view_if_present(MCP_OAUTH_VIEW_ID, params)
        {
            return true;
        }
        self.bottom_pane.replace_selection_view_if_present(
            MCP_LIST_VIEW_ID,
            oauth_refresh_error_params(server, message, self.thread_id()),
        )
    }

    pub(crate) fn finish_mcp_oauth_refresh(
        &mut self,
        statuses: Vec<McpServerStatus>,
        fresh_server: McpServerStatus,
    ) -> bool {
        let has_oauth = self.bottom_pane.has_view_id(MCP_OAUTH_VIEW_ID);
        let has_list = self.bottom_pane.has_view_id(MCP_LIST_VIEW_ID);
        if !has_oauth && !has_list {
            return false;
        }

        if has_list {
            let _ = self
                .bottom_pane
                .replace_selection_view_if_present(MCP_LIST_VIEW_ID, list_params(statuses));
        }
        if has_oauth {
            let _ = self
                .bottom_pane
                .replace_selection_view_if_present(MCP_OAUTH_VIEW_ID, detail_params(fresh_server));
        } else {
            let _ = self
                .bottom_pane
                .replace_selection_view_if_present(MCP_LIST_VIEW_ID, detail_params(fresh_server));
        }
        true
    }

    pub(crate) fn open_mcp_picker_refresh_loading(&mut self) {
        while self.bottom_pane.dismiss_view_by_id(MCP_OAUTH_VIEW_ID) {}
        self.open_mcp_picker_loading();
    }

    pub(crate) fn dismiss_mcp_views(&mut self) {
        for view_id in [
            MCP_OAUTH_VIEW_ID,
            MCP_TOOLS_VIEW_ID,
            MCP_DETAIL_VIEW_ID,
            MCP_LIST_VIEW_ID,
        ] {
            while self.bottom_pane.dismiss_view_by_id(view_id) {}
        }
    }
}
