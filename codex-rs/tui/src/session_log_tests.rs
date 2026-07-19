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
        result: McpOauthRefreshResult::new(Err(format!(
            "reload failed at {SECRET_URL} with state=DO_NOT_PERSIST"
        ))),
    };

    let metadata = [
        oauth_app_event_metadata(&login),
        oauth_app_event_metadata(&refresh),
    ];

    assert_eq!(
        metadata,
        [
            Some(("McpOauthLoginStarted", Some(true))),
            Some(("McpOauthRefreshFinished", Some(false))),
        ]
    );
    let rendered = format!("{metadata:?}");
    assert!(!rendered.contains(SECRET_URL));
    assert!(!rendered.contains("DO_NOT_PERSIST"));
}
