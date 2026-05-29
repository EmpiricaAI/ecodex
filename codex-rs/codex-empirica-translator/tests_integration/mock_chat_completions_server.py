#!/usr/bin/env python3
"""
Mock chat-completions server for translator integration testing.

Listens on a configurable port, accepts POST /v1/chat/completions, and
emits a canonical SSE stream that exercises:
  - text deltas (multiple)
  - finish_reason
  - [DONE] sentinel

Verifies the inbound request shape AND emits a deterministic outbound
stream. Used by the translator smoke test to prove end-to-end flow
without depending on a real provider.

Usage:
    python3 mock_chat_completions_server.py [--port 9999]
"""
import argparse
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

LAST_REQUEST = {}  # captured for test inspection


class MockHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        # Quiet by default; uncomment to debug
        # sys.stderr.write(f"[mock] {fmt % args}\n")
        pass

    def do_POST(self):
        global LAST_REQUEST
        if self.path != "/v1/chat/completions":
            self.send_response(404)
            self.end_headers()
            return

        length = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(length).decode("utf-8")
        try:
            req = json.loads(body)
        except json.JSONDecodeError as e:
            self.send_response(400)
            self.end_headers()
            self.wfile.write(f"bad json: {e}".encode())
            return

        LAST_REQUEST = req

        # Validate request shape: must have model + messages array
        assert "model" in req, "request missing model"
        assert "messages" in req and isinstance(req["messages"], list), \
            "request missing messages array"

        # Authorization header check (translator should forward the key)
        auth = self.headers.get("authorization", "")

        # Stream a canonical SSE response: 3 text deltas + finish + DONE
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("X-Mock-Auth-Seen", "1" if auth.startswith("Bearer ") else "0")
        self.end_headers()

        chunks = [
            {"id": "mock-1", "choices": [{"delta": {"role": "assistant", "content": "Hello"}}]},
            {"id": "mock-1", "choices": [{"delta": {"content": " "}}]},
            {"id": "mock-1", "choices": [{"delta": {"content": "world"}}]},
            {"id": "mock-1", "choices": [{"delta": {}, "finish_reason": "stop"}]},
        ]
        for c in chunks:
            self.wfile.write(f"data: {json.dumps(c)}\n\n".encode())
            self.wfile.flush()
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()

    def do_GET(self):
        # Inspection endpoint for tests: GET /__last_request returns
        # the most recent request body the mock saw, as JSON.
        if self.path == "/__last_request":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(LAST_REQUEST).encode())
            return
        self.send_response(404)
        self.end_headers()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--port", type=int, default=9999)
    p.add_argument("--host", default="127.0.0.1")
    args = p.parse_args()
    server = HTTPServer((args.host, args.port), MockHandler)
    sys.stderr.write(f"mock chat-completions on http://{args.host}:{args.port}/v1/chat/completions\n")
    sys.stderr.flush()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        sys.stderr.write("mock shutting down\n")
        server.server_close()


if __name__ == "__main__":
    main()
