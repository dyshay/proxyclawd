use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use tokio::sync::{broadcast, Mutex};
use tower_http::services::ServeDir;

use crate::state::{AppState, InterceptedRequest, ProxyEvent};

struct WebState {
    requests: Mutex<Vec<InterceptedRequest>>,
    event_tx: broadcast::Sender<ProxyEvent>,
}

pub async fn run_web_server(
    port: u16,
    event_tx: broadcast::Sender<ProxyEvent>,
) -> anyhow::Result<()> {
    let mut event_rx = event_tx.subscribe();

    let state = Arc::new(WebState {
        requests: Mutex::new(Vec::new()),
        event_tx: event_tx.clone(),
    });

    // Spawn task to keep shared request list updated from events
    let state_clone = state.clone();
    tokio::spawn(async move {
        let mut app_state = AppState::new();
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    app_state.apply_event(event);
                    let mut requests = state_clone.requests.lock().await;
                    *requests = app_state.requests.clone();
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Web state updater lagged by {} events", n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let frontend_dir = std::env::current_dir()
        .unwrap_or_default()
        .join("frontend")
        .join("dist");

    let app = Router::new()
        .route("/api/requests", get(get_requests))
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new(&frontend_dir).append_index_html_on_directories(true))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    tracing::info!("Web server listening on port {}", port);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<WebState>) {
    // Subscribe BEFORE snapshot to avoid missing events in the gap
    let mut rx = state.event_tx.subscribe();

    // Send current snapshot
    let requests = state.requests.lock().await;
    let snapshot = serde_json::json!({
        "type": "snapshot",
        "requests": *requests,
    });
    if socket
        .send(Message::Text(snapshot.to_string().into()))
        .await
        .is_err()
    {
        return;
    }
    drop(requests);
    loop {
        match rx.recv().await {
            Ok(event) => {
                let json = serde_json::json!({
                    "type": "event",
                    "event": event,
                });
                if socket
                    .send(Message::Text(json.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

async fn get_requests(
    State(state): State<Arc<WebState>>,
) -> Json<Vec<InterceptedRequest>> {
    let requests = state.requests.lock().await;
    Json(requests.clone())
}
