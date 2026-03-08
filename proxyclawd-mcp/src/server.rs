use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, RoleServer};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::client::ProxyClient;
use crate::types::RequestStatus;

#[derive(Clone)]
pub struct ProxyMcpServer {
    client: Arc<ProxyClient>,
    tool_router: ToolRouter<Self>,
}

// --- Tool parameter types ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRequestsParams {
    /// Maximum number of requests to return
    pub limit: Option<usize>,
    /// Filter by status: "pending", "streaming", "complete", "error"
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRequestParams {
    /// Request ID
    pub id: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendMessageParams {
    /// Message text to send via Claude subprocess
    pub message: String,
    /// Whether to continue an existing conversation
    pub continue_conversation: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRecentEventsParams {
    /// Only return requests with ID greater than this value
    pub since_id: Option<usize>,
    /// Maximum number of requests to return
    pub count: Option<usize>,
}

// --- Tool implementations ---

#[tool_router]
impl ProxyMcpServer {
    pub fn new(client: Arc<ProxyClient>) -> Self {
        Self {
            client,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "List intercepted API requests from ProxyClawd proxy")]
    async fn list_requests(
        &self,
        params: Parameters<ListRequestsParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = params.0;
        let requests = self.client.get_requests().await;
        let limit = params.limit.unwrap_or(50);

        let filtered: Vec<_> = requests
            .iter()
            .filter(|r| {
                if let Some(ref status_filter) = params.status {
                    match status_filter.to_lowercase().as_str() {
                        "pending" => r.status == RequestStatus::Pending,
                        "streaming" => r.status == RequestStatus::Streaming,
                        "complete" => r.status == RequestStatus::Complete,
                        "error" => matches!(r.status, RequestStatus::Error(_)),
                        _ => true,
                    }
                } else {
                    true
                }
            })
            .rev()
            .take(limit)
            .collect();

        let summary: Vec<serde_json::Value> = filtered
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "timestamp": r.timestamp.to_rfc3339(),
                    "model": r.model,
                    "status": r.status,
                    "conversation_id": r.conversation_id,
                    "is_tool_loop": r.is_tool_loop,
                    "is_user_initiated": r.is_user_initiated,
                    "prompt_preview": truncate(&r.prompt_text, 100),
                    "response_preview": truncate(&r.response_text, 100),
                })
            })
            .collect();

        let text = serde_json::to_string_pretty(&summary).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(description = "Get full details of a specific intercepted request by ID")]
    async fn get_request(
        &self,
        params: Parameters<GetRequestParams>,
    ) -> Result<CallToolResult, McpError> {
        let id = params.0.id;
        match self.client.get_request(id).await {
            Some(req) => {
                let text = serde_json::to_string_pretty(&req).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            None => Ok(CallToolResult::success(vec![Content::text(format!(
                "Request with ID {} not found",
                id
            ))])),
        }
    }

    #[tool(description = "Send a message through the ProxyClawd Claude subprocess")]
    async fn send_message(
        &self,
        params: Parameters<SendMessageParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = params.0;
        let cont = params.continue_conversation.unwrap_or(false);
        match self.client.send_message(&params.message, cont).await {
            Ok(response) => Ok(CallToolResult::success(vec![Content::text(response)])),
            Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Error sending message: {}",
                e
            ))])),
        }
    }

    #[tool(description = "Group intercepted requests by conversation ID")]
    async fn get_conversations(&self) -> Result<CallToolResult, McpError> {
        let requests = self.client.get_requests().await;
        let mut conversations: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

        for r in &requests {
            conversations
                .entry(r.conversation_id.clone())
                .or_default()
                .push(serde_json::json!({
                    "id": r.id,
                    "timestamp": r.timestamp.to_rfc3339(),
                    "model": r.model,
                    "status": r.status,
                    "is_tool_loop": r.is_tool_loop,
                    "is_user_initiated": r.is_user_initiated,
                    "message_count": r.message_count,
                    "prompt_preview": truncate(&r.prompt_text, 80),
                }));
        }

        let text = serde_json::to_string_pretty(&conversations).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(description = "Get recent intercepted requests, optionally since a given request ID")]
    async fn get_recent_events(
        &self,
        params: Parameters<GetRecentEventsParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = params.0;
        let requests = self.client.get_requests().await;
        let count = params.count.unwrap_or(10);

        let filtered: Vec<_> = requests
            .iter()
            .filter(|r| {
                if let Some(since) = params.since_id {
                    r.id > since
                } else {
                    true
                }
            })
            .rev()
            .take(count)
            .collect();

        let summary: Vec<serde_json::Value> = filtered
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "timestamp": r.timestamp.to_rfc3339(),
                    "model": r.model,
                    "status": r.status,
                    "conversation_id": r.conversation_id,
                    "is_tool_loop": r.is_tool_loop,
                    "prompt_preview": truncate(&r.prompt_text, 100),
                    "response_preview": truncate(&r.response_text, 100),
                })
            })
            .collect();

        let text = serde_json::to_string_pretty(&summary).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

// --- ServerHandler implementation ---

#[tool_handler]
impl rmcp::handler::server::ServerHandler for ProxyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_instructions(
            "ProxyClawd MCP server: monitor and control a ProxyClawd MITM proxy instance. \
             Use list_requests to see intercepted API calls, get_request for details, \
             send_message to interact via Claude subprocess, and get_conversations \
             to see grouped conversation threads.",
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = vec![
            RawResource::new("proxy://requests", "Intercepted Requests")
                .with_description("Current list of all intercepted API requests")
                .with_mime_type("application/json")
                .no_annotation(),
            RawResource::new("proxy://status", "Proxy Status")
                .with_description("Connection status and request counters")
                .with_mime_type("application/json")
                .no_annotation(),
        ];
        Ok(ListResourcesResult::with_all_items(resources))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match request.uri.as_str() {
            "proxy://requests" => {
                let requests = self.client.get_requests().await;
                let json = serde_json::to_string_pretty(&requests).unwrap_or_default();
                Ok(ReadResourceResult::new(vec![ResourceContents::text(
                    json,
                    "proxy://requests",
                )]))
            }
            "proxy://status" => {
                let requests = self.client.get_requests().await;
                let connected = self.client.connected.load(Ordering::Relaxed);
                let total = requests.len();
                let pending = requests
                    .iter()
                    .filter(|r| r.status == RequestStatus::Pending)
                    .count();
                let streaming = requests
                    .iter()
                    .filter(|r| r.status == RequestStatus::Streaming)
                    .count();
                let complete = requests
                    .iter()
                    .filter(|r| r.status == RequestStatus::Complete)
                    .count();
                let errors = requests
                    .iter()
                    .filter(|r| matches!(r.status, RequestStatus::Error(_)))
                    .count();

                let status = serde_json::json!({
                    "connected": connected,
                    "total_requests": total,
                    "pending": pending,
                    "streaming": streaming,
                    "complete": complete,
                    "errors": errors,
                });
                let json = serde_json::to_string_pretty(&status).unwrap_or_default();
                Ok(ReadResourceResult::new(vec![ResourceContents::text(
                    json,
                    "proxy://status",
                )]))
            }
            _ => Err(McpError::resource_not_found(
                format!("Unknown resource: {}", request.uri),
                None,
            )),
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
