use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_rustls::TlsConnector;
use tokio_stream::wrappers::ReceiverStream;

use crate::sse::{SseEvent, SseParser};
use crate::state::{
    detect_tool_loop, extract_conversation_id, extract_message_count, extract_model,
    extract_prompt, extract_system, next_request_id, CapturedApiKey, CapturedHeaders, ProxyEvent,
};
use crate::tls::CertAuthority;

/// Headers to skip when capturing from proxied traffic (we reconstruct these ourselves)
const SKIP_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "content-type",
    "accept-encoding",
    "connection",
    "user-agent",
];

fn empty_body() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

fn full_body(data: Bytes) -> BoxBody<Bytes, hyper::Error> {
    Full::new(data)
        .map_err(|never| match never {})
        .boxed()
}

pub async fn run_proxy(
    listen_addr: SocketAddr,
    ca: Arc<CertAuthority>,
    event_tx: broadcast::Sender<ProxyEvent>,
    api_key_store: CapturedApiKey,
    captured_headers: CapturedHeaders,
) -> Result<()> {
    let listener = TcpListener::bind(listen_addr).await?;
    tracing::info!("Proxy listening on {}", listen_addr);

    loop {
        let (stream, addr) = listener.accept().await?;
        let ca = ca.clone();
        let event_tx = event_tx.clone();
        let api_key_store = api_key_store.clone();
        let captured_headers = captured_headers.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, addr, ca, event_tx, api_key_store, captured_headers).await {
                tracing::error!("Connection error from {}: {:#}", addr, e);
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    ca: Arc<CertAuthority>,
    event_tx: broadcast::Sender<ProxyEvent>,
    api_key_store: CapturedApiKey,
    captured_headers: CapturedHeaders,
) -> Result<()> {
    let ca = ca.clone();
    let event_tx = event_tx.clone();

    let service = service_fn(move |req: Request<Incoming>| {
        let ca = ca.clone();
        let event_tx = event_tx.clone();
        let api_key_store = api_key_store.clone();
        let captured_headers = captured_headers.clone();
        async move {
            if req.method() == Method::CONNECT {
                handle_connect(req, ca, event_tx, api_key_store, captured_headers).await
            } else {
                Ok(Response::new(empty_body()))
            }
        }
    });

    http1::Builder::new()
        .preserve_header_case(true)
        .title_case_headers(true)
        .serve_connection(TokioIo::new(stream), service)
        .with_upgrades()
        .await
        .map_err(|e| anyhow::anyhow!("HTTP serve error from {}: {}", addr, e))?;

    Ok(())
}

async fn handle_connect(
    req: Request<Incoming>,
    ca: Arc<CertAuthority>,
    event_tx: broadcast::Sender<ProxyEvent>,
    api_key_store: CapturedApiKey,
    captured_headers: CapturedHeaders,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let host = req
        .uri()
        .authority()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let domain = host
        .split(':')
        .next()
        .unwrap_or(&host)
        .to_string();

    tracing::info!("CONNECT to {}", host);

    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                if let Err(e) = mitm_tunnel(upgraded, domain, ca, event_tx, api_key_store, captured_headers).await {
                    tracing::error!("MITM tunnel error: {:#}", e);
                }
            }
            Err(e) => {
                tracing::error!("Upgrade error: {}", e);
            }
        }
    });

    Ok(Response::new(empty_body()))
}

async fn mitm_tunnel(
    upgraded: hyper::upgrade::Upgraded,
    domain: String,
    ca: Arc<CertAuthority>,
    event_tx: broadcast::Sender<ProxyEvent>,
    api_key_store: CapturedApiKey,
    captured_headers: CapturedHeaders,
) -> Result<()> {
    let server_config = ca.server_config_for_domain(&domain).await?;
    let tls_acceptor = tokio_rustls::TlsAcceptor::from(server_config);

    let client_tls = tls_acceptor.accept(TokioIo::new(upgraded)).await?;

    let domain = domain.clone();
    let event_tx = event_tx.clone();

    let service = service_fn(move |req: Request<Incoming>| {
        let domain = domain.clone();
        let event_tx = event_tx.clone();
        let api_key_store = api_key_store.clone();
        let captured_headers = captured_headers.clone();
        async move { forward_and_intercept(req, &domain, event_tx, api_key_store, captured_headers).await }
    });

    http1::Builder::new()
        .preserve_header_case(true)
        .serve_connection(TokioIo::new(client_tls), service)
        .await
        .map_err(|e| anyhow::anyhow!("Inner HTTP error: {}", e))?;

    Ok(())
}

