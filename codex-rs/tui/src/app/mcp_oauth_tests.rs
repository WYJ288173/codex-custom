use super::*;
use crate::app::test_support::make_test_app;
use crate::app::test_support::make_test_app_with_event_rx;
use crate::app_event::AppEvent;
use crate::app_event::McpOauthAuthorizationUrl;
use crate::app_event::McpOauthLoginResult;
use crate::app_event::McpOauthRefreshResult;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::McpAuthStatus;
use codex_app_server_protocol::McpServerOauthLoginCompletedNotification;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use pretty_assertions::assert_eq;

const SECRET_URL: &str = "https://oauth.example/authorize?state=DO_NOT_PERSIST";

fn server(name: &str) -> McpServerStatus {
    McpServerStatus {
        name: name.to_string(),
        plugin_id: None,
        server_info: None,
        tools: Default::default(),
        resources: Vec::new(),
        resource_templates: Vec::new(),
        auth_status: McpAuthStatus::NotLoggedIn,
    }
}

fn lines_to_string(lines: &[ratatui::text::Line<'_>]) -> String {
    let mut rendered = String::new();
    for line in lines {
        for span in &line.spans {
            rendered.push_str(&span.content);
        }
        rendered.push('\n');
    }
    rendered
}

fn oauth_completion(
    name: &str,
    success: bool,
    error: Option<&str>,
) -> McpServerOauthLoginCompletedNotification {
    oauth_completion_for_thread(name, None, success, error)
}

fn oauth_completion_for_thread(
    name: &str,
    thread_id: Option<codex_protocol::ThreadId>,
    success: bool,
    error: Option<&str>,
) -> McpServerOauthLoginCompletedNotification {
    McpServerOauthLoginCompletedNotification {
        name: name.to_string(),
        thread_id: thread_id.map(|thread_id| thread_id.to_string()),
        success,
        error: error.map(str::to_string),
    }
}

fn set_waiting_oauth(app: &mut App, operation_id: &str, selected_server: McpServerStatus) {
    let origin_thread_id = app.current_displayed_thread_id();
    app.pending_mcp_oauth = Some(PendingMcpOauth {
        operation_id: operation_id.to_string(),
        server: selected_server,
        origin_thread_id,
        authorization_url: Some(McpOauthAuthorizationUrl::new(SECRET_URL.to_string())),
        phase: PendingMcpOauthPhase::WaitingForCompletion,
    });
}

fn set_refreshing_oauth(app: &mut App, operation_id: &str, selected_server: McpServerStatus) {
    let origin_thread_id = app.current_displayed_thread_id();
    app.pending_mcp_oauth = Some(PendingMcpOauth {
        operation_id: operation_id.to_string(),
        server: selected_server,
        origin_thread_id,
        authorization_url: None,
        phase: PendingMcpOauthPhase::Refreshing,
    });
}

#[test]
fn oauth_login_request_uses_selected_server_origin_thread_and_unbounded_defaults() {
    let origin_thread_id = codex_protocol::ThreadId::new();
    let request = mcp_oauth_login_request(
        "mcp-oauth-operation",
        &server("sentry"),
        Some(origin_thread_id),
    );

    match request {
        ClientRequest::McpServerOauthLogin { request_id, params } => {
            assert_eq!(
                request_id,
                RequestId::String("mcp-oauth-operation".to_string())
            );
            assert_eq!(params.name, "sentry");
            assert_eq!(params.thread_id, Some(origin_thread_id.to_string()));
            assert_eq!(params.scopes, None);
            assert_eq!(params.timeout_secs, None);
        }
        other => panic!("expected MCP OAuth login request, got {other:?}"),
    }
}

#[tokio::test]
async fn mcp_oauth_completion_for_another_origin_thread_is_ignored() {
    let mut app = make_test_app().await;
    let origin_thread_id = codex_protocol::ThreadId::new();
    app.active_thread_id = Some(origin_thread_id);
    let selected_server = server("sentry");
    set_waiting_oauth(&mut app, "mcp-oauth-operation", selected_server.clone());

    let refresh_operation =
        app.handle_mcp_oauth_login_completed_transition(McpServerOauthLoginCompletedNotification {
            name: "sentry".to_string(),
            thread_id: Some(codex_protocol::ThreadId::new().to_string()),
            success: true,
            error: None,
        });

    assert_eq!(refresh_operation, None);
    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("a completion from another thread must preserve the pending operation");
    assert_eq!(pending.server, selected_server);
    assert_eq!(pending.phase, PendingMcpOauthPhase::WaitingForCompletion);
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
        pending
            .authorization_url
            .as_ref()
            .map(crate::app_event::McpOauthAuthorizationUrl::as_str),
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
async fn matching_oauth_login_success_after_thread_switch_is_discarded_without_ui_or_browser_event()
{
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let origin_thread_id = codex_protocol::ThreadId::new();
    app.active_thread_id = Some(origin_thread_id);
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should start");
    app.chat_widget.dismiss_mcp_views();
    app.active_thread_id = Some(codex_protocol::ThreadId::new());

    app.handle_mcp_oauth_login_started(
        operation_id,
        Ok(McpOauthAuthorizationUrl::new(SECRET_URL.to_string())),
    );

    assert!(app.pending_mcp_oauth.is_none());
    assert!(app.chat_widget.no_modal_or_popup_active());
    assert!(
        app_event_rx.try_recv().is_err(),
        "stale login success must not open a browser or append history"
    );
}

#[tokio::test]
async fn matching_oauth_login_error_after_thread_switch_is_discarded_without_ui_or_history() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let origin_thread_id = codex_protocol::ThreadId::new();
    app.active_thread_id = Some(origin_thread_id);
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should start");
    app.chat_widget.dismiss_mcp_views();
    app.active_thread_id = Some(codex_protocol::ThreadId::new());

    app.handle_mcp_oauth_login_started(
        operation_id,
        Err("authorization service unavailable".to_string()),
    );

    assert!(app.pending_mcp_oauth.is_none());
    assert!(app.chat_widget.no_modal_or_popup_active());
    assert!(
        app_event_rx.try_recv().is_err(),
        "stale login error must not append history"
    );
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

#[tokio::test]
async fn mcp_oauth_url_login_result_emits_id_only_open_event() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should start");

    app.handle_mcp_oauth_login_started(
        operation_id.clone(),
        Ok(McpOauthAuthorizationUrl::new(SECRET_URL.to_string())),
    );

    let event = app_event_rx
        .try_recv()
        .expect("matching login result should request the initial browser open");
    let debug = format!("{event:?}");
    assert_matches::assert_matches!(
        event,
        AppEvent::OpenPendingMcpOauthUrl { operation_id: actual } if actual == operation_id
    );
    assert!(debug.contains(&operation_id));
    assert!(!debug.contains(SECRET_URL));
    assert!(!debug.contains("DO_NOT_PERSIST"));
    assert!(app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn mcp_oauth_url_initial_open_uses_exact_in_memory_url_once_without_history() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should start");
    app.handle_mcp_oauth_login_started(
        operation_id.clone(),
        Ok(McpOauthAuthorizationUrl::new(SECRET_URL.to_string())),
    );
    assert_matches::assert_matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::OpenPendingMcpOauthUrl { operation_id: actual }) if actual == operation_id
    );
    let mut opened = Vec::new();

    app.open_pending_mcp_oauth_url_with(&operation_id, |url| {
        opened.push(url.to_string());
        Ok::<_, String>(())
    });

    assert_eq!(opened, vec![SECRET_URL.to_string()]);
    assert!(app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn queued_mcp_oauth_url_open_after_thread_switch_is_discarded_without_calling_opener() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let origin_thread_id = codex_protocol::ThreadId::new();
    app.active_thread_id = Some(origin_thread_id);
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should start");
    app.handle_mcp_oauth_login_started(
        operation_id.clone(),
        Ok(McpOauthAuthorizationUrl::new(SECRET_URL.to_string())),
    );
    assert_matches::assert_matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::OpenPendingMcpOauthUrl { operation_id: actual })
            if actual == operation_id
    );
    app.chat_widget.dismiss_mcp_views();
    app.active_thread_id = Some(codex_protocol::ThreadId::new());
    let mut opener_called = false;

    app.open_pending_mcp_oauth_url_with(&operation_id, |_url| {
        opener_called = true;
        Err::<(), _>("simulated browser failure")
    });

    assert!(
        !opener_called,
        "stale queued open must not invoke the opener"
    );
    assert!(app.pending_mcp_oauth.is_none());
    assert!(app.chat_widget.no_modal_or_popup_active());
    assert!(
        app_event_rx.try_recv().is_err(),
        "stale queued open must not append browser failure history"
    );
}

