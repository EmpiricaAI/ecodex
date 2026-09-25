# `monitor` tool

The `monitor` tool starts a background subprocess and watches its output. On each line
that matches the supplied regex, a `<task-notification>` message is injected into the
agent's pending input — a sub-second wake on background events, whether or not a turn
is running.

## What it's for

- **Long-running build or test watching**: arm on a watch-mode test runner, wake on the
  first failure line.
- **Log tailing during an incident**: arm on `journalctl -f` or `tail -F`, wake on a
  specific error pattern.
- **Queue and stream listeners**: arm on any line-emitting daemon or held HTTP stream,
  wake only on the events you care about.
- **Detecting a stalled practitioner** (below).

The shape is always the same: a background process produces a stream, and you want the
agent to engage only when something specific appears in it.

**Mesh events don't need it.** ecodex receives Empirica mesh events natively — an
in-process listener holds the Cortex notification stream and wakes the session when a
proposal arrives (see [`cross-ai-mesh.md`](cross-ai-mesh.md)). `monitor` remains the
general-purpose primitive for everything else, and a fallback for custom topics the
native listener doesn't subscribe to.

### Detect an Empirica lab stall

`lab_stall_monitor.py` (shipped with the plugin's hook scripts) polls Empirica's real
transaction-activity signal rather than rollout or translator file times. It emits one
JSON event when an open transaction's `updated_at` has not advanced for the threshold
and the matching practitioner is still alive. Progress resets the detector, so a later
stall emits a new event.

Arm it from an orchestrating ecodex session:

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

Use `--instance <id>` when more than one practitioner inhabits the practice; the id can
be the transaction filename suffix, the harness session id, or the Empirica session id.
The event carries the transaction identity, `tool_call_count`, how long it has been
frozen, and the latest PREFLIGHT/CHECK/POSTFLIGHT phase (read from the project's
`reflexes` table, because the active-transaction file doesn't store it).

The live-process check rejects abandoned transaction files: it accepts a live tmux pane
still running a worker command (not a shell prompt), or a live presence PID. In
containers where PID and tmux namespaces make both checks impossible,
`--allow-unverified-process` is an explicit escape hatch — it weakens the detector and
can report dead sessions.

## API

The tool takes an `action` field that selects the operation.

### `arm` — start a watch

```json
{
  "action": "arm",
  "command": ["tail", "-F", "/var/log/app.log"],
  "pattern": "ERROR|panicked",
  "persistent": true,
  "stream": "stdout",
  "cwd": "/optional/working/directory"
}
```

| Field | Required | Default | Description |
|---|---|---|---|
| `action` | yes | — | `"arm"`. |
| `command` | yes | — | Argv to spawn: program first, then arguments. The child is killed when the watcher is dropped. |
| `pattern` | yes | — | Regex in `regex_lite` syntax (no look-around), matched line by line. |
| `persistent` | no | `false` | `true` keeps the watch armed after each match; `false` disarms it after the first match. |
| `stream` | no | `"stdout"` | `"stdout"`, `"stderr"` or `"both"`. |
| `cwd` | no | inherited | Working directory for the spawned command. |

Returns `{"ok": true, "monitor_id": "…", "armed": true}`. The `monitor_id` is the handle
for `kill`.

Arming is praxic: it starts a process. Do it inside an open transaction.

### Wake injection

On each matching line the agent's pending input receives a user-role message:

```
<task-notification>
  <monitor-id>abc-123-...</monitor-id>
  <command>tail -F /var/log/app.log</command>
  <matched-line>2026-09-25T10:14:03 ERROR upstream timed out</matched-line>
</task-notification>
```

If a turn is running, the notification joins that turn's pending input. If the session
is idle, it starts a new turn: delivery rides upstream's mailbox mechanism, so the
shared pending-work scheduler either attaches it to the active turn or wakes the idle
session.

### `kill` — disarm a watch

```json
{"action": "kill", "monitor_id": "abc-123-..."}
```

Returns `{"ok": true, "killed": true, "monitor_id": "…"}`. `killed: false` means the id
matched no armed monitor (already disarmed, or never existed).

### `list` — what is armed

```json
{"action": "list"}
```

Returns the armed monitors with their `monitor_id`, `command`, `pattern` and
`persistent` flag, plus a `count`.

## Lifecycle

- **Spawn**: the subprocess starts with stdout and stderr piped; the child is tied to the
  watcher so it can't outlive it.
- **Read loop**: a background task reads the chosen stream line by line; the regex is
  compiled once, at arm time.
- **Wake**: each match becomes a user-role message injected through the session's
  mailbox, with the turn-triggering flag set, so it is delivered whether or not a turn
  is running.
- **Disarm**:
  - `persistent: false` → the watcher disarms after its first wake.
  - `persistent: true` → it stays in the read loop until killed or the stream ends.
  - `kill` → the watcher task is aborted and the child reaped.
  - **Session shutdown** → every watcher and child is terminated at the start of
    shutdown, before other tasks are aborted.
- **Natural exit** (end of stream, or the child dies) → the watcher removes itself from
  the registry, so `list` stays accurate.

## Architecture

- **Runtime**: `codex-rs/core/src/monitor.rs` — `MonitorRegistry`, `MonitorEntry`,
  `ArmMonitorOptions`, `spawn_monitor`.
- **Tool handler**: `codex-rs/core/src/tools/handlers/monitor.rs` — `MonitorHandler`,
  dispatching `arm` / `kill` / `list`.
- **Registration**: `codex-rs/core/src/tools/spec_plan.rs` registers `MonitorHandler`
  unconditionally when the tool registry is built.
- **Session integration**: `codex-rs/core/src/state/service.rs` holds
  `monitor_registry: Arc<MonitorRegistry>` on the session services;
  `session/session.rs` creates it; `session/handlers.rs` aborts every watcher at
  shutdown.

## Compared with Claude Code's `Monitor`

| Aspect | Claude Code `Monitor` | ecodex `monitor` |
|---|---|---|
| What it watches | a shell command it starts (or a WebSocket) | a command it starts (argv) |
| Pattern matching | filtering is done in the command itself (`grep --line-buffered`) | a regex argument on the tool |
| Lifetime | expires after at most 30 minutes; the agent re-arms it | `persistent: true` stays armed until killed or the stream ends |
| Wake mechanism | the harness injects a notification into the conversation | mailbox injection into pending input |
| Disarm | `TaskStop` | `{"action": "kill", "monitor_id": "…"}` |
| Session cleanup | the harness | `abort_all()` at shutdown |

The lifetime row is the one that matters for long watches: an ecodex watch costs nothing
while it waits, whereas a capped watch has to be re-armed by a model turn each time it
expires.

## Smoke test

After rebuilding and reinstalling ecodex, an agent can check the round trip:

```
Arm a monitor on `sh -c 'seq 1 5; sleep 60'` with pattern `^3$` and persistent false.
```

The watcher sees `1`, `2`, `3` (match → wake → disarm), and `list` no longer shows it.
The agent should receive a `<task-notification>` containing
`<matched-line>3</matched-line>` within a few hundred milliseconds.
