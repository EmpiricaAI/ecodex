# ecodex — system overview

> **Audience:** developers + AI agents working on the ecodex product. Users
> see one tool; this doc shows the three layers underneath so contributors
> know where each concern lives.

ecodex is a coding agent that ships under one binary (`ecodex`) and one
brand. Internally, the system has three concerns that we build, integrate,
and ship as a single product:

| Layer | What it is | Who owns it |
|---|---|---|
| **L1 — codex foundation** | The agent runtime, TUI, sandbox, app-server, RPC protocol, MCP machinery. Forked from `openai/codex`. | Upstream codex maintainers + us (we PR fixes back). |
| **L2 — empirica integration** | The discipline wiring: PreToolUse / PostToolUse / SessionStart / Stop hooks routed to empirica's sentinel, transaction, and calibration scripts. Shipped as a codex plugin. | Us (this is the integration crate + bundled empirica CLI). |
| **L3 — specialised ecodex code** | The wire-protocol translator, the chat surface, the `ecodex` wrapper + installer, curated open-weights provider defaults. Net-new code with no upstream counterpart. | Us. |

This doc walks each layer top-down, then the user-facing experience that
the three compose into.

---

## L1 — codex foundation (inherited)

### What it provides

- **Agent runtime** (`codex-rs/core`): the agent loop, tool-use machinery,
  conversation state, sandbox enforcement.
- **TUI** (`codex-rs/tui`): the interactive terminal interface users see when
  they run `ecodex` with no arguments.
- **App-server** (`codex-rs/app-server`, `codex-rs/app-server-protocol`):
  JSON-RPC backend that exposes the agent loop to external clients (used by
  the planned ecodex Cockpit + future web product).
- **MCP machinery** (`codex-rs/mcp-server`, `codex-rs/codex-mcp`): Model
  Context Protocol server + client wiring for tool integration.
- **Sandbox** (`codex-rs/process-hardening`, `codex-rs/windows-sandbox-rs`,
  `codex-rs/vendor/bubblewrap`): platform sandboxing for safe shell execution.
- **Plugin system** (`codex-rs/plugins-manager`): the host that loads and
  dispatches to plugins like our empirica integration.
- **Hook system** (`codex-rs/hooks`): the event signal that plugins subscribe
  to (PreToolUse, PostToolUse, SessionStart, UserPromptSubmit, Stop, etc).
- **CLI** (`codex-rs/cli`): the binary entrypoint. We rebrand `bin_name` to
  `ecodex` here without touching upstream argument parsing.
- **SDKs** (`sdk/python`, `sdk/python-runtime`): Python SDKs for programmatic
  codex use.

### Where it lives

```
codex-rs/                        # 30+ Rust crates from upstream
sdk/python/                      # Python SDK (upstream)
sdk/python-runtime/              # Python runtime (upstream)
codex-cli/                       # Upstream npm wrapper (we don't ship this)
docs/                            # Mostly upstream codex docs
```

### How we relate to upstream

ecodex is a **product fork**, not a derivative — we follow upstream and
PR fixes back. Concretely:

- **Hardening flows both ways.** Their improvements come to us via main
  resync; ours go to them via PRs to `openai/codex`.
- **Lint scope.** `ecodex/ruff.toml` excludes upstream-only Python paths
  (`sdk/python`, `codex-rs/scripts`, `codex-cli`, vendored code) so our
  compliance posture reflects work we own. See [T62 commit
  `e505d53f96`](#) for the rationale.
- **Branding.** The `cli` crate renames `bin_name` and completion script
  identifiers to `ecodex`; everything else keeps upstream behavior.
- **Public framing.** "Empirica's branded build of codex with bundled
  defaults", not "a fork that diverges".

---

## L2 — empirica integration (the discipline wiring)

ecodex's differentiator is that every tool call, every prompt, every
session boundary is observed by empirica's epistemic discipline pipeline:
the Sentinel gates praxic actions on epistemic readiness, transactions
measure work, calibration grounds claims against artifacts.

### The plugin crate

`codex-rs/codex-empirica-plugin/` is a thin Rust crate that:

1. Registers itself with codex's plugin host via `manifest.json` (declares
   the crate as a plugin) and `hooks.json` (declares which event kinds the
   plugin subscribes to).
