use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequestStatus {
    Pending,
    Streaming,
    Complete,
    Error(String),
}

#[derive(Debug)]
pub enum ProxyEvent {
    NewRequest {
        id: usize,
        timestamp: DateTime<Utc>,
        method: String,
        path: String,
        model: String,
        system_prompt: Option<String>,
        prompt_text: String,
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
}

impl AppState {
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            selected_index: 0,
            auto_select_latest: true,
            response_scroll: 0,
            prompt_scroll: 0,
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

    pub fn select_next(&mut self) {
        if !self.requests.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.requests.len() - 1);
            self.auto_select_latest = self.selected_index == self.requests.len() - 1;
            self.response_scroll = 0;
            self.prompt_scroll = 0;
        }
    }

    pub fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
        self.auto_select_latest = false;
        self.response_scroll = 0;
        self.prompt_scroll = 0;
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
