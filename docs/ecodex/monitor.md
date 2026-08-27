# `monitor` tool

The `monitor` tool arms a watch on a background subprocess. On each line of subprocess output matching the supplied regex pattern, a `<task-notification>` message is injected into the agent's pending input — giving the conversation a sub-second wake on background events.

Parity with Claude Code's `Monitor` tool. Closes the wake-on-event gap that previously prevented non-Claude models running in ecodex from participating fully in the Empirica AI mesh.

## What it's for

- **Cross-AI mesh participation**: hold an ntfy SSE connection, wake on each push from a peer AI's `cortex_propose`.
- **Long-running build/test watching**: arm on `cargo test --watch` or `npm test --watch`, wake on the first FAIL line.
- **Log tailing for incidents**: arm on `journalctl -f`, wake on a specific error pattern.
- **Queue listeners**: arm on any line-emitting daemon, wake on the events you care about.

The general shape: a background process produces output as a stream; you only want the agent to engage when something specific shows up in that stream.

### Detect an Empirica lab stall

`lab_stall_monitor.py` polls Empirica's real transaction activity signal rather
than rollout or translator file mtimes. It emits a single JSON event when an
open transaction's `updated_at` has not advanced for the threshold and the
matching practitioner is still alive. Progress resets the detector, allowing a
later stall to emit a new event.

Arm it from an orchestrating ecodex session with the existing `monitor` tool:

```json
{
  "action": "arm",
  "command": [
    "python3",
    "-u",
    "/path/to/plugin/hooks_scripts/scripts/lab_stall_monitor.py",
    "--project",
    "/home/user/empirical-ai/ecodex-lab",
    "--ai-id",
    "ecodex-lab",
    "--threshold-seconds",
    "600"
  ],
  "pattern": "\"event\": \"lab_stall\"",
  "persistent": true
}
```

Use `--instance <id>` when more than one practitioner inhabits the practice.
The id can be the transaction filename suffix, Codex/Claude session id, or
Empirica session id. The emitted event includes transaction identity,
`tool_call_count`, frozen duration, and the latest PREFLIGHT/CHECK/POSTFLIGHT
phase. Phase comes from the project's `reflexes` table because it is not stored
in `active_transaction*.json` itself.

The live-process check rejects abandoned open transaction files. It accepts a
live tmux pane that is still running a worker command (not a shell prompt), or a
signal-0-live presence PID. In containers where PID and tmux namespaces make
both checks impossible, `--allow-unverified-process` is an explicit escape
hatch; using it weakens the detector and can report dead sessions.

## API

The tool takes a single `action` argument that selects the operation. All examples are JSON shapes the model emits to the tool.

### `arm` — start a new watch

```json
{
  "action": "arm",
  "command": ["curl", "-N", "-u", "user:pass", "https://ntfy.example/cortex-topic/json"],
  "pattern": "^\\{",
  "persistent": true,
  "stream": "stdout",
  "cwd": "/optional/working/directory"
}
```

| Field | Required | Default | Description |
|---|---|---|---|
| `action` | yes | — | Must be `"arm"`. |
| `command` | yes | — | Argv to spawn. First element is the program, rest are args. Spawn happens via `tokio::Command`; child has `kill_on_drop = true` so child dies when the watcher does. |
| `pattern` | yes | — | Regex (regex_lite syntax — no look-around). Matched line-by-line against the subprocess stream. |
| `persistent` | no | `false` | When `true`, the watch stays armed after each match and continues firing notifications. When `false`, the watch disarms itself after the first match. |
| `stream` | no | `"stdout"` | Which stream to watch: `"stdout"`, `"stderr"`, or `"both"`. |
| `cwd` | no | inherited | Working directory for the spawned command. |

**Returns:**

```json
{"ok": true, "monitor_id": "abc-123-...", "armed": true}
```

The `monitor_id` is the handle for later `kill` operations.

### Wake injection

On each matching line, the agent's pending input receives a user-role message with this body:

```
<task-notification>
  <monitor-id>abc-123-...</monitor-id>
  <command>curl -N -u user:pass https://ntfy.example/cortex-topic/json</command>
  <matched-line>{"event":"cortex_propose","payload":{...}}</matched-line>
</task-notification>
```

If a turn is active, the notification attaches to that turn's pending input. If no turn is active, the wake starts one — delivery rides upstream's mailbox mechanism (`Session::inject_response_items` enqueues the item as a mailbox communication and lets the shared pending-work scheduler either attach it to the active turn or wake the idle session).