async fn forward_and_intercept(
    req: Request<Incoming>,
    domain: &str,
    event_tx: broadcast::Sender<ProxyEvent>,
    api_key_store: CapturedApiKey,
    captured_headers: CapturedHeaders,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    match forward_and_intercept_inner(req, domain, event_tx, api_key_store, captured_headers).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            tracing::error!("Forward error: {:#}", e);
            let body = full_body(Bytes::from(format!("Proxy error: {e}")));
            Ok(Response::builder()
                .status(502)
                .body(body)
                .unwrap())
        }
    }
}

async fn forward_and_intercept_inner(
    req: Request<Incoming>,
    domain: &str,
    event_tx: broadcast::Sender<ProxyEvent>,
    api_key_store: CapturedApiKey,
    captured_headers: CapturedHeaders,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>> {
    let (parts, body) = req.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read request body: {}", e))?
        .to_bytes();

    let request_id = next_request_id();
    let path = parts.uri.path().to_string();
    let method = parts.method.to_string();

    // Parse body and extract prompt info for API requests
    let is_messages_api = path.contains("/v1/messages");
    if is_messages_api && !body_bytes.is_empty() {
        if let Ok(body_json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
            let model = extract_model(&body_json);
            let prompt_text = extract_prompt(&body_json);
            let system_prompt = extract_system(&body_json);
            let conversation_id = extract_conversation_id(&body_json);
            let message_count = extract_message_count(&body_json);
            let is_tool_loop = detect_tool_loop(&body_json);
            let raw_messages = body_json.get("messages").cloned();

            // Capture API key and headers from proxied traffic
            let skip: HashSet<&str> = SKIP_HEADERS.iter().copied().collect();
            let mut hdrs = Vec::new();
            let mut found_api_key = false;
            for (name, value) in &parts.headers {
                let name_lower = name.as_str().to_lowercase();
                if skip.contains(name_lower.as_str()) {
                    continue;
                }
                if let Ok(v) = value.to_str() {
                    // Capture API key from x-api-key or Authorization: Bearer
                    if name_lower == "x-api-key" {
                        let mut store = api_key_store.lock().unwrap();
                        *store = Some(v.to_string());
                        found_api_key = true;
                        tracing::info!("Captured API key from x-api-key header");
                    } else if name_lower == "authorization" {
                        if let Some(token) = v.strip_prefix("Bearer ") {
                            let mut store = api_key_store.lock().unwrap();
                            if store.is_none() {
                                *store = Some(token.to_string());
                                found_api_key = true;
                                tracing::info!("Captured API key from Authorization Bearer header");
                            }
                        }
                    }
                    hdrs.push((name_lower, v.to_string()));
                }
            }
            if !found_api_key {
                tracing::warn!(
                    "No API key found in headers for {} {}. Headers present: {:?}",
                    method, path,
                    parts.headers.keys().map(|k| k.as_str()).collect::<Vec<_>>()
                );
            }
            if !hdrs.is_empty() {
                let mut store = captured_headers.lock().unwrap();
                *store = hdrs;
            }

            let _ = event_tx.send(ProxyEvent::NewRequest {
                id: request_id,
                timestamp: chrono::Utc::now(),
                method: method.clone(),
                path: path.clone(),
                model,
                system_prompt,
                prompt_text,
                conversation_id,
                message_count,
                is_tool_loop,
                is_user_initiated: false,
                raw_messages,
            });
        }
    }

    // Connect to upstream
    let upstream_tls = connect_upstream(domain).await?;

    let (mut sender, conn) =
        hyper::client::conn::http1::handshake(TokioIo::new(upstream_tls)).await?;

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::error!("Upstream connection error: {}", e);
        }
    });

    // Rebuild the request for upstream
    let mut upstream_req_builder = Request::builder()
        .method(parts.method)
        .uri(parts.uri)
        .version(parts.version);

    for (name, value) in &parts.headers {
        // Strip Accept-Encoding so upstream sends uncompressed SSE we can parse
        if name.as_str().eq_ignore_ascii_case("accept-encoding") {
            continue;
        }
        upstream_req_builder = upstream_req_builder.header(name, value);
    }

    let upstream_req = upstream_req_builder
        .body(full_body(body_bytes))
        .map_err(|e| anyhow::anyhow!("Failed to build upstream request: {}", e))?;

    let upstream_res = sender
        .send_request(upstream_req)
        .await
        .map_err(|e| anyhow::anyhow!("Upstream request failed: {}", e))?;

    // Check if this is an SSE response
    let content_type = upstream_res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("none")
        .to_string();
    let is_sse = content_type.contains("text/event-stream");

    tracing::info!(
        "Response for {} {} — content-type: {}, is_sse: {}, is_messages_api: {}",
        method, path, content_type, is_sse, is_messages_api
    );

    if is_sse && is_messages_api {
        tee_sse_response(upstream_res, request_id, event_tx).await
    } else {
        // Forward non-SSE response as-is
        let (parts, body) = upstream_res.into_parts();
        let body_bytes = body
            .collect()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read upstream response: {}", e))?
            .to_bytes();
        Ok(Response::from_parts(parts, full_body(body_bytes)))
    }
}

