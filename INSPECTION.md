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

## Config + credential storage (Subtask 3)

**Codex home:** `$CODEX_HOME` env var, defaults to `~/.codex/`. Resolver in `utils/home-dir/src/lib.rs::find_codex_home()` — must exist and be a directory if env var is set.

**Layout under `~/.codex/`:**
- `config.toml` — main config (TOML; rich schema in `core/config.schema.json`)
- `memories/` — memory artifacts (git-baseline initialized; `~/.codex/memories/.git`)
- `plugins/cache/<plugin_id>/<version>/` — installed plugin payloads (atomic version replace)
- `plugins/data/` — plugin runtime data
- `.agents/plugins/marketplace.json` — installed marketplace state

**Credential storage:**
- `keyring-store/` — OS keyring integration (macOS Keychain, Linux Secret Service, Windows Credential Vault)
- `login/` — auth flows: device-code, PKCE, web server callback
- `device-key/` — per-device key management
- `secrets/` — generic secrets handling

**Where empirica plugin lands:** `~/.codex/plugins/cache/empirica/<version>/manifest.json` (+ optional `hooks.json`, `skills/`, etc.). Install via marketplace OR direct `--bundle` path.

**Config-side empirica integration:** users add the plugin to their `config.toml` (codex has `plugin_edit.rs` for this). The plugin's hooks then activate on every codex session.

## Hook firing frequency estimate (Subtask 4)

Reasoning from architecture (no runtime measurement yet — that's a post-T3 task):

| Event | Per-session frequency |
|---|---|
| `SessionStart` | 1 |
| `UserPromptSubmit` | ~5–50 (one per user turn) |
| `PreToolUse` | ~10–100 (one per tool call; coding sessions vary widely) |
| `PostToolUse` | ~10–100 (paired with PreToolUse) |
| `PermissionRequest` | ~0–10 (only on escalation) |
| `Stop` | 1 |
| **Total per session** | **~30–270 hook fires** |

**Latency budget for subprocess shellout (Option A) per hook fire:**
- Python interpreter startup: 50–100ms
- empirica CLI logic (sentinel-gate.py: state DB read + maybe Qdrant): 50–200ms
- **Total: 100–300ms per fire**

**Cumulative session overhead:** ~3s–80s spread over a session lasting minutes → 1–15% wall-clock overhead. Tolerable.

**Per-tool-call user-visible cost:** PreToolUse + PostToolUse = 200–600ms added to every tool call. Perceptible but not painful.

**Verdict:** **Subprocess shellout (Option A) is viable for v1.** Options B/C/D/E (sidecar IPC, PyO3, full Rust port, AI-translated Rust) are not necessary for the integration. They become relevant only if real-world measurement shows the per-tool-call cost is unacceptable for power users.

## Outstanding (parked for post-T3)

1. **Memory pipeline interop** — does codex's Phase 1/Phase 2 pipeline ingest external sources, or is it rollout-only? Affects whether empirica findings flow into codex memories or stay parallel.
2. **Branding hook** — does the binary name come from `arg0` crate? Forking the name to `ecodex` cleanly needs a swap point.
3. **Goal-pairing protocol** — if we link empirica project-goals ↔ codex thread-goals, what's the wire format? Token-budget pass-through?
4. **Runtime hook latency** — actual measurement once a prototype empirica plugin is wired up.

## Decisions parked for T3 (David sign-off required)

- Empirica plugin distribution: marketplace-only / fork-bundled-only / both
- Goal model: link empirica project-goals ↔ codex thread-goals, or keep separate
- Memory pipeline: empirica feeds codex memories via Phase 1 input, or stays parallel
- ecodex fork's strategic identity now that the technical fork rationale has thinned

---

# T3 Architecture Decision (2026-05-01)

## Decisions

### D1 — Distribution: dual product

Ship **both**:
- **`empirica` plugin for codex** — published to codex's plugin marketplace. Reaches anyone already on codex (developers + future non-coder market).
- **`ecodex` fork** — branded one-click install with empirica plugin pre-bundled, curated open-weights provider defaults (Llama/Qwen/DeepSeek via Ollama/vLLM), Empirica-aware UX. Sold to existing Empirica clients.

Same plugin codebase ships both. The fork is distribution + branding, not a technical fork.

### D2 — Empirica-language strategy: subprocess shellout (Option A)

Empirica's Python CLI stays canonical. The codex plugin's hooks shell out to it (matches the existing Claude Code integration pattern).

Latency analysis (from T2 Subtask 4): 30–270 hook fires/session × 100–300ms/fire = 1–15% session overhead. Tolerable for v1.

Options B/C/D/E (sidecar IPC, PyO3, full Rust port, AI-translated parity Rust) are deferred. They become relevant only if real-world telemetry shows per-tool-call latency unacceptable.

### D3 — Goal pairing: integrate, conditional on prototype validation passing in v1 build

Empirica project-goals pair with codex thread-goals. ThreadGoal struct (verified at `protocol/src/protocol.rs:3608`) has no metadata field, so pairing uses:
- **Embed empirica goal_id in codex thread-goal `objective` text** via stable tag prefix `[empirica:<goal_id>]`. Lossy but no upstream protocol change required.
- **Stop hook** captures thread completion and grounds empirica goal completion against codex thread outcome.
- **Token budget** flows from codex thread-goal back to empirica via PostToolUse hook payload (codex tracks `tokens_used`).

Validate the round-trip during v1 plugin build. If protocol changes prove necessary, escalate.

### D4 — Memory pipeline: parallel coexistence

Codex's Phase 1/2 memory pipeline operates on rollouts, not external sources (verified — orchestration not in `core/src/memories/` as the README states; only `memory_usage.rs` exists in core; memories crates do read/write but are rollout-scoped). Empirica artifacts (findings, decisions, etc.) live in their own store under `~/.codex/empirica/` (or wherever empirica's existing storage lands per its CLI conventions).

If cross-pollination becomes valuable later, build a thin consolidator that reads empirica artifacts and writes codex-memory-format markdown. Out of scope for v1.

## Working assumptions to validate during v1

- Codex hook protocol is stable across recent codex versions (recorded as assumption with confidence 0.7).
- Plugin marketplace install path / format is stable enough to publish against (confidence 0.6).
- Subprocess shellout latency is tolerable for typical sessions (confidence 0.7 — needs runtime confirmation).

## Parked for future sessions (planned empirica goals)

| Goal | Spec | Status |
|---|---|---|
| Web/non-coder product on codex-app-server v2 RPC | [docs/ecodex/web-product-vision.md](docs/ecodex/web-product-vision.md) | planned |
| Asynchronous-ground-truth calibration research | [docs/ecodex/async-calibration-research.md](docs/ecodex/async-calibration-research.md) | planned |

These deferred goals share infrastructure with ecodex (same engine, same plugin system) but are scoped to different audiences and require separate architecture work. Activating them is a future-session decision.

## Next session(s) — what comes after T3

1. **Build v1 of `empirica` plugin for codex** — manifest + hook scripts (porting CC's `sentinel-gate.py` etc.) + skill registration. New goal.
2. **Build ecodex distribution layer** — branding swap, bundled config, curated provider defaults. New goal.
3. **Validate D3 + assumptions during v1 build** — runtime hook latency measurement, goal-pairing round-trip, plugin marketplace publish flow.

These were not part of T3's scope (T3 was decision, not execution). They open as fresh transactions in the next session.