2. Receives hook events from codex at runtime — JSON payloads with the
   tool name, arguments, session info, etc.
3. **Shells out to empirica's existing Python CLI** to handle the actual
   discipline logic. The `src/empirica_cli.rs` module is the single
   subprocess boundary.

The handoff per hook event:

| codex event | Plugin action | Empirica CLI invoked |
|---|---|---|
| `PreToolUse` | Forward tool + args | `python3 .../sentinel-gate.py` |
| `PostToolUse` | Forward result + exit code | `python3 .../tool-failure.py` |
| `SessionStart` | Forward session metadata | `python3 .../session-init.py` |
| `UserPromptSubmit` | Forward user prompt | `python3 .../tool-router.py` |
| `Stop` | Forward final transcript | `python3 .../transaction-enforcer.py` |
| `PermissionRequest` | TBD | (planned for v1.1) |

**Why subprocess shellout (Option A from T15 research):** Empirica's Python
CLI stays canonical. Latency is 30–270 hook fires/session × 100–300ms/fire
= 1–15% session overhead — tolerable for v1. PyO3 / sidecar IPC / full
Rust port (Options B/C/D) become relevant only if real-world telemetry
shows per-tool-call latency unacceptable.

### Bundled CLI + skills

The plugin bundles:

- **`mcp_servers.json`** — the MCP servers empirica exposes (project
  memory, artifact log, etc).
- **`skills/`** — empirica skill definitions ported alongside the plugin
  so they're discoverable in the plugin context, not only in the user's
  global empirica install.

### Managed-config lock (recommended posture for distribution)

For ecodex distributions, `requirements.toml.example` shows how to use
codex's built-in `RequirementSource::SystemRequirementsToml` to pin
`plugins.empirica.enabled=true`. End users can't disable empirica
integration without changing the lock file — appropriate for the
"empirica-curated build" framing without modifying any codex source.
See `docs/ecodex/integrations/discipline-strengthening.md` for the
rationale + alternatives matrix.

### Goal pairing (planned, conditional)

Empirica's project-goals pair with codex's `ThreadGoal`. Since
`ThreadGoal` has no metadata field, pairing embeds the empirica goal_id
in the codex `objective` text via stable tag prefix `[empirica:<goal_id>]`.
The Stop hook captures completion and grounds empirica goal closure
against codex thread outcome. Round-trip validation is part of v1 plugin
build.

---

## L3 — specialised ecodex code (the net-new work)

This is what makes ecodex more than codex+empirica-plugin: net-new code
with no upstream counterpart, addressing the open-weights operator
target user.

### Translator (`codex-rs/codex-empirica-translator/`)

A wire-protocol bridge so codex (which emits OpenAI Responses-format
JSON) can talk to providers that only speak OpenAI Chat Completions
(DeepSeek, Qwen, GLM, Kimi, Ollama, llama.cpp, vLLM, etc.).

- **Architecture:** Canonical Intermediate Format (CIF) + per-provider
  adapters. CIF validated at N=3 (Responses + Chat Completions +
  Anthropic).
- **Runtime:** small `tiny_http` server. ecodex points at
  `http://127.0.0.1:18080/v1/responses`; translator rewrites + forwards.
- **Event tap:** every translation is logged to a configurable JSONL file
  (request_started / stream_event / request_completed / request_errored)
  so chat / cockpit / future surfaces can consume the live event stream
  without proxy access.
- **21/21 unit tests** + live-tested end-to-end against empirica-server
  (Strix Halo) and DeepSeek API.

### Chat (`empirica/empirica/cli/tui/chat_app.py` + `empirica/empirica/core/chat/`)

> Lives in the **empirica** repo (chat is an empirica deliverable that
> consumes the translator); included here because users encounter it as
> part of the ecodex experience.

`empirica chat` — a Textual TUI for collaborative AI conversation with
empirica discipline visible inline. 17 v0 phases shipped (~5025 LOC):

- Conversation render + jsonl persistence + replay mode
- Multi-provider switching (slash commands + Ctrl+M modal)
- Artifact cards with real CLI dispatch (resolve/pin/discuss/confirm)
- Statusline showing live epistemic state with shared
  `empirica.core.statusline` module (AnsiBackend for CC, RichBackend
  for chat)
