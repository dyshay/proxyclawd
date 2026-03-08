use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::types::{InterceptedRequest, ProxyEvent, RequestStatus, WsMessage};

pub struct ProxyClient {
    base_url: String,
    pub requests: Arc<RwLock<Vec<InterceptedRequest>>>,
    pub connected: Arc<AtomicBool>,
}

impl ProxyClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            requests: Arc::new(RwLock::new(Vec::new())),
            connected: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start background WebSocket connection with automatic reconnection.
    pub fn connect(&self) {
        let ws_url = self.base_url.replace("http://", "ws://").replace("https://", "wss://");
        let ws_url = format!("{}/ws", ws_url);
        let requests = self.requests.clone();
        let connected = self.connected.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            let mut backoff = Duration::from_secs(1);
            let max_backoff = Duration::from_secs(30);

            loop {
                info!("Connecting to ProxyClawd WebSocket at {}", ws_url);
                match tokio_tungstenite::connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        connected.store(true, Ordering::Relaxed);
                        backoff = Duration::from_secs(1);
                        info!("Connected to ProxyClawd WebSocket");

                        let (_write, mut read) = ws_stream.split();

                        loop {
                            match read.next().await {
                                Some(Ok(msg)) => {
                                    if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                                        match serde_json::from_str::<WsMessage>(&text) {
                                            Ok(WsMessage::Snapshot { requests: snapshot }) => {
                                                let mut reqs = requests.write().await;
                                                *reqs = snapshot;
                                                info!("Received snapshot with {} requests", reqs.len());
                                            }
                                            Ok(WsMessage::Event { event }) => {
                                                let mut reqs = requests.write().await;
                                                apply_event(&mut reqs, event);
                                            }
                                            Err(e) => {
                                                warn!("Failed to parse WebSocket message: {}", e);
                                            }
                                        }
                                    }
                                }
                                Some(Err(e)) => {
                                    warn!("WebSocket error: {}", e);
                                    break;
                                }
                                None => {
                                    info!("WebSocket connection closed");
                                    break;
                                }
                            }
                        }

                        connected.store(false, Ordering::Relaxed);
                    }
                    Err(e) => {
                        warn!("WebSocket connection failed: {}. Trying REST fallback...", e);

                        // REST fallback: try to fetch current state
                        match fetch_requests_rest(&base_url).await {
                            Ok(reqs) => {
                                let mut requests = requests.write().await;
                                *requests = reqs;
                                info!("Loaded {} requests via REST fallback", requests.len());
                            }
                            Err(e) => {
                                error!("REST fallback also failed: {}", e);
                            }
                        }
                    }
                }

                warn!("Reconnecting in {:?}...", backoff);
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        });
    }

    pub async fn get_requests(&self) -> Vec<InterceptedRequest> {
        self.requests.read().await.clone()
    }

    pub async fn get_request(&self, id: usize) -> Option<InterceptedRequest> {
        self.requests.read().await.iter().find(|r| r.id == id).cloned()
    }

    pub async fn send_message(
        &self,
        message: &str,
        continue_conversation: bool,
    ) -> Result<String, String> {
        let url = format!("{}/api/send", self.base_url);
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "message": message,
            "continue_conversation": continue_conversation,
        });

        match client.post(&url).json(&body).send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    Ok(text)
                } else {
                    Err(format!("HTTP {}: {}", status, text))
                }
            }
            Err(e) => Err(format!("Request failed: {}", e)),
        }
    }
}

/// Apply a ProxyEvent to the request list (mirrors AppState::apply_event).
fn apply_event(requests: &mut Vec<InterceptedRequest>, event: ProxyEvent) {
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
            requests.push(InterceptedRequest {
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
        }
        ProxyEvent::ResponseDelta { id, text } => {
            if let Some(req) = requests.iter_mut().find(|r| r.id == id) {
                req.status = RequestStatus::Streaming;
                req.response_text.push_str(&text);
            }
        }
        ProxyEvent::ResponseComplete { id } => {
            if let Some(req) = requests.iter_mut().find(|r| r.id == id) {
                req.status = RequestStatus::Complete;
            }
        }
        ProxyEvent::ResponseError { id, error } => {
            if let Some(req) = requests.iter_mut().find(|r| r.id == id) {
                req.status = RequestStatus::Error(error);
            }
        }
    }
}

/// REST fallback to fetch requests when WebSocket is unavailable.
async fn fetch_requests_rest(base_url: &str) -> Result<Vec<InterceptedRequest>, String> {
    let url = format!("{}/api/requests", base_url);
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("GET {} failed: {}", url, e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    resp.json::<Vec<InterceptedRequest>>()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))
}
