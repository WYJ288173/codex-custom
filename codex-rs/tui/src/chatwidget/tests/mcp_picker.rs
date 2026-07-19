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

fn press(chat: &mut ChatWidget, code: KeyCode) {
    chat.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
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

#[tokio::test]
async fn mcp_picker_down_and_enter_opens_the_second_sorted_server() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let expected = status(
        "middle-auth",
        McpAuthStatus::NotLoggedIn,
        true,
        &["auth_only_secret_tool"],
    );
    chat.open_mcp_picker_loading();
    chat.on_mcp_picker_inventory_loaded(Ok(mixed_statuses()), /*focus_server*/ None);

    press(&mut chat, KeyCode::Down);
    press(&mut chat, KeyCode::Enter);

    match rx.try_recv() {
        Ok(AppEvent::OpenMcpServerDetail { server }) => assert_eq!(server, expected),
        other => panic!("expected OpenMcpServerDetail event, got {other:?}"),
    }
}

#[tokio::test]
async fn mcp_picker_not_logged_in_server_detail_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status(
        "middle-auth",
        McpAuthStatus::NotLoggedIn,
        true,
        &["bravo_tool", "alpha_tool"],
    );
    chat.open_mcp_picker_loading();

    chat.open_mcp_server_detail(server);

    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(crate::chatwidget::mcp_picker::MCP_DETAIL_VIEW_ID)
    );
    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    assert!(rendered.contains("State: Needs authentication"));
    assert!(rendered.contains("Authentication: Not logged in"));
    assert!(rendered.contains("2 tools"));
    assert!(rendered.contains("View tools (2)"));
    assert!(rendered.contains("Authenticate"));
    assert_chatwidget_snapshot!("mcp_picker_not_logged_in_server_detail", rendered);
}

#[tokio::test]
async fn mcp_picker_oauth_detail_reauthenticate_starts_oauth() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status("oauth-server", McpAuthStatus::OAuth, true, &[]);
    chat.open_mcp_server_detail(server.clone());

    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    let rendered_lower = rendered.to_lowercase();
    assert!(rendered.contains("Re-authenticate"));
    assert!(rendered.contains("Enter Select"));
    assert!(!rendered_lower.contains("clear authentication"));
    assert!(!rendered_lower.contains("logout"));
    assert_chatwidget_snapshot!("mcp_picker_oauth_zero_tool_detail", rendered);

    press(&mut chat, KeyCode::Enter);

    match rx.try_recv() {
        Ok(AppEvent::StartMcpOauth { server: actual }) => assert_eq!(actual, server),
        other => panic!("expected StartMcpOauth event, got {other:?}"),
    }
}

#[tokio::test]
async fn mcp_picker_non_oauth_auth_statuses_have_no_oauth_action() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    for (auth_status, auth_label) in [
        (McpAuthStatus::BearerToken, "Bearer token"),
        (McpAuthStatus::Unsupported, "Unsupported"),
    ] {
        chat.open_mcp_server_detail(status("server", auth_status, true, &[]));

        let rendered = render_bottom_popup(&chat, /*width*/ 80);
        assert!(rendered.contains(&format!("Authentication: {auth_label}")));
        assert!(!rendered.contains("Authenticate"));
        assert!(!rendered.contains("Re-authenticate"));
    }
}

#[tokio::test]
async fn mcp_picker_view_tools_opens_sorted_informational_list_and_esc_navigates_back() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status(
        "alpha-connected",
        McpAuthStatus::OAuth,
        true,
        &["zulu_tool", "alpha_tool", "middle_tool"],
    );
    chat.open_mcp_picker_loading();
    chat.on_mcp_picker_inventory_loaded(Ok(vec![server.clone()]), /*focus_server*/ None);
    chat.open_mcp_server_detail(server.clone());

    press(&mut chat, KeyCode::Enter);
    let server = match rx.try_recv() {
        Ok(AppEvent::OpenMcpServerTools { server: actual }) => {
            assert_eq!(actual, server);
            actual
        }
        other => panic!("expected OpenMcpServerTools event, got {other:?}"),
    };
    chat.open_mcp_server_tools(server);

    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(crate::chatwidget::mcp_picker::MCP_TOOLS_VIEW_ID)
    );
    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    let alpha = rendered
        .find("alpha_tool")
        .expect("alpha tool should render");
    let middle = rendered
        .find("middle_tool")
        .expect("middle tool should render");
    let zulu = rendered.find("zulu_tool").expect("zulu tool should render");
    assert!(alpha < middle && middle < zulu);
    assert_chatwidget_snapshot!("mcp_picker_sorted_tool_list", rendered);

    press(&mut chat, KeyCode::Enter);
    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(crate::chatwidget::mcp_picker::MCP_TOOLS_VIEW_ID)
    );
    assert!(rx.try_recv().is_err());

    press(&mut chat, KeyCode::Esc);
    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(crate::chatwidget::mcp_picker::MCP_DETAIL_VIEW_ID)
    );
    press(&mut chat, KeyCode::Esc);
    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(crate::chatwidget::mcp_picker::MCP_LIST_VIEW_ID)
    );
}

