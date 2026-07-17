use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionRowDisplay;
use crate::bottom_pane::SelectionViewParams;
use codex_app_server_protocol::McpAuthStatus;
use codex_app_server_protocol::McpServerStatus;
use ratatui::style::Color;
use ratatui::style::Stylize;
use ratatui::text::Line;

pub(super) const MCP_LIST_VIEW_ID: &str = "mcp-list";
#[allow(dead_code)]
pub(super) const MCP_DETAIL_VIEW_ID: &str = "mcp-detail";
#[allow(dead_code)]
pub(super) const MCP_TOOLS_VIEW_ID: &str = "mcp-tools";
#[allow(dead_code)]
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
                McpConnectionState::NeedsAuthentication => (
                    "! ".fg(Color::Yellow),
                    format!("Needs authentication · {tool_count}"),
                ),
                McpConnectionState::Unavailable => {
                    ("× ".red(), format!("Unavailable · {tool_count}"))
                }
            };
            SelectionItem {
                name: status.name,
                name_prefix_spans: vec![state_prefix],
                description: Some(description),
                ..Default::default()
            }
        })
        .collect();

    SelectionViewParams {
        view_id: Some(MCP_LIST_VIEW_ID),
        title: Some("MCP servers".to_string()),
        items,
        footer_hint: Some(Line::from("↑↓ Navigate  Esc Close")),
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

        let params = match result {
            Ok(statuses) if statuses.is_empty() => empty_params(),
            Ok(statuses) => list_params(statuses),
            Err(error) => error_params(error, self.thread_id(), focus_server),
        };
        let _ = self
            .bottom_pane
            .replace_selection_view_if_present(MCP_LIST_VIEW_ID, params);
    }
}
