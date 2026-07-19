use super::App;
use crate::app_event::AppEvent;
use crate::app_event::McpOauthAuthorizationUrl;
use crate::app_event::McpOauthLoginResult;
use crate::app_event::McpOauthRefreshResult;
use crate::app_server_session::AppServerSession;
use crate::chatwidget::sanitize_mcp_oauth_message;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::McpServerOauthLoginCompletedNotification;
use codex_app_server_protocol::McpServerOauthLoginParams;
use codex_app_server_protocol::McpServerOauthLoginResponse;
use codex_app_server_protocol::McpServerRefreshResponse;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::RequestId;
use std::fmt::Display;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PendingMcpOauthPhase {
    RequestingUrl,
    WaitingForCompletion,
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
                result: McpOauthLoginResult::new(result),
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

    pub(super) fn handle_mcp_oauth_login_started<R>(&mut self, operation_id: String, result: R)
    where
        R: Into<McpOauthLoginResult>,
    {
        let Some(pending) = self.pending_mcp_oauth.as_mut() else {
            tracing::debug!(%operation_id, "ignored MCP OAuth login result without a pending operation");
            return;
        };
        if pending.operation_id != operation_id
            || pending.phase != PendingMcpOauthPhase::RequestingUrl
        {
            tracing::debug!(%operation_id, "ignored stale MCP OAuth login result");
            return;
        }

        match result.into().into_result() {
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
                let message = sanitize_mcp_oauth_message(&error);
                if !self
                    .chat_widget
                    .show_mcp_oauth_error(server.clone(), message.clone())
                {
                    self.chat_widget.add_error_message(format!(
                        "Authentication failed for MCP server '{}': {message}",
                        server.name
                    ));
                }
            }
        }
    }

    pub(super) fn handle_mcp_oauth_login_completed(
        &mut self,
        app_server: &AppServerSession,
        notification: McpServerOauthLoginCompletedNotification,
    ) {
        let Some(operation_id) = self.handle_mcp_oauth_login_completed_transition(notification)
        else {
            return;
        };
        self.spawn_mcp_oauth_refresh(app_server, operation_id);
    }

    fn handle_mcp_oauth_login_completed_transition(
        &mut self,
        notification: McpServerOauthLoginCompletedNotification,
    ) -> Option<String> {
        let Some(pending) = self.pending_mcp_oauth.as_mut() else {
            tracing::debug!(server = %notification.name, "ignored MCP OAuth completion without a pending operation");
            return None;
        };
        if pending.server.name != notification.name
            || pending.phase == PendingMcpOauthPhase::Refreshing
        {
            tracing::debug!(server = %notification.name, "ignored stale MCP OAuth completion");
            return None;
        }

        if !notification.success {
            let server = pending.server.clone();
            let error = notification
                .error
                .as_deref()
                .unwrap_or("Authentication failed or timed out.");
            let message = sanitize_mcp_oauth_message(error);
            self.pending_mcp_oauth = None;
            if !self
                .chat_widget
                .show_mcp_oauth_error(server.clone(), message.clone())
            {
                self.chat_widget.add_error_message(format!(
                    "Authentication failed for MCP server '{}': {message}",
                    server.name
                ));
            }
            return None;
        }

        pending.authorization_url = None;
        pending.phase = PendingMcpOauthPhase::Refreshing;
        Some(pending.operation_id.clone())
    }

    fn spawn_mcp_oauth_refresh(&self, app_server: &AppServerSession, operation_id: String) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        let thread_id = self.mcp_inventory_request_thread_id(self.current_displayed_thread_id());
        tokio::spawn(async move {
            let result = async {
                let _: McpServerRefreshResponse = request_handle
                    .request_typed(mcp_oauth_refresh_request(&operation_id))
                    .await
                    .map_err(|error| format!("MCP refresh failed: {error}"))?;
                super::background_requests::fetch_all_mcp_server_statuses(
                    request_handle,
                    McpServerStatusDetail::ToolsAndAuthOnly,
                    thread_id,
                )
                .await
                .map_err(|error| error.to_string())
            }
            .await;
            app_event_tx.send(AppEvent::McpOauthRefreshFinished {
                operation_id,
                result: McpOauthRefreshResult::new(result),
            });
        });
    }

    pub(super) fn handle_mcp_oauth_refresh_finished(
        &mut self,
        operation_id: String,
        result: McpOauthRefreshResult,
    ) {
        let Some(pending) = self.pending_mcp_oauth.as_ref() else {
            tracing::debug!(%operation_id, "ignored MCP OAuth refresh result without a pending operation");
            return;
        };
        if pending.operation_id != operation_id || pending.phase != PendingMcpOauthPhase::Refreshing
        {
            tracing::debug!(%operation_id, "ignored stale MCP OAuth refresh result");
            return;
        }

        let server = pending.server.clone();
        match result.into_result() {
            Ok(statuses) => {
                if let Some(fresh_server) = statuses
                    .iter()
                    .find(|status| status.name == server.name)
                    .cloned()
                {
                    if !self
                        .chat_widget
                        .finish_mcp_oauth_refresh(statuses, fresh_server)
                    {
                        self.chat_widget.add_info_message(
                            format!("Authenticated MCP server '{}'.", server.name),
                            /*hint*/ None,
                        );
                    }
                } else {
                    self.report_mcp_oauth_reconnect_failure(
                        server,
                        "the server was absent from the refreshed MCP inventory",
                    );
                }
            }
            Err(error) => self.report_mcp_oauth_reconnect_failure(server, &error),
        }
        self.pending_mcp_oauth = None;
    }

    fn report_mcp_oauth_reconnect_failure(&mut self, server: McpServerStatus, error: &str) {
        let error = sanitize_mcp_oauth_message(error);
        let message = format!(
            "Authentication succeeded, but reconnection failed. Restart Codex if MCP server '{}' does not reconnect. Details: {error}",
            server.name
        );
        if !self
            .chat_widget
            .show_mcp_oauth_refresh_error(server, message.clone())
        {
            self.chat_widget.add_error_message(message);
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

fn mcp_oauth_refresh_request(operation_id: &str) -> ClientRequest {
    ClientRequest::McpServerRefresh {
        request_id: RequestId::String(format!("{operation_id}-refresh")),
        params: None,
    }
}

#[cfg(test)]
#[path = "mcp_oauth_tests.rs"]
mod tests;
