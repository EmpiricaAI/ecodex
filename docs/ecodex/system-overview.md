# ecodex — system overview

> **Audience:** developers and AI agents working on ecodex. Users see one tool;
> this doc shows the three layers underneath so contributors know where each
> concern lives. [`ARCHITECTURE.md`](../../ARCHITECTURE.md) is the shorter map;
> this is the tour.

ecodex ships as one binary (`ecodex`) under one brand. Underneath are three
concerns that we build, integrate and ship as a single product:

| Layer | What it is | Who owns it |
|---|---|---|
| **L1 — codex foundation** | The agent runtime, TUI, sandbox, app-server, RPC protocol, MCP machinery, plugin and hook hosts. Forked from `openai/codex`. | Upstream codex maintainers, and us (we PR fixes back). |
| **L2 — empirica integration** | The discipline wiring: the plugin that routes codex's hook events to Empirica's Sentinel, transaction and calibration hooks, the base prompt, and the startup provisioning that installs it all. | Us. |
| **L3 — ecodex-specific code** | The wire-protocol translator, curated providers and the model registry, the mesh listener and `Monitor` tool, provider hot-swap, installers, branding. Net-new code with no upstream counterpart. | Us. |

This doc walks each layer, then traces a session through all three.

---

## L1 — codex foundation (inherited)

### What it provides

- **Agent runtime** (`codex-rs/core`): the agent loop, tool machinery,
  conversation state, sandbox enforcement.
- **TUI** (`codex-rs/tui`): the interactive interface users see when they run
  `ecodex` with no arguments.
- **App-server** (`codex-rs/app-server`, `codex-rs/app-server-protocol`): the
  JSON-RPC backend that exposes the agent loop to other clients.
- **MCP machinery** (`codex-rs/codex-mcp`, `codex-rs/rmcp-client`): Model
  Context Protocol client and connection management.
- **Sandbox** (`codex-rs/process-hardening`, `codex-rs/windows-sandbox-rs`,
  `codex-rs/vendor/bubblewrap`): platform sandboxing for shell execution.
- **Plugin host** (`codex-rs/core-plugins`, `codex-rs/plugin`): discovers,
  loads and enables plugins, including their skills, MCP servers, hooks and
  `writableRoots`.
- **Hook system** (`codex-rs/hooks`): the events plugins subscribe to
  (`PreToolUse`, `PostToolUse`, `SessionStart`, `UserPromptSubmit`, `Stop`,
  compaction, subagent and session-end events).
- **CLI** (`codex-rs/cli`): the binary entrypoint, branded `ecodex`.
- **SDKs** (`sdk/python`, `sdk/python-runtime`): Python SDKs for programmatic
  use.

### Where it lives

```
codex-rs/            # the Rust workspace: 152 crates, almost all upstream
sdk/python/          # Python SDK (upstream)
sdk/python-runtime/  # Python runtime (upstream)
codex-cli/           # upstream npm wrapper (ecodex publishes its own under npm/)
docs/                # mostly upstream codex docs; ours are in docs/ecodex/
```

### How we relate to upstream

ecodex is a **product fork**: it follows upstream and PRs fixes back.

- **Re-syncs** merge a tagged upstream release into `main`; `codex-rs/UPSTREAM_SYNC_TAG`
  records which, and ecodex's major.minor version follows it.
- **Hardening flows both ways.** Upstream improvements arrive with each
  re-sync; ours go back as PRs to `openai/codex`.
- **Lint scope.** The root `ruff.toml` excludes upstream-only Python paths so
  `empirica compliance-report` scores ecodex on the code it owns.
- **Public framing.** "Empirica's build of codex with the discipline bundled
  in", not "a fork that diverges".

---

## L2 — empirica integration (the discipline wiring)

ecodex's differentiator is that every tool call, prompt and session boundary is
observed by Empirica's discipline: the Sentinel gates praxic actions on
epistemic readiness, transactions measure the work, and calibration grounds
self-assessment against evidence.

