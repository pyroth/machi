//! Tool execution confirmation handlers.
//!
//! This module provides confirmation handlers for different channels:
//! - CLI: Blocking stdin confirmation
//! - Telegram: Inline keyboard buttons

use crate::config::ToolPolicy;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, oneshot};

/// A request for human confirmation before tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationRequest {
    /// Unique ID for this confirmation request.
    pub id: String,
    /// The tool name.
    pub tool_name: String,
    /// The tool arguments as JSON.
    pub arguments: serde_json::Value,
    /// Human-readable description of what the tool will do.
    pub description: String,
    /// Channel this request came from.
    pub channel: String,
    /// Session key for routing the response.
    pub session_key: String,
}

impl ConfirmationRequest {
    /// Create a new confirmation request.
    #[must_use]
    pub fn new(
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
        channel: impl Into<String>,
        session_key: impl Into<String>,
    ) -> Self {
        let tool_name = tool_name.into();
        let description = format!(
            "Tool '{}' wants to execute with arguments:\n```json\n{}\n```",
            tool_name,
            serde_json::to_string_pretty(&arguments).unwrap_or_else(|_| arguments.to_string())
        );
        Self {
            id: crate::util::generate_id("confirm"),
            tool_name,
            arguments,
            description,
            channel: channel.into(),
            session_key: session_key.into(),
        }
    }

    /// Create with a custom description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

/// Response to a tool confirmation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmationResponse {
    /// User approved the tool execution.
    Approved,
    /// User denied the tool execution.
    Denied,
    /// User approved this and all future calls to this tool.
    ApproveAll,
    /// Request timed out.
    Timeout,
}

impl ConfirmationResponse {
    /// Check if the response approves execution.
    #[must_use]
    pub const fn is_approved(&self) -> bool {
        matches!(self, Self::Approved | Self::ApproveAll)
    }

    /// Check if future calls should be auto-approved.
    #[must_use]
    pub const fn should_approve_all(&self) -> bool {
        matches!(self, Self::ApproveAll)
    }
}

/// Handler for tool execution confirmation requests.
#[async_trait]
pub trait ConfirmationHandler: Send + Sync {
    /// Request confirmation for a tool execution.
    ///
    /// Returns the user's decision within the given timeout.
    async fn confirm(
        &self,
        request: &ConfirmationRequest,
        timeout: Duration,
    ) -> ConfirmationResponse;
}

/// Manages pending confirmation requests and their responses.
#[derive(Default)]
pub struct ConfirmationManager {
    /// Pending requests waiting for responses.
    pending: RwLock<HashMap<String, oneshot::Sender<ConfirmationResponse>>>,
    /// Tools that have been auto-approved.
    auto_approved: RwLock<std::collections::HashSet<String>>,
}

impl std::fmt::Debug for ConfirmationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmationManager")
            .finish_non_exhaustive()
    }
}

impl ConfirmationManager {
    /// Create a new confirmation manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pending confirmation request.
    ///
    /// Returns a receiver that will receive the response.
    pub async fn register(&self, request_id: &str) -> oneshot::Receiver<ConfirmationResponse> {
        let (tx, rx) = oneshot::channel();
        self.pending
            .write()
            .await
            .insert(request_id.to_string(), tx);
        rx
    }

    /// Submit a response for a pending request.
    ///
    /// Returns true if the request was found and response was sent.
    pub async fn respond(&self, request_id: &str, response: ConfirmationResponse) -> bool {
        if let Some(tx) = self.pending.write().await.remove(request_id) {
            tx.send(response).is_ok()
        } else {
            false
        }
    }

    /// Check if a tool has been auto-approved.
    pub async fn is_auto_approved(&self, tool_name: &str) -> bool {
        self.auto_approved.read().await.contains(tool_name)
    }

    /// Mark a tool as auto-approved.
    pub async fn mark_auto_approved(&self, tool_name: &str) {
        self.auto_approved
            .write()
            .await
            .insert(tool_name.to_string());
    }

    /// Clear all auto-approved tools.
    pub async fn clear_auto_approved(&self) {
        self.auto_approved.write().await.clear();
    }

    /// Get the effective policy for a tool.
    pub async fn get_effective_policy(
        &self,
        tool_name: &str,
        configured_policy: ToolPolicy,
    ) -> ToolPolicy {
        if self.is_auto_approved(tool_name).await {
            ToolPolicy::Auto
        } else {
            configured_policy
        }
    }
}

/// CLI confirmation handler using stdin.
#[derive(Debug, Clone, Copy, Default)]
pub struct CliConfirmationHandler;

#[async_trait]
impl ConfirmationHandler for CliConfirmationHandler {
    async fn confirm(
        &self,
        request: &ConfirmationRequest,
        timeout: Duration,
    ) -> ConfirmationResponse {
        use std::io::{self, Write};

        println!("\n{}", "=".repeat(60));
        println!("🔐 TOOL CONFIRMATION REQUIRED");
        println!("{}", "=".repeat(60));
        println!("{}", request.description);
        println!("{}", "-".repeat(60));
        println!("Options: [y]es / [n]o / [a]ll (approve all future calls)");
        print!("> ");
        let _ = io::stdout().flush();

        // Use tokio to read with timeout
        let result = tokio::time::timeout(timeout, async {
            tokio::task::spawn_blocking(|| {
                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_ok() {
                    Some(input)
                } else {
                    None
                }
            })
            .await
            .ok()
            .flatten()
        })
        .await;

        match result {
            Ok(Some(input)) => match input.trim().to_lowercase().as_str() {
                "y" | "yes" => {
                    println!("✅ Approved");
                    ConfirmationResponse::Approved
                }
                "a" | "all" => {
                    println!("✅ Approved (all future calls)");
                    ConfirmationResponse::ApproveAll
                }
                _ => {
                    println!("❌ Denied");
                    ConfirmationResponse::Denied
                }
            },
            _ => {
                println!("⏰ Timeout - denied by default");
                ConfirmationResponse::Timeout
            }
        }
    }
}