#[tokio::test]
async fn mcp_oauth_url_stale_missing_and_non_waiting_operations_do_not_open() {
    let mut app = make_test_app().await;
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should start");
    let mut opened = Vec::new();

    app.open_pending_mcp_oauth_url_with("stale-operation", |url| {
        opened.push(url.to_string());
        Ok::<_, String>(())
    });
    app.open_pending_mcp_oauth_url_with(&operation_id, |url| {
        opened.push(url.to_string());
        Ok::<_, String>(())
    });
    app.pending_mcp_oauth
        .as_mut()
        .expect("OAuth operation should remain pending")
        .phase = PendingMcpOauthPhase::WaitingForCompletion;
    app.open_pending_mcp_oauth_url_with(&operation_id, |url| {
        opened.push(url.to_string());
        Ok::<_, String>(())
    });
    app.pending_mcp_oauth
        .as_mut()
        .expect("OAuth operation should remain pending")
        .authorization_url = Some(McpOauthAuthorizationUrl::new(SECRET_URL.to_string()));
    app.open_pending_mcp_oauth_url_with("stale-operation", |url| {
        opened.push(url.to_string());
        Ok::<_, String>(())
    });

    assert!(opened.is_empty());
    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("guard failures must preserve the pending operation");
    assert_eq!(pending.operation_id, operation_id);
    assert_eq!(
        pending
            .authorization_url
            .as_ref()
            .map(McpOauthAuthorizationUrl::as_str),
        Some(SECRET_URL)
    );
    assert_eq!(pending.phase, PendingMcpOauthPhase::WaitingForCompletion);
}

