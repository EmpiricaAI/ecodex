# ecodex Inspection Notes — codex-rs Architecture

**Branch:** `inspect/codex-rs` · **Started:** 2026-05-01 · **Upstream HEAD at start:** `ff27d01676`

Investigation notes from T2 (codex-rs deep inspection) for the ecodex fork. These notes drive T3 (architecture decision) and are not intended for upstream — they live in our fork only.

## Top-level finding

**Codex already has a Claude-Code-compatible hook architecture, a plugin marketplace, and thread-scoped goals.** The integration calculus shifts from "fork and add empirica's machinery" to "publish empirica as a codex plugin." The fork's value becomes branding + bundled defaults, not technical necessity.

## Hook system (`codex-rs/hooks/`)

### Six events (matches Claude Code)

```
PreToolUse   PermissionRequest   PostToolUse
SessionStart UserPromptSubmit    Stop
```

### Protocol — CC-compatible

**Stdin (PreToolUse example):**
```json
{
  "session_id": "...", "turn_id": "...", "cwd": "...",
  "transcript_path": "...", "model": "...",
  "permission_mode": "default | acceptEdits | plan | dontAsk | bypassPermissions",
  "tool_name": "...", "tool_input": {...}, "tool_use_id": "..."
}
```

**Block paths (any of):**
- Exit code `2` + reason on stderr
- Stdout JSON `{"hookSpecificOutput": {"permissionDecision": "deny", "permissionDecisionReason": "..."}}`
- Deprecated stdout `{"decision": "block", "reason": "..."}` (kept for compat)

**Tool-name aliases for CC compat:**
- `apply_patch` accepts matchers `Write` and `Edit` (CC tool names → codex canonical)

### Engine internals

`hooks/src/engine/`: `command_runner.rs`, `dispatcher.rs`, `discovery.rs`, `output_parser.rs`, `schema_loader.rs` — full execution pipeline. JSON schemas for every event in `hooks/schema/generated/`.

### Where it integrates with the agent loop

`core/src/hook_runtime.rs` — the bridge from session/turn state to the hook system. Functions like `run_pending_session_start_hooks`, `run_pre_tool_use_hooks`. Constructs the request payload from `Session` + `TurnContext` and dispatches to `codex_hooks::Hooks`.

## Plugin system (`codex-rs/plugin/` + `codex-rs/core-plugins/`)

### Plugin manifest (JSON)

```json
{
  "name": "...", "version": "...", "description": "...",
  "skills": "./skills/...",
  "mcp_servers": "./mcp_servers.json",
  "apps": "./apps.json",
  "hooks": "./hooks.json | [paths] | inline-HooksFile | [inline-HooksFile]",
  "interface": {
    "displayName": "...", "shortDescription": "...",
    "developerName": "...", "category": "...",
    "capabilities": [...], "websiteUrl": "...",
    "brandColor": "...", "logo": "...", "screenshots": [...]
  }
}
```

