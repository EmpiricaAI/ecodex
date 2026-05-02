#!/usr/bin/env bash
# Reproducible end-to-end smoke test for codex-empirica-translator.
#
# Spins up a mock chat-completions server, starts the translator pointed at
# it, sends a Responses-format request, and verifies the response stream +
# event tap. Exit 0 on success.
#
# Usage:
#   ./smoke_test.sh   (from this directory; assumes release binary built)

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
TRANSLATOR_DIR="${SCRIPT_DIR}/.."
WORKSPACE_DIR="${TRANSLATOR_DIR}/.."  # codex-rs root (where target/ lives)
RELEASE_BIN="${WORKSPACE_DIR}/target/release/codex-empirica-translator"
MOCK_SCRIPT="${SCRIPT_DIR}/mock_chat_completions_server.py"
EVENT_LOG="$(mktemp -t translator-smoke-XXXXXX.jsonl)"
MOCK_PORT=19999
TRANSLATOR_PORT=18080

echo "=== smoke test: codex-empirica-translator ==="

if [[ ! -x "$RELEASE_BIN" ]]; then
  echo "release binary not found at $RELEASE_BIN" >&2
  echo "build it with: (cd $WORKSPACE_DIR && cargo build --release -p codex-empirica-translator)" >&2
  exit 2
fi

# ─── Start mock chat-completions server ───────────────────────────────
python3 "$MOCK_SCRIPT" --port "$MOCK_PORT" >/dev/null 2>&1 &
MOCK_PID=$!
trap 'kill $MOCK_PID 2>/dev/null || true; kill $TRANSLATOR_PID 2>/dev/null || true; rm -f "$EVENT_LOG"' EXIT

for _ in $(seq 1 20); do
  if curl -s --max-time 1 "http://127.0.0.1:${MOCK_PORT}/__last_request" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

# ─── Start translator ─────────────────────────────────────────────────
"$RELEASE_BIN" \
  --upstream-base-url "http://127.0.0.1:${MOCK_PORT}/v1" \
  --bind "127.0.0.1:${TRANSLATOR_PORT}" \
  --event-log "$EVENT_LOG" \
  >/dev/null 2>&1 &
TRANSLATOR_PID=$!

for _ in $(seq 1 20); do
  if curl -s --max-time 1 -o /dev/null "http://127.0.0.1:${TRANSLATOR_PORT}/_unused"; then
    break
  fi
  sleep 0.1
done

# ─── Send a Responses-format request ──────────────────────────────────
RESPONSE=$(curl -s --max-time 5 -X POST "http://127.0.0.1:${TRANSLATOR_PORT}/v1/responses" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "deepseek-chat",
    "instructions": "be brief",
    "input": [{"type":"message","role":"user","content":[{"type":"input_text","text":"hi"}]}],
    "stream": true
  }')

# ─── Verify response stream ───────────────────────────────────────────
echo "--- response stream ---"
echo "$RESPONSE"

if ! grep -q "response.output_text.delta" <<<"$RESPONSE"; then
  echo "FAIL: response.output_text.delta not in stream" >&2
  exit 1
fi
if ! grep -q "response.completed" <<<"$RESPONSE"; then
  echo "FAIL: response.completed not in stream" >&2
  exit 1
fi
if ! grep -q '"text":"Hello world"' <<<"$RESPONSE"; then
  echo "FAIL: assembled text 'Hello world' not in completed event" >&2
  exit 1
fi

# ─── Verify upstream request shape ────────────────────────────────────
LAST_REQ=$(curl -s "http://127.0.0.1:${MOCK_PORT}/__last_request")
echo "--- upstream saw ---"
echo "$LAST_REQ" | python3 -m json.tool

if ! python3 -c "
import json,sys
r=json.loads('''$LAST_REQ''')
assert r['model']=='deepseek-chat'
assert r['messages'][0]['role']=='system'
assert r['messages'][0]['content']=='be brief'
assert r['messages'][1]['role']=='user'
assert r['messages'][1]['content']=='hi'
"; then
  echo "FAIL: upstream request shape wrong" >&2
  exit 1
fi

# ─── Verify event tap ─────────────────────────────────────────────────
EVENT_COUNT=$(wc -l <"$EVENT_LOG")
echo "--- event tap: $EVENT_COUNT events captured ---"
if [[ "$EVENT_COUNT" -lt 6 ]]; then
  echo "FAIL: expected >=6 events in tap, got $EVENT_COUNT" >&2
  exit 1
fi
if ! grep -q '"kind":"request_started"' "$EVENT_LOG"; then
  echo "FAIL: request_started event missing from tap" >&2
  exit 1
fi
if ! grep -q '"kind":"request_completed"' "$EVENT_LOG"; then
  echo "FAIL: request_completed event missing from tap" >&2
  exit 1
fi

echo ""
echo "=== ✓ smoke test PASSED ==="
echo "  - Responses request → chat-completions translation: OK"
echo "  - chat-completions SSE → Responses SSE translation: OK"
echo "  - assembled text correct: OK"
echo "  - event tap structure: OK"
exit 0