#[tokio::test]
async fn mcp_oauth_url_visible_failure_is_sanitized_and_retryable() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let selected_server = server("sentry");
    app.chat_widget
        .open_mcp_server_detail(selected_server.clone());
    let operation_id = app
        .begin_mcp_oauth(selected_server)
        .expect("OAuth operation should start");
    app.handle_mcp_oauth_login_started(
        operation_id.clone(),
        Ok(McpOauthAuthorizationUrl::new(SECRET_URL.to_string())),
    );
    let _ = app_event_rx.try_recv();

    app.open_pending_mcp_oauth_url_with(&operation_id, |_url| {
        Err::<(), _>(format!("simulated opener failure at {SECRET_URL}"))
    });

    let rendered =
        crate::chatwidget::tests::helpers::render_bottom_popup(&app.chat_widget, /*width*/ 80);
    assert!(rendered.contains("Failed to open browser: simulated opener failure at [REDACTED]"));
    assert!(rendered.contains("Open browser again"));
    assert!(!rendered.contains(SECRET_URL));
    assert!(!rendered.contains("DO_NOT_PERSIST"));
    assert!(app_event_rx.try_recv().is_err());
    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("browser failure must retain pending OAuth");
    assert_eq!(pending.operation_id, operation_id);
    assert_eq!(pending.phase, PendingMcpOauthPhase::WaitingForCompletion);
}

