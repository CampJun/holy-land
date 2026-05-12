#!/usr/bin/env bash
# Deploy the Onion package to a Miyoo Mini Plus over Onion's built-in FTP.
#
# Usage:
#   ./cross/deploy-onion.sh 192.168.0.177
#
# The Miyoo WiFi/FTP stack is flaky: it can answer ping while FTP is wedged,
# and transfers can report success after truncation. This script waits for FTP,
# keeps a lightweight ping running during transfer, and verifies remote size.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST="${1:-192.168.0.177}"
USERPASS="${MIYOO_FTP_USERPASS:-onion:onion}"
LOCAL="$ROOT/target/onion/HolyLand/holyland"
REMOTE_DIR="/App/HolyLand"
REMOTE_FILE="holyland"
REMOTE_URL="ftp://$HOST$REMOTE_DIR"
CONNECT_TIMEOUT=5
MAX_TIME=90
WAIT_SECONDS="${MIYOO_WAIT_SECONDS:-90}"

if [ ! -f "$LOCAL" ]; then
    echo "missing $LOCAL; run ./cross/build-onion.sh first" >&2
    exit 1
fi

local_size="$(stat -c '%s' "$LOCAL")"

cleanup() {
    if [ -n "${PING_PID:-}" ]; then
        kill "$PING_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

ftp_list() {
    curl --silent --show-error --fail \
        --connect-timeout "$CONNECT_TIMEOUT" \
        --max-time 15 \
        -u "$USERPASS" \
        "$REMOTE_URL/"
}

remote_size() {
    ftp_list | awk -v name="$REMOTE_FILE" '$NF == name { print $5 }'
}

wait_for_ftp() {
    local deadline=$((SECONDS + WAIT_SECONDS))
    while [ "$SECONDS" -lt "$deadline" ]; do
        if ftp_list >/dev/null 2>&1; then
            return 0
        fi
        printf '.'
        sleep 2
    done
    printf '\n'
    return 1
}

echo "target: $HOST"
echo "local:  $LOCAL ($local_size bytes)"
echo "remote: $REMOTE_URL/$REMOTE_FILE"

printf 'waiting for FTP'
if ! wait_for_ftp; then
    echo "FTP did not respond within ${WAIT_SECONDS}s" >&2
    echo "Wake the Miyoo, open Onion FTP, and keep the screen on." >&2
    exit 1
fi
echo " ok"

ping -i 2 "$HOST" >/dev/null 2>&1 &
PING_PID=$!

echo "uploading..."
curl --fail --show-error \
    --connect-timeout "$CONNECT_TIMEOUT" \
    --max-time "$MAX_TIME" \
    --retry 5 \
    --retry-delay 2 \
    --retry-connrefused \
    -u "$USERPASS" \
    -T "$LOCAL" \
    "$REMOTE_URL/$REMOTE_FILE"

echo "verifying remote size..."
seen_size="$(remote_size)"
if [ "$seen_size" != "$local_size" ]; then
    echo "remote size mismatch: expected $local_size, got ${seen_size:-missing}" >&2
    exit 1
fi

echo "deployed ok: $REMOTE_FILE ($seen_size bytes)"