- Phase indicator badge (🔍 INVESTIGATE / ▶ ACT) + intuition-vs-search
  badge per turn (💡/🔎)
- Natural-language workflow narration (translates raw empirica events
  into terse one-liners, no JSON shown)
- System prompt + autonomy modes (assistant / copilot / autonomous)
- Slash dispatch refactor (`/help` user-facing, `/help debug` for dev
  commands), `/plan`, `/autonomy`, `/compact` lifecycle hooks
- Batch artifact operations (`/batch`, `/resolve-batch`, `/delete-batch`)

See `empirica/docs/architecture/CHAT.md` for the full phase ledger.

### Wrapper + installer (`ecodex/`)

- **`ecodex/install.sh`** — one-command install. Builds the codex CLI,
  copies the binary as `ecodex`, installs the empirica plugin,
  optionally installs the wrapper script.
- **`ecodex/uninstall.sh`** — clean removal of all four installed
  artifacts.
- **`ecodex/scripts/ecodex-wrapper.sh`** — exports `EMPIRICA_SENTINEL_*`
  env vars before `exec`ing the ecodex binary so the empirica context is
  set per-shell-session.
- **`ecodex/config.toml.default`** — curated provider defaults
  (DeepSeek, Qwen, GLM, Kimi, Ollama, LMStudio, empirica-server) so a
  fresh install has working open-weights endpoints out of the box.
- **`ecodex/requirements.toml.example`** — managed-config lock template
  for organizations standardizing on the empirica-required posture.

### Lint scope (`ecodex/ruff.toml`)

`extend-exclude` for upstream-only paths so `empirica compliance-report`
scores ecodex on code we own, not upstream debt. See the file's header
comment for the rationale.

### Specs (`docs/ecodex/`, `docs/ecodex/specs/`, `docs/ecodex/integrations/`)

- **`system-overview.md`** (this file) — three-layer view.
- **`architecture.md`** — T3 decision record (D1–D4 + strategic posture).
- **`inspection.md`** — T2 codex-rs inspection findings.
- **`integrations/`** — per-integration design docs (providers,
  discipline-strengthening, etc.).
- **`specs/`** — planned-but-unbuilt component specs (web product, async
  calibration research, etc.).

---

## How the layers compose at runtime

A typical ecodex session, traced through the layers:

```
User runs: $ ecodex
              │
              ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L1: codex CLI bootstraps                                     │
   │   - reads ~/.codex/config.toml (curated by L3 ecodex defaults)│
   │   - reads ~/.codex/requirements.toml if present (L3 lock)    │
   │   - loads plugin manifest from codex plugin dir              │
   │   - finds codex-empirica-plugin (L2) → registers hook subs   │
   └────────┬─────────────────────────────────────────────────────┘
            │
            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L1: Agent runtime + TUI start                                │
   │   - SessionStart fires                                       │
   └────────┬─────────────────────────────────────────────────────┘
            │
            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L2: codex-empirica-plugin receives SessionStart              │
   │   - shells out to empirica session-init.py                   │
   │   - empirica creates a session record in ~/.empirica/        │
   └────────┬─────────────────────────────────────────────────────┘
            │
   User types prompt; agent decides to run a tool
            │
            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L1: PreToolUse fires before the tool executes                │
   └────────┬─────────────────────────────────────────────────────┘
            │
            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L2: codex-empirica-plugin → sentinel-gate.py                 │
   │   - sentinel checks if action is praxic + transaction is open│
   │   - returns allow / deny                                     │
   │   - on deny: codex blocks the tool call                      │
   └────────┬─────────────────────────────────────────────────────┘
            │
   Tool runs (or doesn't), result returned
            │
            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L1: PostToolUse fires                                        │
   └────────┬─────────────────────────────────────────────────────┘
            │
            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L2: codex-empirica-plugin → tool-failure.py                  │
   │   - logs result to empirica session                          │
   │   - if failure: artifact created (mistake/dead-end)          │
   └────────┬─────────────────────────────────────────────────────┘
            │
   Agent continues; if model needs an OpenAI Chat-Completions provider:
            │
            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L3: codex calls translator at http://127.0.0.1:18080/v1/...  │
   │   - translator parses Responses-format request               │
   │   - re-encodes as Chat-Completions for upstream provider     │
   │   - streams response back, re-encoded as Responses SSE       │
   │   - emits event-tap JSONL (started/chunk/completed)          │
   └────────┬─────────────────────────────────────────────────────┘
            │
   Optionally: user opens empirica chat in another pane
            │
            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L3: empirica chat reads translator event-tap JSONL           │
   │   - renders agent's request lifecycle as muted SystemTurns   │
   │   - statusline shows live epistemic state                    │
   │   - badges show INVESTIGATE/ACT phase + intuition/search     │
   └────────┬─────────────────────────────────────────────────────┘
            │
   User exits or session ends
            │
            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L1: Stop fires                                               │
   └────────┬─────────────────────────────────────────────────────┘
            │
            ▼
   ┌──────────────────────────────────────────────────────────────┐
   │ L2: codex-empirica-plugin → transaction-enforcer.py          │
   │   - verifies any open empirica transaction was POSTFLIGHTed  │
   │   - if not: blocks exit until POSTFLIGHT submitted           │
   └──────────────────────────────────────────────────────────────┘
```

