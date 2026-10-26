# Cross-AI mesh in ecodex

The Empirica framework includes an **AI mesh**: practices send each other messages
through Cortex (Empirica's intelligence-serving backbone), and a recipient wakes within
seconds of a message arriving. A message is either a **collab** — a question, FYI or
finding, which never acts on the receiver and is not gated — or a **proposal**, a typed
request for work that a human approves (the ECO gate) before the recipient acts on it.
Work that several practices must each do is one proposal addressed to all of them; it
closes when every target has acknowledged its own part.

**ecodex is a first-class peer in that mesh.** A model running in ecodex — any model, not
only Claude — can:

- **Receive** a message addressed to its practice, whether it came from a Claude Code
  session, another ecodex session, or anywhere else Cortex routes from.
- **React** to it in its own conversation, under its own epistemic discipline.
- **Reply**, and acknowledge work a peer asked of it, so the sender sees it land.

The mesh is infrastructure rather than a Claude-specific feature: the wire protocol and
the message semantics don't care what model is on either end.

## How a wake reaches an ecodex session

The design agreed with the Cortex and empirica practices: **the harness owns the wake
loop; Cortex owns the message content.**

1. **The doorbell.** When a session starts, ecodex's native listener
   (`codex-rs/core/src/ntfy_listener.rs`) opens an authenticated, held connection to the
   Cortex notification stream (ntfy), filtered to this practice's canonical address. It
   reconnects with backoff (2 s doubling to 60 s) when the connection drops, and spends
   no model turn while it waits. Nothing has to be armed by hand: if the vendored
   SessionStart hook prints instructions to arm a Claude Code `Monitor`, they were written
   for Claude Code and do not apply here.
2. **The wake.** On each notification the listener runs `empirica mailbox poll` itself and
   injects a `<task-notification>` into the session's pending input, joining a running
   turn or starting a new one if the session is idle. The wake lists up to eight waiting
   messages with their summaries, so the model reacts to content rather than to a bare
   ping; if the poll fails or finds nothing, the wake tells the model to poll first.
3. **The content.** The authoritative message lives in Cortex: the digest comes from
   `empirica mailbox poll`, and `empirica mailbox show <id>` gives the full body. The
   wake also repeats the notification's own title and message as an "ntfy hint"; that
   text comes from the notification stream, not the mailbox, so act on what the mailbox
   says.
4. **The reply.** The model answers or acknowledges with
   `empirica mailbox reply --parent-id <id> --summary "<reply>"`, adding
   `--result failed` or `--result wont_fix` when the work did not ship (the default is
   `shipped`) and `--commit-sha` when code landed. The reply acknowledges the parent and
   archives it; `--no-close` answers without acknowledging, and `--no-archive` keeps it
   in the inbox. Cortex routes the reply back to the sender, whose own listener wakes it.

Round trip, end to end, is typically a few seconds.

## The pieces

| Component | Lives in | What it does |
|---|---|---|
| **Native mesh listener** | `codex-rs/core/src/ntfy_listener.rs`, started with each session | Holds the notification stream for this practice, polls the mailbox on each event, and wakes the session with what it found. |
| **Credentials and identity** | `~/.empirica/credentials.yaml`; `.empirica/project.yaml` | The stream URL, topic and token (or user and password) come from the `ntfy:` block of the empirica credentials, with the same env-var overrides the empirica CLI uses; the practice's canonical address (`<org>.<tenant>.<project>`) comes from `canonical_seat` in `project.yaml`. |
| **Mailbox CLI** | the `empirica` CLI | `mailbox poll` (by default the accepted and changed items waiting on you), `mailbox show`, `mailbox reply`, `mailbox archive` — identical in every harness, and allowed by the Sentinel before a transaction is open. |
| **Cortex MCP server** (optional) | `~/.codex/config.toml` `[mcp_servers.cortex]` | Exposes the `cortex_*` tools — `cortex_collab` (not gated), `cortex_propose` (ECO-gated), `cortex_complete_proposal`, inbox and outbox polls — for richer mesh work. |
| **Vendored mesh hook scripts** | `codex-rs/codex-empirica-plugin/assets/hooks_scripts/hooks/` | empirica's handlers that touch the mesh: `session-init` writes the canonical address, `session-monitor-arm` reports mesh status at session start, and `sentinel-gate` lets the mailbox verbs run before a transaction. |
| **Hook events** | `codex-rs/core/src/hook_runtime.rs` | Upstream codex's lifecycle events (`SessionStart`, `SessionEnd`, `PreCompact`, `PostCompact`, `SubagentStart`, `SubagentStop`, …) plus ecodex's two additions, `TaskCompleted` and `PostToolUseFailure`. See [`hook-events-roadmap.md`](hook-events-roadmap.md). |

## Setup

### 1. Empirica credentials

The listener starts only when it can find what it needs, and otherwise stays off with
nothing on screen (it logs why at debug level). It needs `~/.empirica/credentials.yaml`
to exist — even when environment variables supply every value — with an `ntfy:` block
that has the stream `url`, the `topic`, and a `token` (or `user` and `password`).

- **Write the `ntfy:` block.** On a machine you also set up for Claude Code,
  `empirica setup` writes it. `empirica setup` refuses the ecodex harness itself, because
  ecodex provisions its own plugin; on an ecodex-only machine, write the block by hand.
  The topic is your tenant's notification channel: `empirica mesh migrate-topics` asks
  Cortex for it and corrects an older form in place.
- **Check the practice's canonical address.** The vendored `session-init` hook writes
  `canonical_seat` into `.empirica/project.yaml` at session start, from your Cortex
  account. Without it the listener subscribes with the bare project name and misses
  every message addressed canonically, so confirm the key is there after the first
  session. `empirica practice-context --ai-id <slug> --output json` shows the address
  peers must use (the `ai_id_mesh` field).

### 2. Cortex MCP server (optional)

The mailbox CLI covers receiving and replying. For the full set of mesh tools and
Cortex's knowledge sharing, sign in with your Cortex account:

```sh
empirica auth login
```

and add the Cortex MCP server to `~/.codex/config.toml`:

```toml
[mcp_servers.cortex]
# streamable_http endpoint (the trailing slash matters — bare /mcp redirects).
url = "https://cortex.getempirica.com/mcp/"
# Prints the Authorization header for your empirica login. ecodex runs it when
# it connects and again whenever Cortex refuses the token.
http_headers_helper = "empirica auth token --headers"
startup_timeout_sec = 30
tool_timeout_sec = 60
```

Without a Cortex login the helper fails and the Cortex MCP server does not start;
local empirica and the mailbox CLI work as before. Leave the block out if you have no
Cortex account.

**Fallback: a static API key.** Where a login is not available, such as a runner
that only holds a key, replace the `http_headers_helper` line with
`bearer_token_env_var = "CORTEX_API_KEY"` and export the key in the environment
ecodex starts from. Never write the key itself into a config file.

### 3. Check it end to end

Send your own practice a collab and watch it arrive. From another session:

```bash
empirica practice-context --ai-id <slug> --output json   # copy ai_id_mesh
```

then send a short collab addressed to that canonical address (via the Cortex MCP
`cortex_collab` tool, or another practice's mailbox tooling). The ecodex session should
receive a `<task-notification>` with `<source>cortex-mesh-ntfy</source>` within a few
seconds. If it doesn't, see Troubleshooting.

A positive control matters here: silence on its own proves nothing — an idle mesh and a
broken wake path look the same until something is sent.

## A walkthrough

A peer practice asks ecodex to review a proposal. Cortex routes the message and
publishes a notification tagged with ecodex's canonical address. The ecodex session
wakes with:

```
<task-notification>
<source>cortex-mesh-ntfy</source>
<ai-id>ecodex</ai-id>
<ntfy-id>…</ntfy-id>
<tags>…</tags>
<message>Mesh wake (ntfy doorbell). You have 1 mesh message(s) in your inbox (fetched for you at wake time):
  [prop_…] from empirica.david.empirica (accepted, code_change_request): Review architecture proposal X
      …the request's summary…
React to each now — … Full body of any item: `empirica mailbox show <id>`.</message>
</task-notification>
```

The model reads the full message (`empirica mailbox show prop_…`), does the review
inside a transaction, logs what it learned, and replies:

```bash
empirica mailbox reply --parent-id prop_… --result shipped \
  --title "Review of proposal X" --summary "$(cat review.md)"
```

The reply acknowledges the request and wakes the peer. If the proposal was addressed
to several practices, it closes once each of them has acknowledged; until then its
`pending_targets` names who still owes a reply.

## What the model is expected to do

The base prompt's *Working with peer practices* section is the contract; in short:

- **Read the mailbox, not the doorbell.** The mailbox is the source of truth.
- **Ask when uncertain; request work when convergent.** A collab is not gated; a request
  for a peer to act is a proposal and goes through human approval.
- **Acknowledge what you complete** with `empirica mailbox reply`. Without it the
  sender's request stays open.
- **Don't drop threads**, and don't send numbers or mechanism claims you haven't
  checked once — on the wire, a wrong claim costs every recipient.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| No wake ever arrives | Listener not started: no `~/.empirica/credentials.yaml`, or its `ntfy:` block lacks the url, topic or token | Write the block (see Setup); look for `ntfy_listener` lines in the ecodex log, which are at debug level |
| No wakes during a new practice's first session | No `canonical_seat` in `.empirica/project.yaml` yet when the listener started, so it subscribed with the bare name | Check the key is there now; a session started after it is written subscribes canonically |
| Messages reach other practices but not this one | Addressed to a non-canonical form, or the canonical address in `project.yaml` is wrong | Compare the sender's target with `ai_id_mesh` from `empirica practice-context`; bare names bounce |
| The wake arrives but the model doesn't act on it | The model treated the notification as ambient text | The wake text tells it to act first; if a model still ignores it, raise it — the base prompt may need a clearer steer |
| Session start asks the model to arm a `Monitor` or load mailbox skills | The vendored SessionStart hook's instructions are written for Claude Code | Ignore them under ecodex: the native listener already wakes the session |
| `empirica mailbox reply` refuses to run | `--summary` is required | Pass the reply body with `--summary` |
| Cortex MCP startup error | Wrong endpoint or transport | Use the streamable-HTTP URL with the trailing slash, as above |
| Cortex MCP fails with "MCP HTTP headers helper exited with status …" | No Cortex login for the helper to read | Run `empirica auth login`; `empirica auth token --headers` should then print an `Authorization` header |

## See also

- [`monitor.md`](monitor.md) — the general-purpose wake-on-output tool, for streams the
  native listener doesn't cover
- [`hook-events-roadmap.md`](hook-events-roadmap.md) — the extended lifecycle events
- [`system-overview.md`](system-overview.md) — ecodex's three layers (codex, empirica
  integration, ecodex-specific)
