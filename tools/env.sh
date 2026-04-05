#!/usr/bin/env bash
# Shared environment for API tools.
# Source this file — don't run it directly.
#
# Override BASE_URL before sourcing if the server isn't on localhost:3000:
#   export BASE_URL=http://192.168.1.50:3000
#   source tools/env.sh

BASE_URL="${BASE_URL:-http://localhost:3000}"
API="${BASE_URL}/api/v1"

# State directory — holds the access token and cookie jar between calls.
STATE_DIR="${STATE_DIR:-tools/.state}"
mkdir -p "$STATE_DIR"

TOKEN_FILE="$STATE_DIR/access_token"
COOKIE_JAR="$STATE_DIR/cookies.txt"

# ── Helpers ──────────────────────────────────────────────────────────

# Print the stored access token, or exit with a message.
require_token() {
    if [[ ! -f "$TOKEN_FILE" ]]; then
        echo "No access token found. Run ./tools/login or ./tools/register first." >&2
        exit 1
    fi
    cat "$TOKEN_FILE"
}

# Save an access token from a JSON response on stdin.
# Expects the response body piped in; prints it back out unchanged.
save_token() {
    local body
    body=$(cat)
    local token
    token=$(echo "$body" | jq -r '.access_token // empty')
    if [[ -n "$token" ]]; then
        echo "$token" > "$TOKEN_FILE"
    fi
    echo "$body"
}

# Pretty-print JSON if jq is available, otherwise pass through.
pretty() {
    if command -v jq &>/dev/null; then
        jq .
    else
        cat
    fi
}

# Run a curl command, print the JSON body (pretty-printed) and HTTP status
# separately. Pipes the body through save_token so tokens are captured.
# Usage: api_call [curl args...]
api_call() {
    local response status body
    # -w appends the status code after a newline; we split on it.
    response=$(curl -s -w "\n%{http_code}" "$@")
    status="${response##*$'\n'}"
    body="${response%$'\n'*}"
    echo "$body" | save_token | pretty
    echo "HTTP status: $status"
}