#[tokio::test]
async fn mcp_oauth_url_failure_after_close_adds_sanitized_history_without_reopening() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let selected_server = server("sentry");
    app.chat_widget
        .open_mcp_server_detail(selected_server.clone());
    let operation_id = app
        .begin_mcp_oauth(selected_server)
        .expect("OAuth operation should start");
    app.handle_mcp_oauth_login_started(
        operation_id.clone(),
        Ok(McpOauthAuthorizationUrl::new(SECRET_URL.to_string())),
    );
    let _ = app_event_rx.try_recv();
    app.chat_widget.dismiss_mcp_views();

    app.open_pending_mcp_oauth_url_with(&operation_id, |_url| {
        Err::<(), _>(format!("simulated opener failure at {SECRET_URL}"))
    });

    assert!(app.chat_widget.no_modal_or_popup_active());
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected one sanitized history cell, got {other:?}"),
    };
    let rendered = lines_to_string(&cell.display_lines(/*width*/ 80));
    assert!(rendered.contains("Failed to open browser: simulated opener failure at [REDACTED]"));
    assert!(!rendered.contains(SECRET_URL));
    assert!(!rendered.contains("DO_NOT_PERSIST"));
    assert!(app_event_rx.try_recv().is_err());
    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("failure after close must retain pending OAuth");
    assert_eq!(pending.operation_id, operation_id);
    assert_eq!(pending.phase, PendingMcpOauthPhase::WaitingForCompletion);
}

#[tokio::test]
async fn mcp_oauth_completion_for_another_server_is_ignored() {
    let mut app = make_test_app().await;
    let selected_server = server("sentry");
    set_waiting_oauth(&mut app, "mcp-oauth-operation", selected_server.clone());

    let refresh_operation = app.handle_mcp_oauth_login_completed_transition(oauth_completion(
        "other-server",
        true,
        None,
    ));

    assert_eq!(refresh_operation, None);
    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("mismatched completion must preserve pending OAuth");
    assert_eq!(pending.operation_id, "mcp-oauth-operation");
    assert_eq!(pending.server, selected_server);
    assert_eq!(
        pending
            .authorization_url
            .as_ref()
            .map(McpOauthAuthorizationUrl::as_str),
        Some(SECRET_URL)
    );
    assert_eq!(pending.phase, PendingMcpOauthPhase::WaitingForCompletion);
}

#[tokio::test]
async fn mcp_oauth_completion_failure_after_thread_switch_does_not_mutate_new_thread() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let origin_thread_id = codex_protocol::ThreadId::new();
    app.active_thread_id = Some(origin_thread_id);
    let operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should begin");
    app.handle_mcp_oauth_login_started(
        operation_id.clone(),
        McpOauthLoginResult::new(Ok(McpOauthAuthorizationUrl::new(SECRET_URL.to_string()))),
    );
    assert_matches::assert_matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::OpenPendingMcpOauthUrl { operation_id: opened })
            if opened == operation_id
    );
    app.chat_widget.dismiss_mcp_views();
    app.active_thread_id = Some(codex_protocol::ThreadId::new());

    let refresh_operation = app.handle_mcp_oauth_login_completed_transition(oauth_completion(
        "sentry",
        false,
        Some("provider rejected login"),
    ));

    assert_eq!(refresh_operation, None);
    assert!(app.pending_mcp_oauth.is_none());
    assert!(app.chat_widget.no_modal_or_popup_active());
    assert!(
        app_event_rx.try_recv().is_err(),
        "completion from the previous thread must not append history"
    );
}

#[tokio::test]
async fn mcp_oauth_completion_success_after_thread_switch_clears_pending_without_refresh() {
    let mut app = make_test_app().await;
    let origin_thread_id = codex_protocol::ThreadId::new();
    app.active_thread_id = Some(origin_thread_id);
    let _operation_id = app
        .begin_mcp_oauth(server("sentry"))
        .expect("OAuth operation should begin");
    app.chat_widget.dismiss_mcp_views();
    app.active_thread_id = Some(codex_protocol::ThreadId::new());

    let refresh_operation =
        app.handle_mcp_oauth_login_completed_transition(oauth_completion("sentry", true, None));

    assert_eq!(refresh_operation, None);
    assert!(app.pending_mcp_oauth.is_none());
    assert!(app.chat_widget.no_modal_or_popup_active());
}

