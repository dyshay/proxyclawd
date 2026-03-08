use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterceptedRequest {
    pub id: usize,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub model: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub prompt_text: String,
    #[serde(default)]
    pub response_text: String,
    pub status: RequestStatus,
    #[serde(default)]
    pub conversation_id: String,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default)]
    pub is_tool_loop: bool,
    #[serde(default)]
    pub is_user_initiated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_messages: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RequestStatus {
    Pending,
    Streaming,
    Complete,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProxyEvent {
    NewRequest {
        id: usize,
        timestamp: DateTime<Utc>,
        method: String,
        path: String,
        model: String,
        #[serde(default)]
        system_prompt: Option<String>,
        prompt_text: String,
        #[serde(default)]
        conversation_id: String,
        #[serde(default)]
        message_count: usize,
        #[serde(default)]
        is_tool_loop: bool,
        #[serde(default)]
        is_user_initiated: bool,
        #[serde(default)]
        raw_messages: Option<Value>,
    },
    ResponseDelta {
        id: usize,
        text: String,
    },
    ResponseComplete {
        id: usize,
    },
    ResponseError {
        id: usize,
        error: String,
    },
}

/// WebSocket message envelope from the ProxyClawd web server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    #[serde(rename = "snapshot")]
    Snapshot { requests: Vec<InterceptedRequest> },
    #[serde(rename = "event")]
    Event { event: ProxyEvent },
}
