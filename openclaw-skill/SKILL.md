---
name: proxyclawd-bridge
description: Monitor and control ProxyClawd MITM proxy for Claude Code. Inspect intercepted API requests, view conversation history, and send messages through the proxy.
version: 0.1.0
metadata:
  openclaw:
    requires:
      bins: [curl, jq]
    emoji: "\U0001F50D"
---

# ProxyClawd Bridge

You are connected to a ProxyClawd MITM proxy instance. ProxyClawd intercepts Claude API traffic from Claude Code and exposes it via a REST API and WebSocket.

## Available Scripts

Run these scripts to interact with the proxy:

- `bash openclaw-skill/scripts/get-status.sh` - Check if ProxyClawd is running and get basic stats
- `bash openclaw-skill/scripts/list-requests.sh` - List all intercepted API requests
- `bash openclaw-skill/scripts/get-conversations.sh` - Group requests by conversation thread
- `bash openclaw-skill/scripts/send-message.sh "your message"` - Send a message through the Claude subprocess

## Direct API Usage

You can also use the REST API directly:

### List intercepted requests
```bash
curl -s http://${PROXYCLAWD_URL:-127.0.0.1:3000}/api/requests | jq
```

### Send a message
```bash
curl -s -X POST http://${PROXYCLAWD_URL:-127.0.0.1:3000}/api/send \
  -H "Content-Type: application/json" \
  -d '{"message": "hello", "continue_conversation": false}'
```

## Understanding the Data

Each intercepted request contains:
- `id` - Sequential request ID
- `conversation_id` - Hash identifying the conversation thread
- `model` - The Claude model used
- `prompt_text` - The user's prompt
- `response_text` - Claude's response
- `status` - One of: Pending, Streaming, Complete, Error
- `is_tool_loop` - Whether this is an automated tool-use cycle
- `is_user_initiated` - Whether the user directly triggered this request
- `message_count` - Number of messages in the conversation context

## Tips

- Use `get-conversations.sh` to understand the conversation flow
- Filter by `is_user_initiated` to see only user-driven requests (ignoring tool loops)
- The `system_prompt` field (when present) contains the full system prompt sent to Claude