#[tokio::test]
async fn mcp_oauth_completion_failure_visible_is_sanitized_and_retryable() {
    let mut app = make_test_app().await;
    let selected_server = server("sentry");
    app.chat_widget.open_mcp_picker_loading();
    app.chat_widget
        .open_mcp_server_detail(selected_server.clone());
    app.chat_widget
        .open_mcp_oauth_starting(selected_server.clone());
    app.chat_widget
        .show_mcp_oauth_progress(selected_server.clone(), "mcp-oauth-operation".to_string());
    set_waiting_oauth(&mut app, "mcp-oauth-operation", selected_server);
    let secret_error = format!("provider rejected state=DO_NOT_PERSIST at {SECRET_URL}");

    let refresh_operation = app.handle_mcp_oauth_login_completed_transition(oauth_completion(
        "sentry",
        false,
        Some(&secret_error),
    ));

    assert_eq!(refresh_operation, None);
    assert!(app.pending_mcp_oauth.is_none());
    let rendered =
        crate::chatwidget::tests::helpers::render_bottom_popup(&app.chat_widget, /*width*/ 80);
    assert!(rendered.contains("Authentication failed"));
    assert!(rendered.contains("Retry"));
    assert!(rendered.contains("Close"));
    assert!(!rendered.contains(SECRET_URL));
    assert!(!rendered.contains("DO_NOT_PERSIST"));
}

