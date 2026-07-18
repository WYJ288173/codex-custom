use super::*;
use crate::app::test_support::make_test_app;
use crate::app_event::McpOauthAuthorizationUrl;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::McpAuthStatus;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;

fn server(name: &str) -> McpServerStatus {
    McpServerStatus {
        name: name.to_string(),
        server_info: None,
        tools: Default::default(),
        resources: Vec::new(),
        resource_templates: Vec::new(),
        auth_status: McpAuthStatus::NotLoggedIn,
    }
}

#[test]
fn oauth_login_request_uses_selected_server_and_unbounded_defaults() {
    let request = mcp_oauth_login_request("mcp-oauth-operation", &server("sentry"));

    match request {
        ClientRequest::McpServerOauthLogin { request_id, params } => {
            assert_eq!(
                request_id,
                RequestId::String("mcp-oauth-operation".to_string())
            );
            assert_eq!(params.name, "sentry");
            assert_eq!(params.thread_id, None);
            assert_eq!(params.scopes, None);
            assert_eq!(params.timeout_secs, None);
        }
        other => panic!("expected MCP OAuth login request, got {other:?}"),
    }
}

#[test]
fn oauth_authorization_url_debug_is_redacted() {
    let secret_url = "https://auth.example.test/authorize?state=secret-state";
    let authorization_url = McpOauthAuthorizationUrl::new(secret_url.to_string());

    let debug = format!("{authorization_url:?}");

    assert_eq!(debug, "McpOauthAuthorizationUrl([REDACTED])");
    assert!(!debug.contains(secret_url));
    assert!(!debug.contains("secret-state"));
}

#[tokio::test]
async fn first_oauth_operation_enters_requesting_url() {
    let mut app = make_test_app().await;
    let selected_server = server("sentry");

    let operation_id = app
        .begin_mcp_oauth(selected_server.clone())
        .expect("first OAuth operation should start");

    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("OAuth operation should be pending");
    assert_eq!(pending.operation_id, operation_id);
    assert_eq!(pending.server, selected_server);
    assert_eq!(pending.authorization_url, None);
    assert_eq!(pending.phase, PendingMcpOauthPhase::RequestingUrl);
}

#[tokio::test]
async fn second_oauth_operation_is_rejected_without_replacing_pending_state() {
    let mut app = make_test_app().await;
    let first_server = server("first");
    let first_operation_id = app
        .begin_mcp_oauth(first_server.clone())
        .expect("first OAuth operation should start");

    let second = app.begin_mcp_oauth(server("second"));

    assert_eq!(second, None);
    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("first OAuth operation should remain pending");
    assert_eq!(pending.operation_id, first_operation_id);
    assert_eq!(pending.server, first_server);
    assert_eq!(pending.phase, PendingMcpOauthPhase::RequestingUrl);
}

#[tokio::test]
async fn mismatched_oauth_login_result_is_ignored() {
    let mut app = make_test_app().await;
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should start");

    app.handle_mcp_oauth_login_started(
        "stale-operation".to_string(),
        Ok(McpOauthAuthorizationUrl::new(
            "https://auth.example.test/authorize?state=secret-state".to_string(),
        )),
    );

    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("matching operation should remain pending");
    assert_eq!(pending.operation_id, operation_id);
    assert_eq!(pending.authorization_url, None);
    assert_eq!(pending.phase, PendingMcpOauthPhase::RequestingUrl);
}

#[tokio::test]
async fn matching_oauth_login_result_stores_url_and_waits_for_completion() {
    let mut app = make_test_app().await;
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should start");
    let authorization_url = McpOauthAuthorizationUrl::new(
        "https://auth.example.test/authorize?state=secret-state".to_string(),
    );

    app.handle_mcp_oauth_login_started(operation_id, Ok(authorization_url.clone()));

    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("OAuth operation should wait for completion");
    assert_eq!(
        pending.authorization_url.as_ref().map(|url| url.as_str()),
        Some(authorization_url.as_str())
    );
    assert_eq!(pending.phase, PendingMcpOauthPhase::WaitingForCompletion);
}

#[tokio::test]
async fn matching_oauth_login_failure_clears_pending_operation() {
    let mut app = make_test_app().await;
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should start");

    app.handle_mcp_oauth_login_started(
        operation_id,
        Err("authorization service unavailable".to_string()),
    );

    assert!(app.pending_mcp_oauth.is_none());
}

#[tokio::test]
async fn oauth_start_after_mcp_views_close_keeps_panel_closed() {
    let mut app = make_test_app().await;
    let selected_server = server("sentry");
    app.chat_widget
        .open_mcp_server_detail(selected_server.clone());
    app.chat_widget.dismiss_mcp_views();
    assert!(app.chat_widget.no_modal_or_popup_active());

    let operation_id = app
        .begin_mcp_oauth(selected_server.clone())
        .expect("OAuth operation should start after the panel closes");

    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("OAuth operation should continue in the background");
    assert_eq!(pending.operation_id, operation_id);
    assert_eq!(pending.server, selected_server);
    assert_eq!(pending.phase, PendingMcpOauthPhase::RequestingUrl);
    assert!(app.chat_widget.no_modal_or_popup_active());
}

#[tokio::test]
async fn late_oauth_login_result_after_mcp_views_close_does_not_reopen_panel() {
    let mut app = make_test_app().await;
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should start");
    assert!(app.chat_widget.no_modal_or_popup_active());

    app.handle_mcp_oauth_login_started(
        operation_id,
        Ok(McpOauthAuthorizationUrl::new(
            "https://auth.example.test/authorize?state=secret-state".to_string(),
        )),
    );

    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("OAuth operation should wait in the background");
    assert_eq!(pending.phase, PendingMcpOauthPhase::WaitingForCompletion);
    assert!(app.chat_widget.no_modal_or_popup_active());
}

#[tokio::test]
async fn duplicate_oauth_start_shows_busy_feedback_without_replacing_pending_operation() {
    let mut app = make_test_app().await;
    let first_server = server("first");
    let operation_id = app
        .begin_mcp_oauth(first_server.clone())
        .expect("first OAuth operation should start");
    app.chat_widget.open_mcp_server_detail(server("second"));

    assert_eq!(app.begin_mcp_oauth(server("second")), None);

    let rendered =
        crate::chatwidget::tests::helpers::render_bottom_popup(&app.chat_widget, /*width*/ 80);
    assert!(rendered.contains("Already authenticating first"));
    assert_eq!(app.begin_mcp_oauth(server("third")), None);
    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("first OAuth operation should remain pending");
    assert_eq!(pending.operation_id, operation_id);
    assert_eq!(pending.server, first_server);
    assert_eq!(pending.authorization_url, None);
    assert_eq!(pending.phase, PendingMcpOauthPhase::RequestingUrl);
}