### The plugin crate

`codex-rs/codex-empirica-plugin/` is both a library and a binary.

**As a library**, linked into `ecodex`, it carries the plugin's assets
(`manifest.json`, `hooks.json`, `mcp_servers.json`, `skills/`, the vendored
hooks and agents, the default config and translator routes). On every
interactive, `exec`, resume or fork start, `provision()` writes them to
`~/.codex/plugins/cache/empiricaAI/empirica/<version>/` when missing or out of
date, enables `empirica@empiricaAI` in `config.toml` (unless the user set
`enabled = false`), migrates the old `empirica@nubaeon` key, and writes
`config.toml` and `translator-upstreams.toml` when none exist.

**As a binary**, `codex-empirica-plugin`, it is the hook host codex runs for each
event in `hooks.json`. It runs the matching vendored Python hook with the
interpreter of the `empirica` CLI on PATH (from that script's shebang), sets the
hook's identity (`EMPIRICA_INSTANCE_ID` from codex's thread id,
`EMPIRICA_HARNESS=codex`), and translates the hook's output into the shape codex
accepts (`translate_output.rs`). `src/empirica_cli.rs` is the single subprocess
boundary.

| codex event | Hooks | What happens |
|---|---|---|
| `SessionStart` | (host) AGENTS.md seed, subagent seed, practice bootstrap; `session-init`, `ewm-protocol-loader`, `post-compact`, `session-monitor-arm` | Writes the empirica block into `~/.codex/AGENTS.md`, copies the bundled subagents into `~/.codex/agents/empirica/`, prepares a fresh practice from the harness process, then binds the session and loads epistemic context and the workflow protocol |
| `PreToolUse` | `sentinel-gate` | Classifies the call noetic/praxic by effect and denies praxic work before CHECK |
| `PostToolUse` | `tool-failure`, `entity-extractor`, `truncation-legibility` (shell only) | Counts noetic and praxic work, extracts code entities from edited files, tells the model when the output it read was partial |
| `PostToolUseFailure` | `tool-failure` | Records genuine dead-ends, not operational noise |
| `UserPromptSubmit` | `tool-router`, `context-shift-tracker` | Assesses the prompt against the epistemic state; records whether it answers the model or starts something new |
| `Stop` | `transaction-enforcer` | Blocks stopping when a transaction has run many turns without a POSTFLIGHT |
| `PreCompact` / `PostCompact` | `pre-compact`, `post-compact` | Carries epistemic state across a compaction |
| `SessionEnd`, `TaskCompleted`, `SubagentStart` / `SubagentStop` | `session-end-postflight`, `task-completed`, `subagent-*` | Closes the measurement loop; ties tasks and subagents to Empirica goals |

`PermissionRequest` is wired to a no-op for now.

**Why a subprocess per hook.** Empirica's Python stays canonical, and a hook
behaves exactly as it does under Claude Code. The cost is 30–270 hook fires per
session at roughly 100–300 ms each, a 1–15% overhead. An in-process or sidecar
path becomes worth it only if that latency proves unacceptable in real use.

### The base prompt

ecodex replaces codex's base instructions with `codex-rs/models-manager/prompt-empirica.md`
(`BASE_INSTRUCTIONS` in `models-manager/src/model_info.rs`): Empirica's
discipline framed for the ecodex CLI, with upstream's tool, shell, coding and
formatting guidance folded in. Upstream's original prompt is kept alongside only
to diff against on each re-sync.

### Skills, agents and MCP

- **`skills/`** — Empirica skills ported for codex (constitution, epistemic
  transaction, persistence protocol, onboarding interview, code audit and
  others), discoverable through codex's plugin skills.
- **`assets/agents/`** — Empirica's subagents (architecture, security, ux,
  performance, and the outreach scout, search and fact-scorer), seeded at
  `SessionStart` for codex's agent tool.
- **`mcp_servers.json`** — the Empirica MCP server the plugin registers.

### Discipline strengthening