#[tokio::test]
async fn mcp_oauth_starting_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status("sentry", McpAuthStatus::NotLoggedIn, false, &[]);
    chat.open_mcp_server_detail(server.clone());

    chat.open_mcp_oauth_starting(server);

    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(crate::chatwidget::mcp_picker::MCP_OAUTH_VIEW_ID)
    );
    assert_chatwidget_snapshot!(
        "mcp_oauth_starting",
        render_bottom_popup(&chat, /*width*/ 80)
    );
}

#[tokio::test]
async fn mcp_oauth_progress_snapshot_has_enabled_browser_retry_and_close() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status("sentry", McpAuthStatus::NotLoggedIn, false, &[]);
    chat.open_mcp_server_detail(server.clone());
    chat.open_mcp_oauth_starting(server.clone());

    chat.show_mcp_oauth_progress(server, "mcp-oauth-operation".to_string());

    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    assert!(rendered.contains("Open browser again"));
    assert!(rendered.contains("Close"));
    assert_chatwidget_snapshot!("mcp_oauth_progress", rendered);

    press(&mut chat, KeyCode::Enter);
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::OpenPendingMcpOauthUrl { operation_id })
            if operation_id == "mcp-oauth-operation"
    );
}

#[tokio::test]
async fn mcp_oauth_url_browser_failure_snapshot_keeps_retry_enabled() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status("sentry", McpAuthStatus::NotLoggedIn, false, &[]);
    chat.open_mcp_server_detail(server.clone());
    chat.open_mcp_oauth_starting(server.clone());
    chat.show_mcp_oauth_browser_error(
        server,
        "mcp-oauth-operation".to_string(),
        "Failed to open browser: simulated opener failure".to_string(),
    );

    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    assert!(rendered.contains("Failed to open browser: simulated opener failure"));
    assert!(rendered.contains("Open browser again"));
    assert_chatwidget_snapshot!("mcp_oauth_browser_error", rendered);

    press(&mut chat, KeyCode::Enter);
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::OpenPendingMcpOauthUrl { operation_id })
            if operation_id == "mcp-oauth-operation"
    );
}

#[tokio::test]
async fn mcp_oauth_error_snapshot_redacts_urls() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status("sentry", McpAuthStatus::NotLoggedIn, false, &[]);
    let uppercase_url = "HTTPS://auth.example.test/authorize?state=UPPER-SECRET";
    let mixed_case_url = "hTtPs://auth.example.test/authorize?state=MIXED-SECRET";
    let second_url = "http://auth.example.test/authorize?state=SECOND-SECRET";
    chat.open_mcp_server_detail(server.clone());
    chat.open_mcp_oauth_starting(server.clone());

    chat.show_mcp_oauth_error(
        server,
        format!("request failed at {uppercase_url} then {mixed_case_url} and {second_url}"),
    );

    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    assert!(!rendered.contains(uppercase_url));
    assert!(!rendered.contains(mixed_case_url));
    assert!(!rendered.contains(second_url));
    assert!(!rendered.contains("UPPER-SECRET"));
    assert!(!rendered.contains("MIXED-SECRET"));
    assert!(!rendered.contains("SECOND-SECRET"));
    assert_eq!(rendered.matches("[REDACTED]").count(), 3);
    assert_chatwidget_snapshot!("mcp_oauth_error", rendered);

    press(&mut chat, KeyCode::Enter);
    let retry_server = match rx.try_recv() {
        Ok(AppEvent::StartMcpOauth { server: actual }) => actual,
        other => panic!("expected StartMcpOauth event, got {other:?}"),
    };
    chat.open_mcp_oauth_starting(retry_server);
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("Requesting a browser sign-in link"));
}

