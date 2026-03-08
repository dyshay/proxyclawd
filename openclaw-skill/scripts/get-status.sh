#!/usr/bin/env bash
# Check if ProxyClawd is running and display basic stats.
set -euo pipefail

URL="${PROXYCLAWD_URL:-http://127.0.0.1:3000}"

if ! curl -sf "${URL}/api/requests" -o /dev/null 2>/dev/null; then
    echo "ERROR: ProxyClawd is not reachable at ${URL}"
    echo "Make sure ProxyClawd is running with --web flag:"
    echo "  cargo run -p proxyclawd -- --web"
    exit 1
fi

REQUESTS=$(curl -sf "${URL}/api/requests")
TOTAL=$(echo "$REQUESTS" | jq 'length')
PENDING=$(echo "$REQUESTS" | jq '[.[] | select(.status == "Pending")] | length')
STREAMING=$(echo "$REQUESTS" | jq '[.[] | select(.status == "Streaming")] | length')
COMPLETE=$(echo "$REQUESTS" | jq '[.[] | select(.status == "Complete")] | length')
ERRORS=$(echo "$REQUESTS" | jq '[.[] | select(.status | type == "object" and has("Error"))] | length')
CONVERSATIONS=$(echo "$REQUESTS" | jq '[.[].conversation_id] | unique | length')

jq -n \
    --arg url "$URL" \
    --argjson total "$TOTAL" \
    --argjson pending "$PENDING" \
    --argjson streaming "$STREAMING" \
    --argjson complete "$COMPLETE" \
    --argjson errors "$ERRORS" \
    --argjson conversations "$CONVERSATIONS" \
    '{
        status: "connected",
        url: $url,
        total_requests: $total,
        pending: $pending,
        streaming: $streaming,
        complete: $complete,
        errors: $errors,
        conversations: $conversations
    }'
