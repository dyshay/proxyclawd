use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tokio::sync::broadcast;

mod proxy;
mod sse;
mod state;
mod tls;
mod tui;
mod web;

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

    // Print setup instructions
    let cert_path = std::env::current_dir()?.join("ca.crt");
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║              ProxyClawd — MITM Proxy for Claude Code        ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
    eprintln!();
    eprintln!("  CA certificate written to: {}", cert_path.display());
    eprintln!();
    eprintln!("  ── Install CA (choose one) ──────────────────────────────────");
    eprintln!();
    eprintln!("  macOS:");
    eprintln!(
        "    sudo security add-trusted-cert -d -r trustRoot \\",
    );
    eprintln!(
        "      -k /Library/Keychains/System.keychain {}",
        cert_path.display()
    );
    eprintln!();
    eprintln!("  Linux (Debian/Ubuntu):");
    eprintln!(
        "    sudo cp {} /usr/local/share/ca-certificates/proxyclawd.crt",
        cert_path.display()
    );
    eprintln!("    sudo update-ca-certificates");
    eprintln!();
    eprintln!("  Or (simplest — no root required):");
    eprintln!(
        "    export NODE_EXTRA_CA_CERTS={}",
        cert_path.display()
    );
    eprintln!();
    eprintln!("  ── Run Claude Code ─────────────────────────────────────────");
    eprintln!();
    eprintln!(
        "    HTTPS_PROXY=http://127.0.0.1:8080 NODE_EXTRA_CA_CERTS={} claude",
        cert_path.display()
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

    // Spawn proxy in background
    let listen_addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let proxy_ca = ca.clone();
    let proxy_tx = event_tx.clone();
    let proxy_handle = tokio::spawn(async move {
        if let Err(e) = proxy::run_proxy(listen_addr, proxy_ca, proxy_tx).await {
            tracing::error!("Proxy error: {:#}", e);
        }
    });

    // Spawn web server if --web flag is set
    let web_handle = if cli.web {
        let web_tx = event_tx.clone();
        let port = cli.web_port;
        Some(tokio::spawn(async move {
            if let Err(e) = web::run_web_server(port, web_tx).await {
                tracing::error!("Web server error: {:#}", e);
            }
        }))
    } else {
        None
    };

    // Run TUI on main thread
    let tui_rx = event_tx.subscribe();
    let tui_result = tui::run_tui(tui_rx).await;

    // Cleanup
    proxy_handle.abort();
    if let Some(handle) = web_handle {
        handle.abort();
    }

    tui_result
}