---

## File layout cheatsheet

```
ecodex/                              # repo root
├── ruff.toml                        # L3: scope lint to our code
├── codex-rs/                        # L1: upstream codex Rust crates (30+)
│   ├── cli/                         # L1: rebranded entrypoint (bin_name=ecodex)
│   ├── core/                        # L1: agent runtime
│   ├── tui/                         # L1: terminal UI
│   ├── app-server/                  # L1: JSON-RPC backend
│   ├── plugins-manager/             # L1: plugin host
│   ├── hooks/                       # L1: hook event system
│   ├── codex-empirica-plugin/       # L2: our integration plugin
│   │   ├── manifest.json            # plugin registration
│   │   ├── hooks.json               # which events we subscribe to
│   │   ├── mcp_servers.json         # MCP servers we expose
│   │   ├── skills/                  # empirica skills bundled with plugin
│   │   └── src/
│   │       ├── main.rs              # plugin dispatcher
│   │       ├── empirica_cli.rs      # subprocess boundary (THE handoff point)
│   │       └── hooks/               # per-event handlers
│   └── codex-empirica-translator/   # L3: wire-protocol bridge
│       ├── src/                     # adapter + tiny_http server
│       └── tests_integration/       # live smoke tests
├── ecodex/                          # L3: distribution layer
│   ├── install.sh
│   ├── uninstall.sh
│   ├── config.toml.default          # curated provider defaults
│   ├── requirements.toml.example    # managed-config lock template
│   └── scripts/ecodex-wrapper.sh    # env-var pre-export
├── sdk/python/                      # L1: upstream Python SDK
├── sdk/python-runtime/              # L1: upstream Python runtime
└── docs/
    ├── ecodex/                      # L3: ecodex-specific docs
    │   ├── system-overview.md       # this file
    │   ├── architecture.md          # T3 decision record
    │   ├── inspection.md            # T2 codex-rs investigation
    │   ├── integrations/            # per-integration design
    │   └── specs/                   # planned components
    └── *.md                         # L1: upstream codex docs
```

---

## Pointers

- Discipline-strengthening options + recommendation:
  [`docs/ecodex/integrations/discipline-strengthening.md`](integrations/discipline-strengthening.md)
- Provider defaults + curation rationale:
  [`docs/ecodex/integrations/providers.md`](integrations/providers.md)
- Codex-rs inspection findings (T2):
  [`inspection.md`](inspection.md)
- Architectural decision record (T3 D1–D4):
  [`architecture.md`](architecture.md)
- Chat phase ledger:
  `empirica/docs/architecture/CHAT.md` (in empirica repo — 17 v0 phases shipped)
- Compliance scope (lint excludes for upstream):
  [`../../ruff.toml`](../../ruff.toml)
- Translator architecture:
  `codex-rs/codex-empirica-translator/README.md`
- Plugin architecture:
  `codex-rs/codex-empirica-plugin/README.md`