**Hooks can be inline in the manifest** (matches CC's `settings.json` shape directly). A plugin can register: skills, MCP servers, app-connectors, hooks.

### Marketplace infrastructure

`core-plugins/src/`:
- `marketplace.rs`, `marketplace_add.rs`, `marketplace_remove.rs`, `marketplace_upgrade.rs`
- `remote.rs`, `remote_bundle.rs`, `startup_remote_sync.rs`
- `installed_marketplaces.rs`, `store.rs`, `toggles.rs`

Plugins distribute remotely. Install/upgrade/remove. Per-plugin enable/disable.

### Loading

`core-plugins/src/loader.rs` + `manager.rs`. Discovery via `find_plugin_manifest_path(plugin_root)`.

## Thread-scoped goals — complementary to empirica goals

`core/src/goals.rs` reveals codex has its own goal system at the **thread** level:
- `ThreadGoal { objective, status, token_budget }`
- Persisted in state DB
- Token-budget-aware (steers model when budget approaches limit)
- Emits `ThreadGoalUpdatedEvent`
- Continuation + budget-limit prompt templates inject into model context

**Relationship to empirica goals:**

| Layer | Empirica goal | Codex thread goal |
|---|---|---|
| Scope | Project-scoped, multi-session | Thread/conversation-scoped |
| Lifetime | Days/weeks | A single run |
| Currency | Epistemic vectors, calibration | Token budget |
| Decomposition | Subtasks | (none built in) |

These are **complementary, not duplicative.** Codex's thread-goal could be the per-session child of an empirica project-goal.

## Codex's other built-in systems (overlap analysis)

| Empirica concept | Codex equivalent | Relationship |
|---|---|---|
| Goals (project) | `core/src/goals.rs` (thread) | Complementary; pair them |
| Memory | `memories/read` + `memories/write` (Phase 1 rollout extraction → Phase 2 git-baseline consolidation) | Codex's is more sophisticated. Empirica should feed in or layer alongside. |
| Skills | `codex-skills` + plugin skills | Compatible; empirica skills register through plugin manifest |
| Hooks | `codex-hooks` (6 events) | Direct port of CC hook architecture; reuse |
| MCP | `codex-mcp` + `mcp-server` | Plugin can expose MCP servers |
| Sub-agents | `codex_delegate.rs` | Codex has built-in sub-agent delegation |
| Sandboxing | `sandboxing/`, `linux-sandbox`, `windows-sandbox-rs`, `sandbox_tags.rs` | Codex has full sandbox already |
| Telemetry | `codex-otel`, `codex-analytics` | OTel-based; empirica vectors could emit OTel |

## Implications for T3

### Integration patterns — re-ranked

| Approach | Updated assessment |
|---|---|
| **Plugin (NEW)** — empirica as a codex plugin manifest with inline hooks shelling to `empirica` CLI | **Most likely correct.** Reuses existing CC hook scripts with minimal porting. No fork required for the integration itself. |
| Hard-fork + Rust hooks | Now obsolete — codex already provides what we'd build. |
| Sidecar-MCP | Possible but redundant — hook-based path is simpler for the firewall. |
| PTY-wrap | Obsolete. |

### Empirica-language decision (was Options A/B/C/D/E)

The Rust-port pressure largely dissolves. Each hook is a subprocess invocation of the existing Python `empirica` CLI — same as Claude Code. Latency budget is dominated by per-hook subprocess fork+exec, not by empirica's internal logic.

**Likely landing:** Option A (subprocess shellout). C/D/E (PyO3, full port, AI-translated) become only relevant if hot-path measurements show the per-hook cost is intolerable.

### Fork value (revised)

Forking codex into `Nubaeon/ecodex` still earns its keep:
- Branding (`ecodex` binary)
- Bundled default config (empirica plugin pre-installed, target open-weights providers)
- Curated provider defaults targeting Llama/Qwen/DeepSeek via Ollama/vLLM
- Single-install UX vs "install codex + install empirica plugin + configure"

But the fork is a **distribution artifact**, not a deep technical fork.

## Outstanding (still in T2 noetic)

1. **Plugin install path** — where do users place plugins? Likely `~/.codex/plugins/<name>/` based on existing patterns; need to confirm via `core-plugins/src/loader.rs`.
2. **Hook firing frequency** — for latency-budget grounding (Subtask 4).
3. **Config + credential storage layout** — Subtask 3.
4. **Memory pipeline interop** — does codex's Phase 1/Phase 2 pipeline ingest external sources, or is it rollout-only?
5. **Branding hook** — does the binary name come from `arg0` crate? Forking the name needs a clean swap point.

## Decisions parked for T3 (David sign-off required)

- Empirica plugin distribution: marketplace-only / fork-bundled-only / both
- Goal model: link empirica project-goals ↔ codex thread-goals, or keep separate
- Memory pipeline: empirica feeds codex memories via Phase 1 input, or stays parallel
- ecodex fork's strategic identity now that the technical fork rationale has thinned
