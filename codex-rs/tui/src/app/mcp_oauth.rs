use super::App;
use crate::app_event::AppEvent;
use crate::app_event::McpOauthAuthorizationUrl;
use crate::app_server_session::AppServerSession;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::McpServerOauthLoginParams;
use codex_app_server_protocol::McpServerOauthLoginResponse;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::RequestId;
use std::fmt::Display;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PendingMcpOauthPhase {
    RequestingUrl,
    WaitingForCompletion,
    #[allow(dead_code)]
    Refreshing,
}

#[derive(Clone)]
pub(super) struct PendingMcpOauth {
    pub(super) operation_id: String,
    pub(super) server: McpServerStatus,
    pub(super) authorization_url: Option<McpOauthAuthorizationUrl>,
    pub(super) phase: PendingMcpOauthPhase,
}

impl App {
    pub(super) fn start_mcp_oauth(
        &mut self,
        app_server: &AppServerSession,
        server: McpServerStatus,
    ) {
        let Some(operation_id) = self.begin_mcp_oauth(server.clone()) else {
            tracing::debug!(server = %server.name, "ignored MCP OAuth start while another operation is pending");
            return;
        };
        let request = mcp_oauth_login_request(&operation_id, &server);
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result: Result<McpServerOauthLoginResponse, _> =
                request_handle.request_typed(request).await;
            let result = result
                .map(|response| McpOauthAuthorizationUrl::new(response.authorization_url))
                .map_err(|error| error.to_string());
            app_event_tx.send(AppEvent::McpOauthLoginStarted {
                operation_id,
                result,
            });
        });
    }

    fn begin_mcp_oauth(&mut self, server: McpServerStatus) -> Option<String> {
        if let Some(pending) = &self.pending_mcp_oauth {
            self.chat_widget.show_mcp_oauth_busy(pending.server.clone());
            return None;
        }

        let operation_id = format!("mcp-oauth-{}", Uuid::new_v4());
        self.pending_mcp_oauth = Some(PendingMcpOauth {
            operation_id: operation_id.clone(),
            server: server.clone(),
            authorization_url: None,
            phase: PendingMcpOauthPhase::RequestingUrl,
        });
        self.chat_widget.open_mcp_oauth_starting(server);
        Some(operation_id)
    }

    pub(super) fn handle_mcp_oauth_login_started(
        &mut self,
        operation_id: String,
        result: Result<McpOauthAuthorizationUrl, String>,
    ) {
        let Some(pending) = self.pending_mcp_oauth.as_mut() else {
            tracing::debug!(%operation_id, "ignored MCP OAuth login result without a pending operation");
            return;
        };
        if pending.operation_id != operation_id {
            tracing::debug!(%operation_id, "ignored stale MCP OAuth login result");
            return;
        }

        match result {
            Ok(authorization_url) => {
                pending.authorization_url = Some(authorization_url);
                pending.phase = PendingMcpOauthPhase::WaitingForCompletion;
                let server = pending.server.clone();
                self.chat_widget
                    .show_mcp_oauth_progress(server, operation_id.clone());
                self.app_event_tx
                    .send(AppEvent::OpenPendingMcpOauthUrl { operation_id });
            }
            Err(error) => {
                let server = pending.server.clone();
                self.pending_mcp_oauth = None;
                self.chat_widget.show_mcp_oauth_error(server, error);
            }
        }
    }

    pub(super) fn open_pending_mcp_oauth_url(&mut self, operation_id: String) {
        self.open_pending_mcp_oauth_url_with(&operation_id, webbrowser::open);
    }

    fn open_pending_mcp_oauth_url_with<E, F>(&mut self, operation_id: &str, opener: F)
    where
        E: Display,
        F: FnOnce(&str) -> Result<(), E>,
    {
        let Some(pending) = self.pending_mcp_oauth.as_ref() else {
            return;
        };
        if pending.operation_id != operation_id
            || pending.phase != PendingMcpOauthPhase::WaitingForCompletion
        {
            return;
        }
        let Some(authorization_url) = pending.authorization_url.as_ref() else {
            return;
        };
        let server = pending.server.clone();

        if let Err(error) = opener(authorization_url.as_str()) {
            let message =
                crate::chatwidget::ChatWidget::mcp_oauth_browser_error_message(&error.to_string());
            if !self.chat_widget.show_mcp_oauth_browser_error(
                server,
                operation_id.to_string(),
                message.clone(),
            ) {
                self.chat_widget.add_error_message(message);
            }
        }
    }
}

fn mcp_oauth_login_request(operation_id: &str, server: &McpServerStatus) -> ClientRequest {
    ClientRequest::McpServerOauthLogin {
        request_id: RequestId::String(operation_id.to_string()),
        params: McpServerOauthLoginParams {
            name: server.name.clone(),
            thread_id: None,
            scopes: None,
            timeout_secs: None,
        },
    }
}

#[cfg(test)]
#[path = "mcp_oauth_tests.rs"]
mod tests;