### `kill` — disarm a watch

```json
{"action": "kill", "monitor_id": "abc-123-..."}
```

**Returns:**

```json
{"ok": true, "killed": true, "monitor_id": "abc-123-..."}
```

`killed: false` means the id did not match any armed monitor (already disarmed or never existed).

### `list` — introspect armed monitors

```json
{"action": "list"}
```

**Returns:**

```json
{
  "ok": true,
  "count": 2,
  "monitors": [
    {
      "monitor_id": "abc-123-...",
      "command": ["curl", "-N", "-u", "...", "https://ntfy.example/cortex-topic/json"],
      "pattern": "^\\{",
      "persistent": true
    },
    {
      "monitor_id": "def-456-...",
      "command": ["cargo", "test", "--watch"],
      "pattern": "^FAIL",
      "persistent": false
    }
  ]
}
```

## Lifecycle

- **Spawn**: ecodex spawns the subprocess via `tokio::Command` with `stdout` + `stderr` piped and `kill_on_drop = true`.
- **Read loop**: a background `tokio::spawn` task reads the chosen stream line-by-line. The regex is compiled once at arm time.
- **Wake**: on each match, the watcher constructs a `ResponseInputItem::Message` (user role) and calls `Session::inject_response_items`, which wraps the item as a synthetic `InterAgentCommunication` (`trigger_turn = true`) on upstream's mailbox API: an active turn picks it up as pending input, an idle session is woken into a new turn by the shared pending-work scheduler.
- **Disarm**:
  - **`persistent = false`** + match → watcher self-disarms after the first wake.
  - **`persistent = true`** → watcher stays in the read loop indefinitely.
  - Explicit `kill` → ecodex aborts the watcher task; `kill_on_drop` reaps the child.
  - **Session shutdown** → `Session::services.monitor_registry.abort_all()` runs at the start of `shutdown()` (before `abort_all_tasks`); all watchers + children are terminated cleanly.
- **Subprocess exits naturally** (stream EOF or child dies) → watcher removes itself from the registry; `list` / `len` stay accurate.

## Architecture

- **Module**: `codex-rs/core/src/monitor.rs` — runtime types, `MonitorRegistry`, `MonitorEntry`, `ArmMonitorOptions`, `spawn_monitor`.
- **Tool handler**: `codex-rs/core/src/tools/handlers/monitor.rs` — `MonitorHandler` implementing `ToolHandler`, dispatches `arm` / `kill` / `list`.
- **Session integration**: `codex-rs/core/src/state/service.rs` adds `monitor_registry: Arc<MonitorRegistry>` to `SessionServices`. `session/session.rs` initializes it at session construction. `session/handlers.rs:shutdown()` calls `abort_all`.
- **Tool spec**: `codex-rs/core/src/tools/spec.rs` builds the JSON-schema spec and registers the handler unconditionally at the tail of the registry-build function.

## Comparison to Claude Code's `Monitor`

| Aspect | Claude Code `Monitor` | ecodex `monitor` |
|---|---|---|
| Watches existing task | Yes (takes `task_id` from prior `Bash` w/ `run_in_background: true`) | No (bundles spawn + watch in one call) |
| Pattern matching | Regex on stream | Regex on stream (identical) |
| Wake mechanism | Harness injects into conversation | `Session::inject_response_items` → pending input |
| Persistent mode | Yes | Yes |
| Disarm | `TaskStop` on the underlying task | `{"action":"kill","monitor_id":"..."}` |
| Per-session cleanup | Harness handles | `monitor_registry.abort_all()` in `shutdown()` |

The bundled spawn+watch is a deliberate divergence: codex's `shell` tool is synchronous and has no native `run_in_background` flag. Bundling keeps the API atomic for the common use case (held connection) without requiring a separate background-shell primitive. If codex later gains a background-shell tool, this can be split.

## Smoke test

Once ecodex is rebuilt + reinstalled, an agent can verify the round trip:

```
Arm a monitor on `seq 1 5 | tr ' ' '\n' && sleep 60`,
with pattern `^3$`, persistent=false.
```

The watcher will see lines `1`, `2`, `3` (match → wake → disarm), then the child stays alive but the watcher has exited. The agent should receive a `<task-notification>` containing `<matched-line>3</matched-line>` within a few hundred milliseconds.
