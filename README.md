# ProxyClawd

MITM proxy that intercepts traffic between Claude Code CLI and `api.anthropic.com`, displaying prompts and streamed responses in a real-time terminal UI.

## Features

- HTTPS MITM via CONNECT tunnel with runtime TLS certificate generation
- SSE stream parsing with zero-latency forwarding
- Three-panel TUI: request list, prompt view, live streaming response
- Concurrent connection handling with tokio
- Pre-built binaries for Linux (x86_64, aarch64), macOS (x86_64, aarch64), and Windows (x86_64)

## Install

Download the latest binary from [Releases](../../releases/latest), or build from source:

```bash
cargo build --release
```

## Usage

1. Start the proxy:

```bash
./proxyclawd
```

2. The proxy generates a CA certificate (`ca.crt`) on first run. Install it:

**macOS:**
```bash
sudo security add-trusted-cert -d -r trustRoot \
  -k /Library/Keychains/System.keychain ca.crt
```

**Linux (Debian/Ubuntu):**
```bash
sudo cp ca.crt /usr/local/share/ca-certificates/proxyclawd.crt
sudo update-ca-certificates
```

**Linux/macOS (no root required):**
```bash
export NODE_EXTRA_CA_CERTS=$(pwd)/ca.crt
```

3. Run Claude Code through the proxy (in a separate terminal):

**Linux/macOS:**
```bash
HTTPS_PROXY=http://127.0.0.1:8080 NODE_EXTRA_CA_CERTS=$(pwd)/ca.crt claude
```

**Windows (PowerShell):**
```powershell
$env:HTTPS_PROXY = "http://127.0.0.1:8080"
$env:NODE_EXTRA_CA_CERTS = "$pwd\ca.crt"
claude
```

4. Press Enter in the proxy terminal to launch the TUI and watch requests in real time.

## TUI Controls

- `Tab` — switch between panels
- `Up/Down` — navigate request list
- `q` — quit

## Releasing

Releases are automatic. Bump the version in `Cargo.toml` and push — a GitHub Release with binaries for all platforms will be created.

## Stack

Rust, tokio, hyper, rustls, rcgen, ratatui, crossterm
