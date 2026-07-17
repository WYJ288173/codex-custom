use super::*;
use codex_app_server_protocol::McpAuthStatus;
use codex_app_server_protocol::McpServerStatus;
use codex_protocol::mcp::McpServerInfo;
use codex_protocol::mcp::Tool;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn tool(name: &str) -> Tool {
    Tool {
        name: name.to_string(),
        title: None,
        description: None,
        input_schema: serde_json::json!({"type": "object"}),
        output_schema: None,
        annotations: None,
        icons: None,
        meta: None,
    }
}

fn status(
    name: &str,
    auth_status: McpAuthStatus,
    connected: bool,
    tool_names: &[&str],
) -> McpServerStatus {
    McpServerStatus {
        name: name.to_string(),
        server_info: connected.then(|| McpServerInfo {
            name: name.to_string(),
            title: None,
            version: "1.0.0".to_string(),
            description: None,
            icons: None,
            website_url: None,
        }),
        tools: tool_names
            .iter()
            .map(|name| ((*name).to_string(), tool(name)))
            .collect(),
        resources: Vec::new(),
        resource_templates: Vec::new(),
        auth_status,
    }
}

fn mixed_statuses() -> Vec<McpServerStatus> {
    vec![
        status(
            "zeta-unavailable",
            McpAuthStatus::BearerToken,
            false,
            &["zeta_secret_tool_name"],
        ),
        status(
            "alpha-connected",
            McpAuthStatus::OAuth,
            true,
            &["secret_tool_name", "another_private_tool"],
        ),
        status(
            "middle-auth",
            McpAuthStatus::NotLoggedIn,
            true,
            &["auth_only_secret_tool"],
        ),
    ]
}

#[tokio::test]
async fn mcp_picker_loading_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_mcp_picker_loading();

    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(crate::chatwidget::mcp_picker::MCP_LIST_VIEW_ID)
    );
    assert_chatwidget_snapshot!(
        "mcp_picker_loading",
        render_bottom_popup(&chat, /*width*/ 80)
    );
}

#[tokio::test]
async fn mcp_picker_sorted_mixed_state_list_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.open_mcp_picker_loading();

    chat.on_mcp_picker_inventory_loaded(Ok(mixed_statuses()), /*focus_server*/ None);

    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    assert!(!rendered.contains("secret_tool_name"));
    assert!(!rendered.contains("another_private_tool"));
    assert!(!rendered.contains("auth_only_secret_tool"));
    assert_chatwidget_snapshot!("mcp_picker_sorted_mixed_state_list", rendered);
}

#[tokio::test]
async fn mcp_picker_empty_inventory_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.open_mcp_picker_loading();

    chat.on_mcp_picker_inventory_loaded(Ok(Vec::new()), /*focus_server*/ None);

    assert_chatwidget_snapshot!(
        "mcp_picker_empty_inventory",
        render_bottom_popup(&chat, /*width*/ 80)
    );
}

#[tokio::test]
async fn mcp_picker_inventory_error_offers_retry_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.open_mcp_picker_loading();

    chat.on_mcp_picker_inventory_loaded(
        Err("inventory service unavailable".to_string()),
        /*focus_server*/ None,
    );

    assert_chatwidget_snapshot!(
        "mcp_picker_inventory_error",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::FetchMcpPickerInventory {
            thread_id: Some(actual_thread_id),
            focus_server: None,
        }) if actual_thread_id == thread_id
    );
}

#[tokio::test]
async fn mcp_picker_narrow_width_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.open_mcp_picker_loading();
    chat.on_mcp_picker_inventory_loaded(Ok(mixed_statuses()), /*focus_server*/ None);

    let rendered = render_bottom_popup(&chat, /*width*/ 36);
    assert!(!rendered.contains("secret_tool_name"));
    assert!(!rendered.contains("another_private_tool"));
    assert!(!rendered.contains("auth_only_secret_tool"));
    assert_chatwidget_snapshot!("mcp_picker_narrow_width", rendered);
}

#[tokio::test]
async fn mcp_picker_ignores_inventory_loaded_after_list_is_closed() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.open_mcp_picker_loading();
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    chat.on_mcp_picker_inventory_loaded(Ok(mixed_statuses()), /*focus_server*/ None);

    assert!(chat.bottom_pane.no_modal_or_popup_active());
}
