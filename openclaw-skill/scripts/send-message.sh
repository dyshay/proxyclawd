#!/usr/bin/env bash
# Send a message through the ProxyClawd Claude subprocess.
# Usage: send-message.sh "message" [--continue]
set -euo pipefail

URL="${PROXYCLAWD_URL:-http://127.0.0.1:3000}"

if [ $# -lt 1 ]; then
    echo "Usage: send-message.sh \"message\" [--continue]" >&2
    exit 1
fi

MESSAGE="$1"
CONTINUE=false
if [ "${2:-}" = "--continue" ]; then
    CONTINUE=true
fi

RESPONSE=$(curl -sf -X POST "${URL}/api/send" \
    -H "Content-Type: application/json" \
    -d "$(jq -n --arg msg "$MESSAGE" --argjson cont "$CONTINUE" \
        '{message: $msg, continue_conversation: $cont}')" \
    2>/dev/null) || {
    echo "ERROR: Cannot connect to ProxyClawd at ${URL}" >&2
    exit 1
}

echo "$RESPONSE" | jq .
