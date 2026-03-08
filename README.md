# ProxyClawd

MITM proxy for Claude Code with real-time TUI & web UI — intercept, inspect, and **send your own messages** via Claude Code subprocess.

## Features

- **HTTPS MITM** via CONNECT tunnel with runtime TLS certificate generation
- **SSE stream parsing** with zero-latency forwarding
- **Three-panel TUI**: request list, prompt view, live streaming response
- **Web UI** (React + Tailwind) with WebSocket for real-time viewing in the browser
- **Headless mode** (`--headless`): run without TUI — API-only mode for integrations
- **MCP server** (`proxyclawd-mcp`): expose proxy data to any MCP client (Claude Code, Cursor, etc.)
- **Send messages**: compose and send prompts from the TUI or web UI — spawns a `claude -p` subprocess that routes through the proxy automatically, zero configuration needed
- **Continue conversations**: use `--continue` to reply in the last Claude Code conversation
- **Conversation threading** with collapsible tool-loop grouping
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

## Headless Mode

Run the proxy without the TUI — useful for servers, CI, or when using only the web UI / MCP server:

```bash
./proxyclawd --web --headless
```

This starts the proxy and web API on port 3000 without the terminal interface. The process runs until you press Ctrl+C.

You can combine it with a custom port:

```bash
./proxyclawd --web --headless --web-port 8000
```

## Sending Messages

You can compose and send messages directly from the TUI or web UI. Under the hood, this spawns a `claude -p --dangerously-skip-permissions` subprocess with `HTTPS_PROXY` set to route through the proxy. The request is intercepted like any other Claude Code traffic, and the response appears in real time.

> **Warning:** New/Reply spawns a new Claude Code session (`claude -p`). This is a **separate subprocess** — it does NOT reuse your existing interactive Claude Code session. The `--continue` flag (Reply) continues the **last conversation of the spawned subprocess**, not necessarily the conversation you selected in the UI.

### From the TUI

- `n` — compose a new message (new Claude Code session)
- `r` — reply (`--continue` — continues the last subprocess conversation)
- Type your message, then `Ctrl+S` to send
- `Esc` — cancel compose

### From the Web UI

1. Click **New** to compose a new message, or **Reply** to continue with `--continue`
2. Type your message and press **Ctrl+Enter** (or click Send)
3. The response streams in real time via the proxy — it appears in the request list like any other intercepted request

## Web UI

To launch the web interface alongside the TUI:

```bash
./proxyclawd --web
```

Then open http://localhost:3000 in your browser. The web UI shows the same data as the TUI in real time via WebSocket.

To use a different port:

```bash
./proxyclawd --web --web-port 8000
```

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/ws` | GET | WebSocket — real-time events |
| `/api/requests` | GET | All intercepted requests |
| `/api/send` | POST | Spawn a `claude -p` subprocess |

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

## MCP Server

`proxyclawd-mcp` is a standalone MCP (Model Context Protocol) server that connects to the ProxyClawd web API. It lets any MCP-compatible client (Claude Code, Cursor, etc.) query intercepted requests, view conversations, and send messages.

### Build

```bash
cargo build -p proxyclawd-mcp --release
```

### Configure

Add to your MCP client configuration (e.g. `~/.claude/claude_desktop_config.json` or Claude Code settings):

```json
{
  "mcpServers": {
    "proxyclawd": {
      "command": "proxyclawd-mcp",
      "args": ["--proxy-url", "http://127.0.0.1:3000"]
    }
  }
}
```

### MCP Tools

| Tool | Description |
|------|-------------|
| `list_requests` | List intercepted API requests (with optional `limit` and `status` filter) |
| `get_request` | Get full details of a request by ID |
| `send_message` | Send a message through the Claude subprocess |
| `get_conversations` | Group requests by conversation thread |
| `get_recent_events` | Get recent requests (optionally since a given ID) |

### MCP Resources

| URI | Description |
|-----|-------------|
| `proxy://requests` | Current list of all intercepted requests (JSON) |
| `proxy://status` | Connection status and request counters |

## OpenClaw Skill

An [OpenClaw](https://github.com/openclaw) skill is included in `openclaw-skill/` for agents that use the OpenClaw framework. It provides shell scripts to interact with the ProxyClawd REST API:

```bash
# Check if ProxyClawd is running
bash openclaw-skill/scripts/get-status.sh

# List intercepted requests
bash openclaw-skill/scripts/list-requests.sh

# Group by conversation
bash openclaw-skill/scripts/get-conversations.sh

# Send a message
bash openclaw-skill/scripts/send-message.sh "hello"
```

Set `PROXYCLAWD_URL` to override the default (`http://127.0.0.1:3000`).

## TUI Controls

| Key | Action |
|-----|--------|
| `Up/Down` or `k/j` | Navigate request list |
| `Enter` | Toggle collapse on conversation/tool-loop |
| `Page Up/Page Down` | Scroll response |
| `n` | New message (compose mode) |
| `r` | Reply (`--continue`) |
| `Ctrl+S` | Send message (in compose mode) |
| `Esc` | Cancel compose / quit |
| `q` | Quit |

## CLI Reference

```
proxyclawd [OPTIONS]

Options:
  --web              Enable web UI server
  --web-port <PORT>  Web UI port [default: 3000]
  --headless         Run without the TUI (requires --web)
  -h, --help         Print help
```

## Screenshots

<img width="1907" height="1025" alt="image" src="https://github.com/user-attachments/assets/637f19ed-9f6b-4c4e-9fd0-a88c0484c696" />


## Releasing

Releases are automatic. Bump the version in `Cargo.toml` and push — a GitHub Release with binaries for all platforms will be created.

## Stack

Rust, tokio, hyper, rustls, rcgen, ratatui, crossterm, axum, rmcp, clap | React, TypeScript, Tailwind CSS, Vite