#[tokio::test]
async fn mcp_oauth_completion_failure_after_close_adds_one_safe_history_message() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let origin_thread_id = codex_protocol::ThreadId::new();
    app.active_thread_id = Some(origin_thread_id);
    set_waiting_oauth(&mut app, "mcp-oauth-operation", server("sentry"));
    let secret_error = format!("provider rejected state=DO_NOT_PERSIST at {SECRET_URL}");

    let refresh_operation = app.handle_mcp_oauth_login_completed_transition(
        oauth_completion_for_thread("sentry", Some(origin_thread_id), false, Some(&secret_error)),
    );

    assert_eq!(refresh_operation, None);
    assert!(app.pending_mcp_oauth.is_none());
    assert!(app.chat_widget.no_modal_or_popup_active());
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected one OAuth failure history cell, got {other:?}"),
    };
    let rendered = lines_to_string(&cell.display_lines(/*width*/ 80));
    assert!(rendered.contains("Authentication failed for MCP server 'sentry'"));
    assert!(!rendered.contains(SECRET_URL));
    assert!(!rendered.contains("DO_NOT_PERSIST"));
    assert!(app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn mcp_oauth_completion_failure_history_redacts_structured_secret_payloads() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    set_waiting_oauth(&mut app, "mcp-oauth-operation", server("sentry"));
    let structured_error = r#"headers={"Authorization":"Bearer DO_NOT_PERSIST"} {"state":"STATE_SECRET","access_token":"TOKEN_SECRET"}"#;

    let refresh_operation = app.handle_mcp_oauth_login_completed_transition(oauth_completion(
        "sentry",
        false,
        Some(structured_error),
    ));

    assert_eq!(refresh_operation, None);
    assert!(app.pending_mcp_oauth.is_none());
    assert!(app.chat_widget.no_modal_or_popup_active());
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected one OAuth failure history cell, got {other:?}"),
    };
    let rendered = lines_to_string(&cell.display_lines(/*width*/ 200));
    for secret in ["DO_NOT_PERSIST", "STATE_SECRET", "TOKEN_SECRET"] {
        assert!(
            !rendered.contains(secret),
            "history leaked {secret}: {rendered}"
        );
    }
    assert!(!rendered.contains("{\"Authorization\""));
    assert!(rendered.contains("[REDACTED]"));
    assert!(app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn mcp_oauth_completion_success_enters_refreshing_drops_url_and_builds_typed_request() {
    let mut app = make_test_app().await;
    set_waiting_oauth(&mut app, "mcp-oauth-operation", server("sentry"));

    let refresh_operation =
        app.handle_mcp_oauth_login_completed_transition(oauth_completion("sentry", true, None));

    assert_eq!(refresh_operation.as_deref(), Some("mcp-oauth-operation"));
    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("matching success should stay pending through refresh");
    assert_eq!(pending.authorization_url, None);
    assert_eq!(pending.phase, PendingMcpOauthPhase::Refreshing);
    assert_matches::assert_matches!(
        mcp_oauth_refresh_request("mcp-oauth-operation"),
        ClientRequest::McpServerRefresh {
            request_id: RequestId::String(request_id),
            params: None,
        } if request_id == "mcp-oauth-operation-refresh"
    );
}

#[tokio::test]
async fn mcp_oauth_completion_notification_is_consumed_once_at_app_level() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    set_waiting_oauth(&mut app, "mcp-oauth-operation", server("sentry"));
    let app_server = crate::start_embedded_app_server_for_picker(&app.config)
        .await
        .expect("embedded app server");

    app.handle_app_server_event(
        &app_server,
        AppServerEvent::ServerNotification(Box::new(
            ServerNotification::McpServerOauthLoginCompleted(oauth_completion(
                "sentry",
                false,
                Some("provider rejected login"),
            )),
        )),
    )
    .await;

    assert!(app.pending_mcp_oauth.is_none());
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected one completion history cell, got {other:?}"),
    };
    assert!(
        lines_to_string(&cell.display_lines(/*width*/ 80))
            .contains("Authentication failed for MCP server 'sentry'")
    );
    assert!(app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn mcp_oauth_completion_refresh_inventory_reuses_paginated_tools_and_auth_fetch() {
    let mut app = make_test_app().await;
    let mut config_toml = String::new();
    let configured_servers = (0..101)
        .map(|index| {
            let name = format!("server-{index:03}");
            config_toml.push_str(&format!("[mcp_servers.\"{name}\"]\ncommand = \"true\"\n"));
            let config = toml::from_str::<toml::Value>("command = 'true'")
                .expect("test MCP config should parse")
                .try_into()
                .expect("test MCP config should deserialize");
            (name, config)
        })
        .collect();
    app.config
        .mcp_servers
        .set(configured_servers)
        .expect("test MCP servers should accept any configuration");
    std::fs::write(app.config.codex_home.join("config.toml"), config_toml)
        .expect("test MCP config should be written");
    let app_server = crate::start_embedded_app_server_for_picker(&app.config)
        .await
        .expect("embedded app server");

    let statuses = crate::app::background_requests::fetch_all_mcp_server_statuses(
        app_server.request_handle(),
        McpServerStatusDetail::ToolsAndAuthOnly,
        /*thread_id*/ None,
    )
    .await
    .expect("paginated MCP inventory should load");

    assert_eq!(statuses.len(), 101);
    assert_eq!(
        statuses.first().map(|status| status.name.as_str()),
        Some("server-000")
    );
    assert_eq!(
        statuses.last().map(|status| status.name.as_str()),
        Some("server-100")
    );
    assert!(
        statuses
            .iter()
            .all(|status| { status.resources.is_empty() && status.resource_templates.is_empty() })
    );
}

#[tokio::test]
async fn mcp_oauth_late_login_url_after_completion_is_ignored_without_browser_event() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    set_waiting_oauth(&mut app, "mcp-oauth-operation", server("sentry"));
    assert_eq!(
        app.handle_mcp_oauth_login_completed_transition(oauth_completion("sentry", true, None,))
            .as_deref(),
        Some("mcp-oauth-operation")
    );

    app.handle_mcp_oauth_login_started(
        "mcp-oauth-operation".to_string(),
        McpOauthLoginResult::new(Ok(McpOauthAuthorizationUrl::new(SECRET_URL.to_string()))),
    );

    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("refreshing operation should remain pending");
    assert_eq!(pending.authorization_url, None);
    assert_eq!(pending.phase, PendingMcpOauthPhase::Refreshing);
    assert!(app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn mcp_oauth_stale_refresh_operation_and_phase_are_ignored() {
    let mut app = make_test_app().await;
    let selected_server = server("sentry");
    set_refreshing_oauth(&mut app, "mcp-oauth-operation", selected_server.clone());

    app.handle_mcp_oauth_refresh_finished(
        "stale-operation".to_string(),
        None,
        McpOauthRefreshResult::new(Ok(vec![selected_server.clone()])),
    );

    let pending = app
        .pending_mcp_oauth
        .as_ref()
        .expect("stale refresh must preserve pending state");
    assert_eq!(pending.operation_id, "mcp-oauth-operation");
    assert_eq!(pending.phase, PendingMcpOauthPhase::Refreshing);

    app.pending_mcp_oauth.as_mut().expect("pending OAuth").phase =
        PendingMcpOauthPhase::WaitingForCompletion;
    app.handle_mcp_oauth_refresh_finished(
        "mcp-oauth-operation".to_string(),
        None,
        McpOauthRefreshResult::new(Ok(vec![selected_server])),
    );
    assert_eq!(
        app.pending_mcp_oauth
            .as_ref()
            .expect("wrong phase must preserve pending state")
            .phase,
        PendingMcpOauthPhase::WaitingForCompletion
    );
}

#[tokio::test]
async fn mcp_oauth_refresh_success_restores_fresh_exact_detail_only_while_panel_is_open() {
    let mut app = make_test_app().await;
    let stale_server = server("sentry");
    let mut fresh_server = server("sentry");
    fresh_server.auth_status = McpAuthStatus::OAuth;
    let differently_cased = server("Sentry");
    app.chat_widget.open_mcp_picker_loading();
    app.chat_widget
        .on_mcp_picker_inventory_loaded(Ok(vec![stale_server.clone()]), /*focus_server*/ None);
    app.chat_widget.open_mcp_server_detail(stale_server.clone());
    app.chat_widget
        .open_mcp_oauth_starting(stale_server.clone());
    set_refreshing_oauth(&mut app, "mcp-oauth-operation", stale_server);

    app.handle_mcp_oauth_refresh_finished(
        "mcp-oauth-operation".to_string(),
        None,
        McpOauthRefreshResult::new(Ok(vec![differently_cased, fresh_server])),
    );

    assert!(app.pending_mcp_oauth.is_none());
    let rendered =
        crate::chatwidget::tests::helpers::render_bottom_popup(&app.chat_widget, /*width*/ 80);
    assert!(rendered.contains("Authentication: OAuth"));
    assert!(rendered.contains("MCP server · sentry"));
}

#[tokio::test]
async fn mcp_oauth_refresh_success_after_close_adds_one_history_message_without_modal() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let origin_thread_id = codex_protocol::ThreadId::new();
    app.active_thread_id = Some(origin_thread_id);
    let selected_server = server("sentry");
    set_refreshing_oauth(&mut app, "mcp-oauth-operation", selected_server.clone());

    app.handle_mcp_oauth_refresh_finished(
        "mcp-oauth-operation".to_string(),
        Some(origin_thread_id),
        McpOauthRefreshResult::new(Ok(vec![selected_server])),
    );

    assert!(app.pending_mcp_oauth.is_none());
    assert!(app.chat_widget.no_modal_or_popup_active());
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected one OAuth success history cell, got {other:?}"),
    };
    assert_eq!(
        lines_to_string(&cell.display_lines(/*width*/ 80)).trim(),
        "• Authenticated MCP server 'sentry'."
    );
    assert!(app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn mcp_oauth_refresh_result_after_thread_switch_does_not_mutate_new_thread() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    let origin_thread_id = codex_protocol::ThreadId::new();
    app.active_thread_id = Some(origin_thread_id);
    let selected_server = server("sentry");
    let operation_id = app
        .begin_mcp_oauth(selected_server.clone())
        .expect("OAuth operation should begin");
    assert_eq!(
        app.handle_mcp_oauth_login_completed_transition(oauth_completion_for_thread(
            "sentry",
            Some(origin_thread_id),
            true,
            None,
        )),
        Some(operation_id.clone())
    );
    app.chat_widget.dismiss_mcp_views();
    app.active_thread_id = Some(codex_protocol::ThreadId::new());

    app.handle_mcp_oauth_refresh_finished(
        operation_id,
        Some(origin_thread_id),
        McpOauthRefreshResult::new(Ok(vec![selected_server])),
    );

    assert!(app.pending_mcp_oauth.is_none());
    assert!(app.chat_widget.no_modal_or_popup_active());
    assert!(
        app_event_rx.try_recv().is_err(),
        "refresh from the previous thread must not append history"
    );
}

#[tokio::test]
async fn mcp_oauth_refresh_failure_visible_mentions_restart_and_hides_secret() {
    let mut app = make_test_app().await;
    let selected_server = server("sentry");
    app.chat_widget.open_mcp_picker_loading();
    app.chat_widget
        .open_mcp_server_detail(selected_server.clone());
    app.chat_widget
        .open_mcp_oauth_starting(selected_server.clone());
    set_refreshing_oauth(&mut app, "mcp-oauth-operation", selected_server);

    app.handle_mcp_oauth_refresh_finished(
        "mcp-oauth-operation".to_string(),
        None,
        McpOauthRefreshResult::new(Err(format!(
            "reload failed for state=DO_NOT_PERSIST at {SECRET_URL}"
        ))),
    );

    assert!(app.pending_mcp_oauth.is_none());
    let rendered =
        crate::chatwidget::tests::helpers::render_bottom_popup(&app.chat_widget, /*width*/ 80);
    assert!(rendered.contains("Authentication succeeded"));
    assert!(rendered.to_ascii_lowercase().contains("restart"));
    assert!(rendered.contains("Retry"));
    assert!(!rendered.contains(SECRET_URL));
    assert!(!rendered.contains("DO_NOT_PERSIST"));
}

#[tokio::test]
async fn mcp_oauth_refresh_missing_server_after_close_reports_safe_restart_guidance() {
    let (mut app, mut app_event_rx) = make_test_app_with_event_rx().await;
    set_refreshing_oauth(&mut app, "mcp-oauth-operation", server("sentry"));

    app.handle_mcp_oauth_refresh_finished(
        "mcp-oauth-operation".to_string(),
        None,
        McpOauthRefreshResult::new(Ok(vec![server("other-server")])),
    );

    assert!(app.pending_mcp_oauth.is_none());
    assert!(app.chat_widget.no_modal_or_popup_active());
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected one reconnect failure history cell, got {other:?}"),
    };
    let rendered = lines_to_string(&cell.display_lines(/*width*/ 80));
    assert!(rendered.contains("Authentication succeeded"));
    assert!(rendered.to_ascii_lowercase().contains("restart"));
    assert!(app_event_rx.try_recv().is_err());
}

#[test]
fn mcp_oauth_app_event_debug_redacts_login_and_refresh_payloads() {
    let login = AppEvent::McpOauthLoginStarted {
        operation_id: "mcp-oauth-operation".to_string(),
        result: McpOauthLoginResult::new(Err(format!(
            "failed at {SECRET_URL} with state=DO_NOT_PERSIST"
        ))),
    };
    let refresh = AppEvent::McpOauthRefreshFinished {
        operation_id: "mcp-oauth-operation".to_string(),
        thread_id: None,
        result: McpOauthRefreshResult::new(Err(format!(
            "failed at {SECRET_URL} with state=DO_NOT_PERSIST"
        ))),
    };

    for debug in [format!("{login:?}"), format!("{refresh:?}")] {
        assert!(!debug.contains(SECRET_URL));
        assert!(!debug.contains("DO_NOT_PERSIST"));
        assert!(debug.contains("[REDACTED]"));
    }
}