#[tokio::test]
async fn esc_from_mcp_oauth_views_dismisses_the_whole_mcp_stack() {
    for stage in ["starting", "progress", "error"] {
        let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        let server = status("sentry", McpAuthStatus::NotLoggedIn, false, &[]);
        chat.open_mcp_picker_loading();
        chat.on_mcp_picker_inventory_loaded(Ok(vec![server.clone()]), /*focus_server*/ None);
        chat.open_mcp_server_detail(server.clone());
        chat.open_mcp_oauth_starting(server.clone());
        match stage {
            "starting" => {}
            "progress" => chat.show_mcp_oauth_progress(server, "mcp-oauth-operation".to_string()),
            "error" => {
                let _ = chat.show_mcp_oauth_error(server, "request failed".to_string());
            }
            other => panic!("unexpected OAuth view stage {other}"),
        }

        press(&mut chat, KeyCode::Esc);

        assert_matches!(rx.try_recv(), Ok(AppEvent::DismissMcpViews));
        chat.dismiss_mcp_views();
        assert!(chat.bottom_pane.no_modal_or_popup_active());
    }
}

#[tokio::test]
async fn mcp_oauth_busy_feedback_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status("sentry", McpAuthStatus::NotLoggedIn, false, &[]);
    chat.open_mcp_server_detail(server.clone());

    chat.show_mcp_oauth_busy(server);

    assert_chatwidget_snapshot!("mcp_oauth_busy", render_bottom_popup(&chat, /*width*/ 80));
}

#[tokio::test]
async fn dismiss_mcp_views_preserves_unrelated_overlay() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status("sentry", McpAuthStatus::NotLoggedIn, false, &[]);
    chat.show_selection_view(crate::bottom_pane::SelectionViewParams {
        view_id: Some("unrelated-test-overlay"),
        title: Some("Unrelated overlay".to_string()),
        ..Default::default()
    });
    chat.open_mcp_picker_loading();
    chat.open_mcp_server_detail(server.clone());
    chat.open_mcp_oauth_starting(server);

    chat.dismiss_mcp_views();

    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some("unrelated-test-overlay")
    );
}

#[tokio::test]
async fn mcp_oauth_refresh_error_retry_fetches_inventory_without_restarting_oauth() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    let server = status("sentry", McpAuthStatus::OAuth, false, &[]);
    chat.open_mcp_picker_loading();
    chat.open_mcp_server_detail(server.clone());
    chat.open_mcp_oauth_starting(server.clone());

    assert!(chat.show_mcp_oauth_refresh_error(
        server,
        "Authentication succeeded, but reconnecting failed. Restart may be needed.".to_string(),
    ));
    press(&mut chat, KeyCode::Enter);

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::FetchMcpPickerInventory {
            thread_id: Some(actual_thread_id),
            focus_server: Some(server),
        }) if actual_thread_id == thread_id && server == "sentry"
    );
    assert!(rx.try_recv().is_err(), "retry must not emit StartMcpOauth");
}

#[tokio::test]
async fn mcp_oauth_refresh_retry_restores_exact_fresh_server_detail() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut fresh_server = status("sentry", McpAuthStatus::OAuth, true, &[]);
    fresh_server
        .tools
        .insert("fresh_tool".to_string(), tool("fresh_tool"));
    chat.open_mcp_picker_loading();

    chat.on_mcp_picker_inventory_loaded(
        Ok(vec![
            status("Sentry", McpAuthStatus::NotLoggedIn, false, &[]),
            fresh_server,
        ]),
        Some("sentry".to_string()),
    );

    assert_eq!(
        chat.bottom_pane.active_view_id(),
        Some(crate::chatwidget::mcp_picker::MCP_DETAIL_VIEW_ID)
    );
    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    assert!(rendered.contains("MCP server · sentry"));
    assert!(rendered.contains("Authentication: OAuth"));
    assert!(rendered.contains("View tools (1)"));
}

