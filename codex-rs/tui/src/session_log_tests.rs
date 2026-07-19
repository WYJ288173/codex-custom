use super::*;
use crate::app_event::McpOauthAuthorizationUrl;
use crate::app_event::McpOauthLoginResult;
use crate::app_event::McpOauthRefreshResult;

const SECRET_URL: &str = "https://oauth.example/authorize?state=DO_NOT_PERSIST";

#[test]
fn mcp_oauth_session_log_metadata_never_formats_sensitive_event_payloads() {
    let login = AppEvent::McpOauthLoginStarted {
        operation_id: "mcp-oauth-operation".to_string(),
        result: McpOauthLoginResult::new(Ok(McpOauthAuthorizationUrl::new(SECRET_URL.to_string()))),
    };
    let refresh = AppEvent::McpOauthRefreshFinished {
        operation_id: "mcp-oauth-operation".to_string(),
        thread_id: None,
        result: McpOauthRefreshResult::new(Err(format!(
            "reload failed at {SECRET_URL} with state=DO_NOT_PERSIST"
        ))),
    };

    let metadata = [
        safe_app_event_metadata(&login),
        safe_app_event_metadata(&refresh),
    ];

    assert_eq!(
        metadata,
        [
            Some(serde_json::json!({
                "variant": "McpOauthLoginStarted",
                "ok": true,
            })),
            Some(serde_json::json!({
                "variant": "McpOauthRefreshFinished",
                "ok": false,
            })),
        ]
    );
    let rendered = format!("{metadata:?}");
    assert!(!rendered.contains(SECRET_URL));
    assert!(!rendered.contains("DO_NOT_PERSIST"));
}

#[test]
fn mcp_inventory_session_log_metadata_never_formats_errors_or_statuses() {
    let picker = AppEvent::McpPickerInventoryLoaded {
        result: Err("picker failed with token DO_NOT_PERSIST".to_string()),
        thread_id: None,
        focus_server: Some("sentry".to_string()),
    };
    let inventory_error = AppEvent::McpInventoryLoaded {
        result: Err("inventory failed with credential DO_NOT_PERSIST".to_string()),
        detail: codex_app_server_protocol::McpServerStatusDetail::Full,
        thread_id: None,
    };
    let inventory_success = AppEvent::McpInventoryLoaded {
        result: Ok(Vec::new()),
        detail: codex_app_server_protocol::McpServerStatusDetail::ToolsAndAuthOnly,
        thread_id: None,
    };

    let metadata = [
        safe_app_event_metadata(&picker),
        safe_app_event_metadata(&inventory_error),
        safe_app_event_metadata(&inventory_success),
    ];

    assert_eq!(
        metadata,
        [
            Some(serde_json::json!({
                "variant": "McpPickerInventoryLoaded",
                "ok": false,
                "count": null,
            })),
            Some(serde_json::json!({
                "variant": "McpInventoryLoaded",
                "ok": false,
                "count": null,
                "detail": "full",
            })),
            Some(serde_json::json!({
                "variant": "McpInventoryLoaded",
                "ok": true,
                "count": 0,
                "detail": "toolsAndAuthOnly",
            })),
        ]
    );
    let rendered = format!("{metadata:?}");
    assert!(!rendered.contains("DO_NOT_PERSIST"));
    assert!(!rendered.contains("picker failed"));
    assert!(!rendered.contains("inventory failed"));
}
