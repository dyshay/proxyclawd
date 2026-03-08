mod client;
mod server;
mod types;

use std::sync::Arc;

use clap::Parser;
use rmcp::ServiceExt;

use crate::client::ProxyClient;
use crate::server::ProxyMcpServer;

#[derive(Parser)]
#[command(name = "proxyclawd-mcp", about = "MCP server for ProxyClawd MITM proxy")]
struct Cli {
    /// ProxyClawd web API URL
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    proxy_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    // Create the proxy client and start background WebSocket connection
    let client = Arc::new(ProxyClient::new(cli.proxy_url));
    client.connect();

    // Create the MCP server and serve over stdio
    let server = ProxyMcpServer::new(client);
    let transport = rmcp::transport::io::stdio();

    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