#[tokio::test]
async fn mcp_oauth_refresh_error_snapshot_contains_retry_close_and_restart_guidance() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status("sentry", McpAuthStatus::OAuth, false, &[]);
    chat.open_mcp_picker_loading();
    chat.open_mcp_server_detail(server.clone());
    chat.open_mcp_oauth_starting(server.clone());

    assert!(chat.show_mcp_oauth_refresh_error(
        server,
        "Authentication succeeded, but reconnecting failed. Restart may be needed.".to_string(),
    ));

    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    assert!(rendered.contains("Retry"));
    assert!(rendered.contains("Close"));
    assert!(rendered.contains("Restart"));
    assert_chatwidget_snapshot!("mcp_oauth_refresh_error", rendered);
}

#[tokio::test]
async fn mcp_oauth_refresh_inventory_retry_failure_stays_safe_and_retryable() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.open_mcp_picker_loading();
    let secret = "https://oauth.example/authorize?state=DO_NOT_PERSIST";

    chat.on_mcp_picker_inventory_loaded(
        Err(format!("inventory failed at {secret}")),
        Some("sentry".to_string()),
    );

    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    assert!(rendered.contains("Authentication succeeded"));
    assert!(rendered.to_ascii_lowercase().contains("restart"));
    assert!(rendered.contains("Retry"));
    assert!(!rendered.contains(secret));
    assert!(!rendered.contains("DO_NOT_PERSIST"));
    press(&mut chat, KeyCode::Enter);
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::FetchMcpPickerInventory {
            thread_id: Some(actual_thread_id),
            focus_server: Some(server),
        }) if actual_thread_id == thread_id && server == "sentry"
    );
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn mcp_oauth_error_sanitizer_redacts_header_token_and_environment_values() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status("sentry", McpAuthStatus::NotLoggedIn, false, &[]);
    chat.open_mcp_server_detail(server.clone());
    chat.open_mcp_oauth_starting(server.clone());

    let _ = chat.show_mcp_oauth_error(
        server,
        "Authorization: Bearer DO_NOT_PERSIST; token: TOKEN_SECRET env=ENV_SECRET".to_string(),
    );

    let rendered = render_bottom_popup(&chat, /*width*/ 100);
    assert!(!rendered.contains("DO_NOT_PERSIST"));
    assert!(!rendered.contains("TOKEN_SECRET"));
    assert!(!rendered.contains("ENV_SECRET"));
    assert!(rendered.contains("[REDACTED]"));
}

#[tokio::test]
async fn mcp_oauth_error_sanitizer_redacts_structured_quoted_and_plural_values_in_ui() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let server = status("sentry", McpAuthStatus::NotLoggedIn, false, &[]);
    chat.open_mcp_server_detail(server.clone());
    chat.open_mcp_oauth_starting(server.clone());

    for (message, secrets) in [
        (
            r#"headers={"Authorization":"Bearer DO_NOT_PERSIST"}"#,
            &["DO_NOT_PERSIST"][..],
        ),
        (
            r#"{"state":"STATE_SECRET","access_token":"TOKEN_SECRET"}"#,
            &["STATE_SECRET", "TOKEN_SECRET"][..],
        ),
        (
            r#"{"credential":"CREDENTIAL_SECRET","environment":"ENVIRONMENT_SECRET","env":"ENV_SECRET","token":"QUOTED_TOKEN_SECRET"}"#,
            &[
                "CREDENTIAL_SECRET",
                "ENVIRONMENT_SECRET",
                "ENV_SECRET",
                "QUOTED_TOKEN_SECRET",
            ][..],
        ),
        (
            "Authorization Bearer AUTH_SECRET header X-Api-Key HEADER_SECRET",
            &["AUTH_SECRET", "HEADER_SECRET"][..],
        ),
    ] {
        assert!(chat.show_mcp_oauth_error(server.clone(), message.to_string()));
        let rendered = render_bottom_popup(&chat, /*width*/ 200);
        for secret in secrets {
            assert!(
                !rendered.contains(secret),
                "rendered secret {secret}: {rendered}"
            );
        }
        assert!(rendered.contains("[REDACTED]"));
    }
}