Two things keep the discipline on: the plugin is enabled by default, and the
binary turns on strict mode at startup on every install path. Nothing locks the
plugin on; `enabled = false` on its entry turns it off.
[`integrations/discipline-strengthening.md`](integrations/discipline-strengthening.md)
has the decision and the details.

---

## L3 — ecodex-specific code

Net-new code with no upstream counterpart, mostly for the open-weights
operator.

### Translator (`codex-rs/codex-empirica-translator/`)

codex speaks only the Responses API; many open-weights providers speak only
Chat Completions or Anthropic Messages. The translator is a small HTTP server
(`tiny_http`, a thread per request) on `127.0.0.1:18080` that accepts Responses
requests and forwards them through a Canonical Intermediate Format (CIF) and
per-protocol adapters.

- Routes by model name from `~/.codex/translator-upstreams.toml` (written on
  first start with routes for Mistral, DeepSeek, Qwen, GLM and Kimi); keys come
  from the environment or `~/.empirica/credentials.yaml`, and a route without a
  key is skipped.
- Retries a provider's rate limit (429) itself, honouring `Retry-After`.
- Repairs tool-call arguments chat models produce but codex rejects
  (`tool_args.rs`).
- Can log every translation to a JSONL event log for other surfaces to consume.
- Started by hand; nothing auto-spawns it.

### Providers and the model registry

- **Curated providers** (`codex-rs/tui/src/ecodex_curated_models.rs` and the
  default config) lead with open-weights clouds and local servers;
  [`integrations/providers.md`](integrations/providers.md) has the set.
- **The model registry** (`codex-rs/models-manager/models.curated.json` plus a
  user overlay at `~/.codex/models.user.json`): `ecodex models list` shows it,
  `ecodex models refresh` probes each configured provider's `/v1/models`.
  See [`integrations/model-registry.md`](integrations/model-registry.md).
- **Provider hot-swap**: switching to a model on another provider mid-session
  swaps the model client in place (`codex-rs/core/src/session/`), and choosing
  a bare OpenAI-family id switches to the built-in `openai` provider.

### Mesh

- **ntfy listener** (`codex-rs/core/src/ntfy_listener.rs`): holds an
  authenticated ntfy stream and turns each ECO-decided proposal event into a
  session wake. It is transport only; the content is fetched over the Cortex MCP
  tools by the woken model.
- **`Monitor` tool** (`codex-rs/core/src/tools/handlers/monitor.rs`): lets the
  model watch a background stream and be woken by its events.
- [`cross-ai-mesh.md`](cross-ai-mesh.md) covers the design.

### Branding

`bin_name` and the completion scripts say `ecodex`; the TUI's logo animation
renders the Empirica "E" mark at both ends of its morph
(`codex-rs/tui/src/empty_state_animation/paths.rs`) instead of turning the Codex
mark into the OpenAI one.

### Installers

- **`scripts/install.sh`** — the prebuilt installer behind `curl | bash` and
  `ecodex update`: downloads the release, installs `ecodex`,
  `codex-empirica-plugin` and `codex-empirica-translator`, and sets up the
  `empirica` CLI with pipx or uv when it is missing.
- **Homebrew** (`packaging/homebrew/ecodex.rb`, published to the EmpiricaAI tap),
  depending on the tap's `empirica` formula.
- **`ecodex/scripts/install.sh`** — the source-build installer: builds the
  binaries, installs them behind `ecodex/scripts/ecodex-wrapper.sh` (which also
  passes a cortex key for mesh installs), per user or, with `--system`, under
  `/usr/local`. `uninstall.sh` reverses it.

### Empirica chat

`empirica chat`, a terminal chat surface with the discipline visible inline,
lives in the Empirica repo. It can read the translator's event log to show an
agent's requests as they happen.

---

## How the layers compose at runtime

A typical session, traced through the layers:

```
$ ecodex
   │
   ▼
L3/L2  ecodex binary starts
       - strict-mode env vars default to true (arg0)
       - provision(): plugin, config entry, default config and routes written if needed
   │
   ▼
L1     codex loads config, discovers empirica@empiricaAI in the plugin cache,
       registers its hooks, starts the agent runtime and TUI; the ntfy listener
       connects if configured
   │
   ▼
L2     SessionStart → codex-empirica-plugin session-start
       - AGENTS.md block and subagents seeded, practice prepared
       - session-init.py binds the session; its context reaches the model's first turn
   │
   │   user prompt → UserPromptSubmit → tool-router.py
   │   model decides to run a tool
   ▼
L2     PreToolUse → sentinel-gate.py (under the empirica CLI's interpreter)
       - praxic before CHECK → deny; codex blocks the call
   │
   ▼
L1     tool runs
   │
   ▼
L2     PostToolUse → tool-failure.py, entity-extractor.py, truncation-legibility.py
   │
   │   model on a Chat-Completions provider?
   ▼
L3     codex → translator :18080 → CIF → provider's protocol → back as Responses SSE
   │
   │   a peer proposal arrives
   ▼
L3     ntfy listener wakes the session; the model reads the proposal over Cortex MCP
   │
   ▼
L2     Stop → transaction-enforcer.py; SessionEnd → session-end-postflight.py
```

---

## File layout cheatsheet

```
ecodex/                                   # repo root
├── ruff.toml                             # lint scope: the code ecodex owns
├── codex-rs/                             # the Rust workspace
│   ├── cli/                              # L1: entrypoint (bin_name ecodex) + L2 provisioning call
│   ├── arg0/                             # L1; L2 strict-mode defaults at startup
│   ├── core/                             # L1 agent runtime; L3 ntfy listener, Monitor, hot-swap
│   ├── tui/                              # L1 UI; L3 curated models, Empirica mark
│   ├── models-manager/                   # L1; L2 base prompt, L3 curated registry seed
│   ├── core-plugins/, plugin/, hooks/    # L1 plugin and hook hosts
│   ├── codex-empirica-plugin/            # L2
│   │   ├── manifest.json, hooks.json, mcp_servers.json
│   │   ├── skills/                       # Empirica skills for codex
│   │   ├── assets/hooks_scripts/         # vendored Empirica hooks (setup-codex.py)
│   │   ├── assets/agents/                # vendored Empirica subagents
│   │   ├── assets/config/                # default config.toml + translator routes
│   │   ├── tests/vendored_hooks/         # pytest against a real empirica
│   │   └── src/                          # provision, empirica_cli, translate_output, hooks/
│   └── codex-empirica-translator/        # L3: Responses ↔ Chat/Anthropic bridge
├── ecodex/                               # source-build installer and wrapper
├── scripts/                              # install.sh, release.sh, sync-homebrew.sh, setup-codex.py, CI guards
├── packaging/homebrew/                   # the Homebrew formula
├── npm/                                  # the ecodex npm package
└── docs/ecodex/                          # ecodex docs: this file, api/, integrations/, specs/
```

---

## Pointers

- The short map: [`ARCHITECTURE.md`](../../ARCHITECTURE.md)
- Install, update, troubleshooting: [`INSTALL.md`](INSTALL.md)
- Discipline strengthening: [`integrations/discipline-strengthening.md`](integrations/discipline-strengthening.md)
- Providers and the translator: [`integrations/providers.md`](integrations/providers.md), [`MISTRAL_SOVEREIGN.md`](MISTRAL_SOVEREIGN.md)
- Model registry: [`integrations/model-registry.md`](integrations/model-registry.md)
- How the integration is tested: [`api/integration-tests.md`](api/integration-tests.md)
- The mesh: [`cross-ai-mesh.md`](cross-ai-mesh.md)
- Original fork decisions (historical): [`architecture.md`](architecture.md), [`inspection.md`](inspection.md)
- Crate READMEs: `codex-rs/codex-empirica-plugin/README.md`, `codex-rs/codex-empirica-translator/README.md`