/// Telegram confirmation handler using inline keyboard buttons.
#[derive(Debug, Clone)]
pub struct TelegramConfirmationHandler {
    manager: Arc<ConfirmationManager>,
}

impl TelegramConfirmationHandler {
    /// Create a new Telegram confirmation handler.
    #[must_use]
    pub fn new(manager: Arc<ConfirmationManager>) -> Self {
        Self { manager }
    }

    /// Get the confirmation manager.
    #[must_use]
    pub fn manager(&self) -> &Arc<ConfirmationManager> {
        &self.manager
    }

    /// Build inline keyboard for confirmation.
    #[must_use]
    pub fn build_keyboard(request_id: &str) -> Vec<Vec<(String, String)>> {
        vec![vec![
            ("✅ Yes".to_string(), format!("confirm:{}:y", request_id)),
            ("❌ No".to_string(), format!("confirm:{}:n", request_id)),
            ("✅ All".to_string(), format!("confirm:{}:a", request_id)),
        ]]
    }

    /// Parse callback data from button press.
    ///
    /// Returns (request_id, response) if valid.
    #[must_use]
    pub fn parse_callback(data: &str) -> Option<(String, ConfirmationResponse)> {
        let parts: Vec<&str> = data.split(':').collect();
        if parts.len() == 3 && parts[0] == "confirm" {
            let request_id = parts[1].to_string();
            let response = match parts[2] {
                "y" => ConfirmationResponse::Approved,
                "a" => ConfirmationResponse::ApproveAll,
                _ => ConfirmationResponse::Denied,
            };
            Some((request_id, response))
        } else {
            None
        }
    }
}

#[async_trait]
impl ConfirmationHandler for TelegramConfirmationHandler {
    async fn confirm(
        &self,
        request: &ConfirmationRequest,
        timeout: Duration,
    ) -> ConfirmationResponse {
        // Register the request and wait for response
        let rx = self.manager.register(&request.id).await;

        // Wait for response with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => ConfirmationResponse::Timeout,
            Err(_) => ConfirmationResponse::Timeout,
        }
    }
}

/// Auto-approve handler that approves all requests.
#[derive(Debug, Clone, Copy, Default)]
pub struct AutoApproveHandler;

#[async_trait]
impl ConfirmationHandler for AutoApproveHandler {
    async fn confirm(
        &self,
        _request: &ConfirmationRequest,
        _timeout: Duration,
    ) -> ConfirmationResponse {
        ConfirmationResponse::Approved
    }
}

/// Always-deny handler for testing or strict policies.
#[derive(Debug, Clone, Copy, Default)]
pub struct AlwaysDenyHandler;

#[async_trait]
impl ConfirmationHandler for AlwaysDenyHandler {
    async fn confirm(
        &self,
        _request: &ConfirmationRequest,
        _timeout: Duration,
    ) -> ConfirmationResponse {
        ConfirmationResponse::Denied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirmation_request() {
        let req = ConfirmationRequest::new(
            "exec",
            serde_json::json!({"command": "ls"}),
            "cli",
            "session_1",
        );
        assert_eq!(req.tool_name, "exec");
        assert!(req.description.contains("exec"));
    }

    #[test]
    fn test_confirmation_response() {
        assert!(ConfirmationResponse::Approved.is_approved());
        assert!(ConfirmationResponse::ApproveAll.is_approved());
        assert!(!ConfirmationResponse::Denied.is_approved());
        assert!(!ConfirmationResponse::Timeout.is_approved());

        assert!(ConfirmationResponse::ApproveAll.should_approve_all());
        assert!(!ConfirmationResponse::Approved.should_approve_all());
    }

    #[test]
    fn test_telegram_keyboard() {
        let keyboard = TelegramConfirmationHandler::build_keyboard("req_123");
        assert_eq!(keyboard.len(), 1);
        assert_eq!(keyboard[0].len(), 3);
    }

    #[test]
    fn test_telegram_callback_parse() {
        let (id, resp) = TelegramConfirmationHandler::parse_callback("confirm:req_123:y").unwrap();
        assert_eq!(id, "req_123");
        assert_eq!(resp, ConfirmationResponse::Approved);

        let (id, resp) = TelegramConfirmationHandler::parse_callback("confirm:req_456:a").unwrap();
        assert_eq!(id, "req_456");
        assert_eq!(resp, ConfirmationResponse::ApproveAll);

        assert!(TelegramConfirmationHandler::parse_callback("invalid").is_none());
    }

    #[tokio::test]
    async fn test_confirmation_manager() {
        let manager = ConfirmationManager::new();

        // Test auto-approval
        assert!(!manager.is_auto_approved("exec").await);
        manager.mark_auto_approved("exec").await;
        assert!(manager.is_auto_approved("exec").await);

        // Test effective policy
        let policy = manager
            .get_effective_policy("exec", ToolPolicy::RequireConfirmation)
            .await;
        assert_eq!(policy, ToolPolicy::Auto);

        let policy = manager
            .get_effective_policy("other", ToolPolicy::RequireConfirmation)
            .await;
        assert_eq!(policy, ToolPolicy::RequireConfirmation);
    }
}
