use anyhow::Result;
use tokio::process::Command;

/// Spawn `claude -p` subprocess that sends its request through our proxy.
///
/// The message is passed via stdin (pipe) to avoid shell escaping issues.
/// Environment variables HTTPS_PROXY and NODE_EXTRA_CA_CERTS are set so
/// the subprocess routes through our MITM proxy.
pub async fn spawn_claude_message(
    message: &str,
    continue_conversation: bool,
    ca_cert_path: &str,
) -> Result<()> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p");
    cmd.arg("--dangerously-skip-permissions");

    if continue_conversation {
        cmd.arg("--continue");
    }

    cmd.env("HTTPS_PROXY", "http://127.0.0.1:8080");
    cmd.env("NODE_EXTRA_CA_CERTS", ca_cert_path);

    // Pass message via stdin to avoid escaping issues
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("Claude CLI not found in PATH. Install it with: npm install -g @anthropic-ai/claude-code")
        } else {
            anyhow::anyhow!("Failed to spawn claude process: {}", e)
        }
    })?;

    // Write message to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(message.as_bytes()).await?;
        // Drop stdin to close it, signaling EOF
    }

    // Wait for the process to complete in background
    let output = child.wait_with_output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!("claude subprocess failed: {}", stderr);
    } else {
        tracing::info!("claude subprocess completed successfully");
    }

    Ok(())
}
