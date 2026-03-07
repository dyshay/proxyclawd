# ProxyClawd

MITM proxy that intercepts traffic between Claude Code CLI and `api.anthropic.com`, displaying prompts and streamed responses in a real-time terminal UI or web interface.

## Features

- HTTPS MITM via CONNECT tunnel with runtime TLS certificate generation
- SSE stream parsing with zero-latency forwarding
- Three-panel TUI: request list, prompt view, live streaming response
- Web UI (React + Tailwind) with WebSocket for real-time viewing in the browser
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

3. Set the environment variables **in the terminal where you will run Claude Code** (not the proxy terminal):

**Linux/macOS (session only):**
```bash
export HTTPS_PROXY=http://127.0.0.1:8080
export NODE_EXTRA_CA_CERTS=$(pwd)/ca.crt
claude
```

**Windows (PowerShell, session only):**
```powershell
$env:HTTPS_PROXY = "http://127.0.0.1:8080"
$env:NODE_EXTRA_CA_CERTS = "$pwd\ca.crt"
claude
```

**To set them permanently (Windows):**
```powershell
[Environment]::SetEnvironmentVariable("HTTPS_PROXY", "http://127.0.0.1:8080", "User")
[Environment]::SetEnvironmentVariable("NODE_EXTRA_CA_CERTS", "C:\path\to\ca.crt", "User")
```

**To set them permanently (Linux/macOS):** add to `~/.bashrc` or `~/.zshrc`:
```bash
export HTTPS_PROXY=http://127.0.0.1:8080
export NODE_EXTRA_CA_CERTS=/path/to/ca.crt
```

4. Press Enter in the proxy terminal to launch the TUI and watch requests in real time.

## Web UI

To also launch the web interface alongside the TUI:

```bash
./proxyclawd --web
```

Then open http://localhost:3000 in your browser. The web UI shows the same data as the TUI in real time via WebSocket.

To use a different port:

```bash
./proxyclawd --web --web-port 8000
```

### Building the frontend

The web UI is served from `frontend/dist/`. To rebuild it:

```bash
cd frontend
npm install
npm run build
```

For development with hot reload (proxies API/WS to the Rust server on port 3000):

```bash
cd frontend
npm run dev
```

## TUI Controls

- `Up/Down` or `k/j` — navigate request list
- `Page Up/Page Down` — scroll response
- `q` or `Esc` — quit

## Screenshots

<img width="1907" height="1025" alt="image" src="https://github.com/user-attachments/assets/637f19ed-9f6b-4c4e-9fd0-a88c0484c696" />


## Releasing

Releases are automatic. Bump the version in `Cargo.toml` and push — a GitHub Release with binaries for all platforms will be created.

## Stack

Rust, tokio, hyper, rustls, rcgen, ratatui, crossterm, axum, clap | React, TypeScript, Tailwind CSS, Vite
