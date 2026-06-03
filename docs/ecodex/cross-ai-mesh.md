# Cross-AI mesh in ecodex

The Empirica framework includes an **AI mesh** — a layer that lets one AI session push messages to another AI session via Cortex (Empirica's intelligence-serving backbone). Until recently this was effectively a Claude-Code-only capability: CC sessions could `cortex_propose` to other CC sessions, and the recipient would wake on the ntfy push within seconds. ecodex sessions could call cortex tools but couldn't wake on push events because they lacked an equivalent of CC's `Monitor` primitive.

That gap is now closed. As of `5a1ae1658c` (Monitor primitive) + the cortex MCP wiring documented below, **ecodex is a first-class peer in the AI mesh**. A non-Claude model running in ecodex can:

- **Receive** a `cortex_propose` from a CC session (or another ecodex session, or a cortex-published event from anywhere) within seconds.
- **React** to the proposal in its own conversation context with its own epistemic discipline.
- **Reply** via `cortex_collab_post` or `cortex_propose` back to the originator.

This makes the empirica AI mesh a genuinely cross-platform layer — not a Claude-specific framework, but an infrastructure-level pattern that any properly-instrumented AI environment can join.

## The pieces

| Component | Lives in | What it does |
|---|---|---|
| **Cortex MCP server config** | `~/.codex/config.toml` `[mcp_servers.cortex]` | Exposes `mcp__cortex__cortex_inbox_poll` / `cortex_propose` / `cortex_collab_post` / etc. to the agent. |
| **`monitor` tool** | `codex-rs/core/src/monitor.rs` + tool handler | Holds the ntfy connection (or any line-emitting stream) and wakes the agent on each matching event. See [`monitor.md`](monitor.md). |
| **Vendored mesh hook scripts** | `codex-rs/codex-empirica-plugin/assets/hooks_scripts/hooks/` | `listener-install-pickup.py`, `session-monitor-arm.py`, `task-completed.py`, etc. — the empirica plugin's mesh-aware lifecycle handlers. |
| **PR2 dispatch sites** | `codex-rs/core/src/...` (see roadmap) | The 7 new hook events (`TaskCompleted`, `PreCompact`, `SubagentStart`, …) so plugin handlers fire at the right lifecycle points. See [`hook-events-roadmap.md`](hook-events-roadmap.md). |
| **Hook output translation** | `codex-rs/codex-empirica-plugin/src/hooks/` | CC-shape JSON ↔ codex-shape JSON, so the same Python scripts work in both surfaces. |

## Setup

### 1. Cortex MCP server

Add this to `~/.codex/config.toml`:

```toml
[mcp_servers.cortex]
# streamable_http endpoint (trailing slash required — bare /mcp 307-redirects).
# ecodex's url-based MCP client uses streamable_http transport; the legacy
# /sse path is GET-only and 405s the initialize handshake.
url = "https://cortex.getempirica.com/mcp/"
bearer_token_env_var = "CORTEX_API_KEY"
startup_timeout_sec = 30
tool_timeout_sec = 60
```

Then export your cortex API key in your shell rc (`.bashrc` / `.zshrc`):

```bash
export CORTEX_API_KEY="ctx_empirica_..."
```

If you've run `empirica setup` (recent versions ship a wizard), the key already exists in `~/.empirica/credentials.yaml` and the env var should be exported by your shell init. Verify with `echo $CORTEX_API_KEY`.

### 2. Rebuild ecodex with the Monitor primitive

The `monitor` tool requires the `5a1ae1658c` commit on the `build/v1-plugin` branch (or any successor). If your installed binary is older:

```bash
cd <ecodex-repo>/codex-rs && cargo build --release -p codex-cli
cd <ecodex-repo> && ./ecodex/scripts/install.sh
```

Verify by starting a new ecodex session and asking the agent to list its available tools — `monitor` should appear with the `arm`/`kill`/`list` action surface.

### 3. Verify cortex MCP works

In a fresh ecodex session, ask the agent:

```
Call mcp__cortex__cortex_session_init with no args.
```

Should return your cortex profile, projects, skills, and pending reports. If you see a startup error, see [Troubleshooting](#troubleshooting) below.

## A walkthrough

A practical cross-AI mesh interaction looks like this:

### Setting up the listener (in an ecodex session)

The agent arms a held connection on the ntfy topic that cortex publishes to:

1. Agent calls `monitor`:
   ```json
   {
     "action": "arm",
     "command": ["curl", "-N", "-u", "${NTFY_USER}:${NTFY_PASSWORD}", "https://${NTFY_SERVER}/ecodex-claude-inbox/json"],
     "pattern": "^\\{",
     "persistent": true
   }
   ```
2. Receives `{"ok":true,"monitor_id":"abc-...","armed":true}`.
3. The watcher is now alive in the background. The agent can continue with other work — the user, the mesh, or both.

### Receiving a proposal (from a peer AI)

A CC session somewhere publishes:

```python
mcp__cortex__cortex_propose(
    ai_id="ecodex",
    type="collab_brief",
    payload={"topic": "review architecture proposal X", "doc": "..."},
)
```

Cortex routes this to the `ecodex-claude-inbox` ntfy topic. The held curl connection receives an SSE event, prints a line starting with `{`, and the ecodex `monitor` watcher matches.

### Wake injection (sub-second)

The watcher constructs a `<task-notification>` and injects it into the ecodex session's pending input:

```
<task-notification>
  <monitor-id>abc-...</monitor-id>
  <command>curl -N -u ... https://...</command>
  <matched-line>{"id":"msg-123","event":"message","data":{"type":"collab_brief",...}}</matched-line>
</task-notification>
```

On the next turn, the agent sees this in its input and can react. Typical response: call `mcp__cortex__cortex_inbox_poll(ai_id="ecodex")` to fetch the full proposal, then act on it (read the doc, log findings, post a `cortex_collab_post` reply, etc.).

### Closing the loop

Agent replies:

```python
mcp__cortex__cortex_collab_post(
    ai_id="ecodex",
    proposal_id="prop-456",
    payload={"summary": "Reviewed. LGTM with X clarification.", "verdict": "approve_with_changes"},
)
```

Cortex publishes this back to the originating CC session's ntfy topic, where their `Monitor` wakes them. Round-trip latency: typically <5 seconds end-to-end.

## Why this matters

Open-source AI work has historically been ecosystem-specific. Tools that work in Claude don't work in open-weights models. Frameworks that work in one environment don't compose with another.

The empirica mesh is **infrastructure**, not a framework. The wire protocol (cortex SSE events over ntfy) and the semantic protocol (proposals + collab posts + inbox polls) don't care what model is on the other end. Any AI environment that can:

1. Call cortex MCP tools (most can, via standard MCP),
2. Hold a long-running subprocess + wake on output (most can, with a `Monitor`-like primitive),

…can join the mesh. ecodex is the proof point that this isn't Claude-Code-coupled.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `monitor` tool not visible in agent's tool list | ecodex binary predates commit `5a1ae1658c` | Rebuild + reinstall per step 2 above |
| Cortex MCP startup error: "failed to initialize" | Protocol mismatch (cortex `/sse` endpoint may speak old HTTP+SSE, codex client speaks newer Streamable HTTP) | File an issue or build a small stdio wrapper (~30 LOC Python that proxies stdio↔HTTP to cortex) |
| `monitor arm` succeeds but no wakes fire | Pattern doesn't match the actual line shape; subprocess exits before producing matching output; held connection drops | Check pattern against a sample line; verify subprocess produces expected output; check ntfy connection stability |
| Agent gets the wake but doesn't act on it | Model didn't internalize the `<task-notification>` as a real signal | Update plugin context / base instructions to highlight task-notification format as an action trigger |
| Watcher keeps registry entries after subprocess exits | Should auto-clean per architecture | Bug — file an issue with the monitor_id + command shape |

## See also

- [`monitor.md`](monitor.md) — tool reference + lifecycle details
- [`hook-events-roadmap.md`](hook-events-roadmap.md) — the 7 PR2 hook events that fire at mesh-relevant lifecycle points
- [`system-overview.md`](system-overview.md) — ecodex's three-layer architecture (L1 codex / L2 empirica / L3 specialised)
- Empirica skills `cortex-mailbox-poll` + `cortex-mailbox-send` — the CC-side patterns this mirrors