pub(crate) async fn connect_upstream(
    domain: &str,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(tls_config));

    let addr = format!("{}:443", domain);
    let tcp = TcpStream::connect(&addr).await?;

    let server_name = ServerName::try_from(domain.to_string())
        .map_err(|e| anyhow::anyhow!("Invalid server name '{}': {}", domain, e))?;

    let tls_stream = connector.connect(server_name, tcp).await?;
    Ok(tls_stream)
}

async fn tee_sse_response(
    upstream_res: Response<Incoming>,
    request_id: usize,
    event_tx: broadcast::Sender<ProxyEvent>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>> {
    let (parts, mut body) = upstream_res.into_parts();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Frame<Bytes>, hyper::Error>>(64);

    tokio::spawn(async move {
        let mut parser = SseParser::new();
        tracing::info!("SSE tee started for req {}", request_id);

        while let Some(frame_result) = body.frame().await {
            match frame_result {
                Ok(frame) => {
                    if let Some(data) = frame.data_ref() {
                        let preview = String::from_utf8_lossy(&data[..data.len().min(500)]);
                        tracing::info!("SSE chunk for req {}: {} bytes — {:?}", request_id, data.len(), preview);
                        let events = parser.feed(data);
                        for event in &events {
                            tracing::info!("SSE event for req {}: {:?}", request_id, event);
                        }
                        for event in events {
                            match event {
                                SseEvent::ContentBlockDelta { text } => {
                                    let _ = event_tx.send(ProxyEvent::ResponseDelta {
                                        id: request_id,
                                        text,
                                    });
                                }
                                SseEvent::MessageStop => {
                                    let _ = event_tx.send(ProxyEvent::ResponseComplete {
                                        id: request_id,
                                    });
                                }
                                SseEvent::Other => {}
                            }
                        }
                    }
                    if tx.send(Ok(frame)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(ProxyEvent::ResponseError {
                        id: request_id,
                        error: e.to_string(),
                    });
                    let _ = tx.send(Err(e)).await;
                    break;
                }
            }
        }
    });

    let stream = ReceiverStream::new(rx);
    let stream_body = StreamBody::new(stream);
    Ok(Response::from_parts(parts, stream_body.boxed()))
}
