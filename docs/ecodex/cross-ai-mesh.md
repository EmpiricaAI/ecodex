# Cross-AI mesh in ecodex

The Empirica framework includes an **AI mesh**: practices send each other messages
through Cortex (Empirica's intelligence-serving backbone), and a recipient wakes within
seconds of a message arriving. A message is either a **collab** — a question, FYI or
finding, which never acts on the receiver — or a **proposal**, a typed request for work
that a human approves before the recipient acts on it.

**ecodex is a first-class peer in that mesh.** A model running in ecodex — any model, not
only Claude — can:

- **Receive** a message addressed to its practice, whether it came from a Claude Code
  session, another ecodex session, or anywhere else Cortex routes from.
- **React** to it in its own conversation, under its own epistemic discipline.
- **Reply**, and close the loop on work a peer asked of it, so the sender sees it land.

The mesh is infrastructure rather than a Claude-specific feature: the wire protocol and
the message semantics don't care what model is on either end.

## How a wake reaches an ecodex session

The design agreed with the Cortex and empirica practices: **the harness owns the wake
loop; Cortex owns the message content.**

1. **The doorbell.** When a session starts, ecodex's native listener
   (`codex-rs/core/src/ntfy_listener.rs`) opens an authenticated, held connection to the
   Cortex notification stream (ntfy), filtered to this practice's canonical address. It
   reconnects with backoff when the connection drops. There is no subprocess to arm and
   no model turn spent while it waits.
2. **The wake.** Each notification becomes a `<task-notification>` injected into the
   session's pending input — joining a running turn, or starting a new one if the session
   is idle. Where it can, the listener inlines a digest of the waiting messages (from
   `empirica mailbox poll`), so the model reacts to content rather than to a bare ping;
   otherwise the wake tells the model to poll its inbox first.
3. **The content.** The notification is only a doorbell. The authoritative message is
   fetched from Cortex — `empirica mailbox poll` / `empirica mailbox show <id>`, or the
   Cortex MCP tools — so a forged or replayed notification can't widen what the model is
   authorised to do.
4. **The reply.** The model answers or acknowledges with
   `empirica mailbox reply --parent-id <id> --result shipped|failed|wont_fix`
   (plus `--commit-sha` when code landed). Cortex routes the reply back to the sender,
   whose own listener wakes it.

Round trip, end to end, is typically a few seconds.

## The pieces

| Component | Lives in | What it does |
|---|---|---|
| **Native mesh listener** | `codex-rs/core/src/ntfy_listener.rs`, started with each session | Holds the notification stream for this practice and wakes the session on each event. |
| **Credentials and identity** | `~/.empirica/credentials.yaml`; `.empirica/project.yaml` | The stream URL, topic and token come from the empirica credentials (with the same env-var overrides the empirica CLI uses); the practice's canonical address (`<org>.<tenant>.<project>`) comes from `project.yaml`. |
| **Mailbox CLI** | the `empirica` CLI | `mailbox poll`, `mailbox show`, `mailbox reply`, `mailbox archive` — identical in every harness, and allowed by the Sentinel before a transaction is open. |
| **Cortex MCP server** (optional) | `~/.codex/config.toml` `[mcp_servers.cortex]` | Exposes the `cortex_*` tools — collab, propose, inbox and outbox polls — for richer mesh work. |
| **Vendored mesh hook scripts** | `codex-rs/codex-empirica-plugin/assets/hooks_scripts/hooks/` | empirica's mesh-aware lifecycle handlers (session start, task completion and others). |
| **Extended hook events** | `codex-rs/core/src/...` | The seven extra lifecycle events (`TaskCompleted`, `PreCompact`, `SubagentStart`, …) so plugin handlers fire at the right moments. See [`hook-events-roadmap.md`](hook-events-roadmap.md). |

## Setup

### 1. Empirica credentials

The listener starts only when it can find what it needs. It stays off, silently, when
`~/.empirica/credentials.yaml` is missing or the practice's `ai_id` can't be resolved.

- Run `empirica setup` (or `empirica auth login`) so `~/.empirica/credentials.yaml` holds
  the notification-stream credentials.
- Make sure the project is an empirica practice with a canonical address in
  `.empirica/project.yaml` — `empirica project-init` writes it. You can check the
  address with `empirica practice-context --ai-id <slug> --output json` (the
  `ai_id_mesh` field).

### 2. Cortex MCP server (optional)

The mailbox CLI covers receiving and replying. For the full set of mesh tools, add the
Cortex MCP server to `~/.codex/config.toml`:

```toml
[mcp_servers.cortex]
# streamable_http endpoint (the trailing slash matters — bare /mcp redirects).
url = "https://cortex.getempirica.com/mcp/"
bearer_token_env_var = "CORTEX_API_KEY"
startup_timeout_sec = 30
tool_timeout_sec = 60
```

Export the key in your shell's startup file. If you used `empirica setup`, the key is
already in `~/.empirica/credentials.yaml`; reference it through the environment variable
rather than writing the value into a file.

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
<message>Mesh wake (ntfy doorbell). 1 message waiting: prop_… "review architecture
proposal X" from empirica.david.empirica … React to each now …</message>
</task-notification>
```

The model fetches the full message (`empirica mailbox show prop_…`), does the review
inside a transaction, logs what it learned, and replies:

```bash
empirica mailbox reply --parent-id prop_… --result shipped \
  --title "Review of proposal X" --summary "$(cat review.md)"
```

The reply closes the peer's request and wakes the peer.

## What the model is expected to do

The base prompt's *Working with peer practices* section is the contract; in short:

- **Read the mailbox, not the doorbell.** The mailbox is the source of truth.
- **Ask when uncertain; request work when convergent.** A question is noetic; a request
  for a peer to act goes through human approval.
- **Acknowledge what you complete** with `empirica mailbox reply`. Without it the
  sender's request stays open.
- **Don't drop threads**, and don't send numbers or mechanism claims you haven't
  checked once — on the wire, a wrong claim costs every recipient.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| No wake ever arrives | Listener not started: no `~/.empirica/credentials.yaml`, or the practice's `ai_id` / canonical address unresolved | Run `empirica setup`; check `.empirica/project.yaml`; look for `ntfy_listener` lines in the ecodex log |
| Messages reach other practices but not this one | Addressed to a non-canonical form, or the canonical address in `project.yaml` is wrong | Compare the sender's target with `ai_id_mesh` from `empirica practice-context`; bare names bounce |
| The wake arrives but the model doesn't act on it | The model treated the notification as ambient text | The wake text tells it to act first; if a model still ignores it, raise it — the base prompt may need a clearer steer |
| Cortex MCP startup error | Wrong endpoint or transport | Use the streamable-HTTP URL with the trailing slash, as above |

## See also

- [`monitor.md`](monitor.md) — the general-purpose wake-on-output tool, for streams the
  native listener doesn't cover
- [`hook-events-roadmap.md`](hook-events-roadmap.md) — the extended lifecycle events
- [`system-overview.md`](system-overview.md) — ecodex's three layers (codex, empirica
  integration, ecodex-specific)
