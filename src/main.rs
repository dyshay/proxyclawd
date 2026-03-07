use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Parser;
use tokio::sync::broadcast;

mod claude_subprocess;
mod proxy;
mod sse;
mod state;
mod tls;
mod tui;
mod web;

use state::{CapturedApiKey, CapturedHeaders};

#[derive(Parser)]
#[command(name = "proxyclawd", about = "MITM Proxy for Claude Code")]
struct Cli {
    /// Enable web UI server
    #[arg(long)]
    web: bool,

    /// Web UI port
    #[arg(long, default_value = "3000")]
    web_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize file-based logging (TUI owns stdout)
    let log_dir = std::env::current_dir()?.join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let file_appender = tracing_appender::rolling::daily(&log_dir, "proxy.log");
    tracing_subscriber::fmt()
        .with_writer(file_appender)
        .with_ansi(false)
        .init();

    // Generate CA certificate
    let ca = Arc::new(
        tls::CertAuthority::generate("ca.crt", "ca.key")
            .expect("Failed to generate CA certificate"),
    );

    // Compute CA cert path for subprocess usage
    let ca_cert_path = std::env::current_dir()?
        .join("ca.crt")
        .to_string_lossy()
        .to_string();

    // Print setup instructions
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║              ProxyClawd — MITM Proxy for Claude Code        ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
    eprintln!();
    eprintln!("  CA certificate written to: {}", ca_cert_path);
    eprintln!();
    eprintln!("  ── Install CA (choose one) ──────────────────────────────────");
    eprintln!();
    eprintln!("  macOS:");
    eprintln!(
        "    sudo security add-trusted-cert -d -r trustRoot \\",
    );
    eprintln!(
        "      -k /Library/Keychains/System.keychain {}",
        ca_cert_path
    );
    eprintln!();
    eprintln!("  Linux (Debian/Ubuntu):");
    eprintln!(
        "    sudo cp {} /usr/local/share/ca-certificates/proxyclawd.crt",
        ca_cert_path
    );
    eprintln!("    sudo update-ca-certificates");
    eprintln!();
    eprintln!("  Or (simplest — no root required):");
    eprintln!(
        "    export NODE_EXTRA_CA_CERTS={}",
        ca_cert_path
    );
    eprintln!();
    eprintln!("  ── Run Claude Code ─────────────────────────────────────────");
    eprintln!();
    eprintln!(
        "    HTTPS_PROXY=http://127.0.0.1:8080 NODE_EXTRA_CA_CERTS={} claude",
        ca_cert_path
    );
    if cli.web {
        eprintln!();
        eprintln!(
            "  ── Web UI ──────────────────────────────────────────────────"
        );
        eprintln!(
            "    http://localhost:{}",
            cli.web_port
        );
    }
    eprintln!();
    eprintln!("  Press Enter to start...");

    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf)?;

    // Create broadcast event channel
    let (event_tx, _) = broadcast::channel::<state::ProxyEvent>(4096);

    // Shared API key and headers stores (still needed by proxy for capture)
    let api_key_store: CapturedApiKey = Arc::new(Mutex::new(None));
    let captured_headers: CapturedHeaders = Arc::new(Mutex::new(Vec::new()));

    // Spawn proxy in background
    let listen_addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let proxy_ca = ca.clone();
    let proxy_tx = event_tx.clone();
    let proxy_api_key = api_key_store.clone();
    let proxy_headers = captured_headers.clone();
    let proxy_handle = tokio::spawn(async move {
        if let Err(e) = proxy::run_proxy(listen_addr, proxy_ca, proxy_tx, proxy_api_key, proxy_headers).await {
            tracing::error!("Proxy error: {:#}", e);
        }
    });

    // Spawn web server if --web flag is set, wait for it to bind before starting TUI
    let web_handle = if cli.web {
        let web_tx = event_tx.clone();
        let port = cli.web_port;
        let web_ca_cert_path = ca_cert_path.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();
        let handle = tokio::spawn(async move {
            if let Err(e) = web::run_web_server(port, web_tx, ready_tx, web_ca_cert_path).await {
                tracing::error!("Web server error: {:#}", e);
                eprintln!("Web server error: {:#}", e);
            }
        });
        // Wait for web server to be ready (or fail)
        let _ = ready_rx.await;
        Some(handle)
    } else {
        None
    };

    // Run TUI on main thread
    let tui_rx = event_tx.subscribe();
    let tui_result = tui::run_tui(tui_rx, ca_cert_path).await;

    // Cleanup
    proxy_handle.abort();
    if let Some(handle) = web_handle {
        handle.abort();
    }

    tui_result
}
