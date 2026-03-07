use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

static REQUEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn next_request_id() -> usize {
    REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub type CapturedApiKey = Arc<Mutex<Option<String>>>;
pub type CapturedHeaders = Arc<Mutex<Vec<(String, String)>>>;

#[derive(Debug, Clone, Serialize)]
pub struct InterceptedRequest {
    pub id: usize,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub model: String,
    pub system_prompt: Option<String>,
    pub prompt_text: String,
    pub response_text: String,
    pub status: RequestStatus,
    pub conversation_id: String,
    pub message_count: usize,
    pub is_tool_loop: bool,
    pub is_user_initiated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_messages: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum RequestStatus {
    Pending,
    Streaming,
    Complete,
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
pub enum ProxyEvent {
    NewRequest {
        id: usize,
        timestamp: DateTime<Utc>,
        method: String,
        path: String,
        model: String,
        system_prompt: Option<String>,
        prompt_text: String,
        conversation_id: String,
        message_count: usize,
        is_tool_loop: bool,
        is_user_initiated: bool,
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

pub struct AppState {
    pub requests: Vec<InterceptedRequest>,
    pub selected_index: usize,
    pub auto_select_latest: bool,
    pub response_scroll: u16,
    pub prompt_scroll: u16,
    pub collapsed_conversations: HashSet<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            selected_index: 0,
            auto_select_latest: true,
            response_scroll: 0,
            prompt_scroll: 0,
            collapsed_conversations: HashSet::new(),
        }
    }

    pub fn apply_event(&mut self, event: ProxyEvent) {
        match event {
            ProxyEvent::NewRequest {
                id,
                timestamp,
                method,
                path,
                model,
                system_prompt,
                prompt_text,
                conversation_id,
                message_count,
                is_tool_loop,
                is_user_initiated,
                raw_messages,
            } => {
                self.requests.push(InterceptedRequest {
                    id,
                    timestamp,
                    method,
                    path,
                    model,
                    system_prompt,
                    prompt_text,
                    response_text: String::new(),
                    status: RequestStatus::Pending,
                    conversation_id,
                    message_count,
                    is_tool_loop,
                    is_user_initiated,
                    raw_messages,
                });
                if self.auto_select_latest {
                    self.selected_index = self.requests.len().saturating_sub(1);
                    self.response_scroll = 0;
                    self.prompt_scroll = 0;
                }
            }
            ProxyEvent::ResponseDelta { id, text } => {
                if let Some(req) = self.requests.iter_mut().find(|r| r.id == id) {
                    req.status = RequestStatus::Streaming;
                    req.response_text.push_str(&text);
                }
                if self.auto_select_latest {
                    if let Some(idx) = self.requests.iter().position(|r| r.id == id) {
                        self.selected_index = idx;
                    }
                }
            }
            ProxyEvent::ResponseComplete { id } => {
                if let Some(req) = self.requests.iter_mut().find(|r| r.id == id) {
                    req.status = RequestStatus::Complete;
                }
            }
            ProxyEvent::ResponseError { id, error } => {
                if let Some(req) = self.requests.iter_mut().find(|r| r.id == id) {
                    req.status = RequestStatus::Error(error);
                }
            }
        }
    }

    pub fn selected_request(&self) -> Option<&InterceptedRequest> {
        self.requests.get(self.selected_index)
    }

    pub fn scroll_response_down(&mut self, amount: u16) {
        self.response_scroll = self.response_scroll.saturating_add(amount);
    }

    pub fn scroll_response_up(&mut self, amount: u16) {
        self.response_scroll = self.response_scroll.saturating_sub(amount);
    }
}

/// Extract the user prompt text from a parsed Anthropic API request body.
pub fn extract_prompt(body: &Value) -> String {
    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in messages.iter().rev() {
            if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                return extract_content(msg);
            }
        }
    }
    String::new()
}

/// Extract the system prompt from a parsed Anthropic API request body.
pub fn extract_system(body: &Value) -> Option<String> {
    match body.get("system") {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Array(arr)) => {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|block| {
                    if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                        block.get("text").and_then(|t| t.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        _ => None,
    }
}

/// Extract the model name from a parsed Anthropic API request body.
pub fn extract_model(body: &Value) -> String {
    body.get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Hash the first message in the messages array to produce a stable conversation ID.
pub fn extract_conversation_id(body: &Value) -> String {
    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        if let Some(first) = messages.first() {
            let repr = first.to_string();
            let mut hasher = DefaultHasher::new();
            repr.hash(&mut hasher);
            return format!("{:016x}", hasher.finish());
        }
    }
    "0000000000000000".to_string()
}

/// Return the number of messages in the request.
pub fn extract_message_count(body: &Value) -> usize {
    body.get("messages")
        .and_then(|m| m.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

/// Detect if the last two messages form a tool-use loop (assistant tool_use + user tool_result).
pub fn detect_tool_loop(body: &Value) -> bool {
    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        if messages.len() >= 2 {
            let second_last = &messages[messages.len() - 2];
            let last = &messages[messages.len() - 1];

            let is_assistant_tool_use = second_last
                .get("role")
                .and_then(|r| r.as_str())
                == Some("assistant")
                && has_content_type(second_last, "tool_use");

            let is_user_tool_result = last
                .get("role")
                .and_then(|r| r.as_str())
                == Some("user")
                && has_content_type(last, "tool_result");

            return is_assistant_tool_use && is_user_tool_result;
        }
    }
    false
}

fn has_content_type(msg: &Value, content_type: &str) -> bool {
    if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
        content.iter().any(|block| {
            block.get("type").and_then(|t| t.as_str()) == Some(content_type)
        })
    } else {
        false
    }
}

fn extract_content(msg: &Value) -> String {
    match msg.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|block| {
                    if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                        block.get("text").and_then(|t| t.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
                .collect();
            parts.join("\n")
        }
        _ => String::new(),
    }
}
