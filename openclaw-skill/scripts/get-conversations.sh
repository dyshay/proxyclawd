#!/usr/bin/env bash
# Group intercepted requests by conversation ID.
set -euo pipefail

URL="${PROXYCLAWD_URL:-http://127.0.0.1:3000}"

RESPONSE=$(curl -sf "${URL}/api/requests" 2>/dev/null) || {
    echo "ERROR: Cannot connect to ProxyClawd at ${URL}" >&2
    exit 1
}

echo "$RESPONSE" | jq '
    group_by(.conversation_id) |
    map({
        conversation_id: .[0].conversation_id,
        request_count: length,
        models: [.[].model] | unique,
        user_initiated: [.[] | select(.is_user_initiated)] | length,
        tool_loops: [.[] | select(.is_tool_loop)] | length,
        first_timestamp: (sort_by(.timestamp) | first.timestamp),
        last_timestamp: (sort_by(.timestamp) | last.timestamp),
        requests: [.[] | {
            id,
            timestamp,
            status,
            is_tool_loop,
            is_user_initiated,
            prompt_preview: (.prompt_text[:80])
        }]
    }) |
    sort_by(.last_timestamp) |
    reverse
'
