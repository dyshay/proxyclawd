#!/usr/bin/env bash
# List intercepted API requests from ProxyClawd.
# Usage: list-requests.sh [limit]
set -euo pipefail

URL="${PROXYCLAWD_URL:-http://127.0.0.1:3000}"
LIMIT="${1:-50}"

RESPONSE=$(curl -sf "${URL}/api/requests" 2>/dev/null) || {
    echo "ERROR: Cannot connect to ProxyClawd at ${URL}" >&2
    exit 1
}

echo "$RESPONSE" | jq --argjson limit "$LIMIT" '
    [.[-$limit:][]] |
    reverse |
    [.[] | {
        id,
        timestamp,
        model,
        status,
        conversation_id,
        is_tool_loop,
        is_user_initiated,
        prompt_preview: (.prompt_text[:100]),
        response_preview: (.response_text[:100])
    }]
'
