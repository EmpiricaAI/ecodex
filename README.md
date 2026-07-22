<p align="center"><strong>ecodex</strong> — the epistemic agent environment by Empirica.</p>
<p align="center"><em>A coding agent that measures what it knows.</em></p>

---

ecodex is a fork of [openai/codex](https://github.com/openai/codex) bundled with the **Empirica** epistemic-discipline framework. Where vanilla codex runs an agent loop and lets the model speak with whatever confidence it generates, ecodex gates the loop on a measured cycle:

- Every **transaction** (a unit of measured agent work) opens with a **PREFLIGHT**, where the agent declares what it knows and doesn't via 13 calibration vectors.
- A **CHECK** gate decides whether the agent has enough context to act (transition into the **praxic** phase) or needs to keep investigating (stay in the **noetic** phase).
- **POSTFLIGHT** closes the loop and grounds the agent's self-assessment against deterministic services — test results, git metrics, artifact counts — so the divergence between belief and outcome is recorded.
- A **Sentinel** firewall sits between the model and the tools. Actions that would change state (Edit, Write, Bash on non-read commands) require an open transaction with a CHECK in `proceed`. Investigation tools (Read, Grep, Glob) flow freely — until a hypothesis-bearing prompt arms the **investigation-proportionality budget**, which caps survey-mode after a configurable threshold.

The result is an agent that can build a calibration history. Over time, the divergence between what the agent believed and what actually happened becomes a signal you can act on.

This is **not** a drop-in replacement for codex. It is opinionated: the discipline overhead is the point.

---

## Status

Alpha. **v0.2.5 shipped 2026-07-20** (first public release v0.1.0 was 2026-06-02) — see [the releases](https://github.com/EmpiricaAI/ecodex/releases). The release pipeline (gated build/test/clippy, GH release, crates.io publish for owned crates, Homebrew tap) lives in `scripts/release.sh`.

The repo's branches:

- `main` (default) — the canonical ecodex branch: all active work — the empirica plugin, the protocol translator, curated open-weights provider defaults + L3 model registry, the native ntfy mesh listener, the koru-spiral welcome screen, and discipline wiring. Upstream `openai/codex` is tracked via the `upstream` remote and synced in periodically (see `docs/ecodex/` + the upstream-sync issue template).

Public-facing pieces (CI workflows, contributor templates, expanded docs) land incrementally as we approach a broader launch. The CLI itself is daily-driven by the ecodex team.

## What ecodex adds on top of codex

Three layers (full architecture: [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md)):

| Layer | Owns | Examples |
|---|---|---|
| **L1 — codex foundation** | Upstream | Agent runtime, TUI, sandbox, app-server, MCP, plugin host, hook system |
| **L2 — empirica integration** | Us (`codex-rs/codex-empirica-plugin/`) | Hook routing to Sentinel, transaction lifecycle, calibration grounding |
| **L3 — specialised ecodex code** | Us | Wire-protocol translator, curated provider defaults, `ecodex` wrapper, install/uninstall, lint scope, marketing surface |

Concretely, what users notice that vanilla codex doesn't do:

- **Curated open-weights provider defaults**: out-of-the-box config for DeepSeek, Qwen3-Coder, Kimi K2.6, GLM, Ollama, LM Studio, llama.cpp, vLLM. Pick a provider in `/model` and routing swaps mid-session — no restart.
- **Curated model registry + discovery** (`ecodex models`): a pre-filled, capability-tagged seed of coding/agentic models (context, tool-use, reasoning, route, jurisdiction) so the picker is short and relevant — not a 300-entry dump. `ecodex models refresh` probes your configured providers' `/v1/models` (incl. local Ollama/llama.cpp/vLLM/LM Studio) and adds only recognized coding-family models, latest-per-line. Per-model `calibration_tier` ships `unmeasured` and is filled from your own grounded usage. See [`docs/ecodex/integrations/model-registry.md`](docs/ecodex/integrations/model-registry.md).
- **Wire-protocol translator** (`codex-rs/codex-empirica-translator/`): a small `tiny_http` bridge that lets codex's Responses-format API talk to providers that only speak Chat Completions or Anthropic Messages. CIF (Canonical Intermediate Format) validated at N=3 adapters.
- **Pinned epistemic skills**: framework-level skills (`epistemic-transaction`, `empirica-constitution`, `epistemic-persistence-protocol`) survive `/compact` so the agent retains its own discipline guidance across context boundaries.
- **Subagent seeding**: empirica's specialised subagents (security, ux, performance, outreach-scout, …) are bundled and dispatched through the standard codex Agent tool.
- **Statusline + welcome screen**: the koru-spiral animation matches Empirica's marketing identity; statusline shows live epistemic state (phase indicator, intuition-vs-search badge).
- **Cross-AI mesh participation** (`[mcp_servers.cortex]` + new `monitor` tool): ecodex agents call the same `cortex_inbox_poll` / `cortex_propose` tools as Claude Code sessions, and the `monitor` primitive provides sub-second wake on background subprocess events (held ntfy connection, queue listener, etc.). This makes ecodex a **first-class peer** in the Empirica AI mesh — a non-Claude model running in ecodex can receive a proposal from a CC session, react to it, and ship a response within seconds, with no human in the loop. See [`docs/ecodex/cross-ai-mesh.md`](docs/ecodex/cross-ai-mesh.md).
- **Extended hook event surface** (7 new events beyond stock codex's 6): `TaskCompleted`, `PostToolUseFailure`, `PreCompact`, `PostCompact`, `SessionEnd`, `SubagentStart`, `SubagentStop` — full lifecycle coverage so plugin handlers can enforce POSTFLIGHT, capture failures as dead-ends, snapshot state across compaction, and track parent→child subagent relationships. Plugin authors declare handlers in `hooks.json`; ecodex fires them at the right lifecycle points. See [`docs/ecodex/hook-events-roadmap.md`](docs/ecodex/hook-events-roadmap.md).

## Install

| Channel | Command | Compiles? |
|---|---|---|
| **Install script** (Mac/Linux) | `curl -fsSL https://raw.githubusercontent.com/EmpiricaAI/ecodex/main/scripts/install.sh \| bash` | No — prebuilt |
| **Homebrew** (Mac/Linux) | `brew install EmpiricaAI/tap/ecodex` | No — prebuilt |
| **Direct binary** | Download `ecodex-<target>.tar.gz` from [Releases](https://github.com/EmpiricaAI/ecodex/releases/latest) | No — prebuilt |
| **Cargo** (Rust devs) | `cargo install --git https://github.com/EmpiricaAI/ecodex codex-cli` | Yes (source) |
| **Build from source** | `git clone … && cd ecodex && ./ecodex/scripts/install.sh` | Yes (source) |

The first three paths download prebuilt, stripped binaries for macOS (arm64/x64) and Linux (arm64/x64) — **no Rust toolchain, no compile**. Non-devs should use the install script or Homebrew. The cargo + source-build paths compile the workspace (10–25 min) and are for developers.

The empirica CLI must also be on `PATH` — install from [`EmpiricaAI/empirica`](https://github.com/EmpiricaAI/empirica). Without it the empirica plugin shells fail-quiet and discipline goes dark.

See [`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md) for `--user` vs `--system` install, prerequisites, provider configuration, and troubleshooting.

## Run

```shell
ecodex
```

The first run uses the curated `config.toml` defaults. Add your API keys (per-provider env vars are documented in the seeded config), pick a model with `/model`, and start a session. Hot-swap to a different provider mid-session via `/model` — no restart needed.

## Glossary

| Term | What it means |
|---|---|
| **transaction** | One measured chunk of agent work, framed by PREFLIGHT → (CHECK → praxic) → POSTFLIGHT. Linked to one or more goals. |
| **PREFLIGHT** | The transaction-opening assessment: declares 13 calibration vectors (`know`, `uncertainty`, `do`, `clarity`, …) representing the agent's belief about its current epistemic state. |
| **CHECK** | The gate that decides whether the agent transitions from the noetic (investigation) phase into the praxic (action) phase. Returns `proceed` or `investigate`. |
| **POSTFLIGHT** | The transaction-closing reflection: re-declares vectors, gets compared against deterministic service observations (lint, tests, git metrics) to compute calibration deltas. |
| **noetic** | Investigation phase. Read/Grep/Glob always allowed. The agent is building understanding. |
| **praxic** | Action phase. Edit/Write/Bash require an open transaction with CHECK=proceed. The agent is committing to a path. |
| **Sentinel** | The firewall component that gates praxic tool calls on transaction state. Lives in `sentinel-gate.py`, runs as a codex `PreToolUse` hook subprocess. |
| **calibration** | The divergence between an agent's stated vector and the deterministic-service observation. Brier-scored. The signal that improves over time. |
| **artifact** | A logged epistemic unit: finding (verified discovery), unknown (open question), assumption (unverified belief), decision (chosen path), dead-end (failed approach), mistake (recognized error), goal (target). |
| **investigation-proportionality budget** | A per-session counter that caps Read/Grep/Glob calls after a hypothesis-bearing prompt fires the budget. Prevents investigation-as-procrastination. |
| **monitor** | ecodex tool that arms a watched subprocess + regex pattern. On each matching output line, a `<task-notification>` message is injected into the agent's pending input — sub-second wake on background events. Parity with Claude Code's `Monitor` tool. |
| **cross-AI mesh** | Empirica's AI-to-AI communication layer: peer AIs send each other `cortex_propose` events; recipients react via inbox polling or held-connection wake. ecodex participates as a first-class peer via the cortex MCP server + the `monitor` primitive. |

For the full vocabulary + how the pieces compose, see [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md).

## Documentation

- [`CONTRIBUTING.md`](CONTRIBUTING.md) — three-layer contribution model, dev workflow, conventions
- [`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md) — install modes, providers, troubleshooting
- [`docs/ecodex/system-overview.md`](docs/ecodex/system-overview.md) — three-layer architecture, runtime composition, file layout
- [`docs/ecodex/architecture.md`](docs/ecodex/architecture.md) — T3 decision record (distribution model, fork posture, integration strategy)
- [`docs/ecodex/inspection.md`](docs/ecodex/inspection.md) — T2 inspection of codex-rs (hook system, plugin marketplace, thread-scoped goals)
- [`docs/ecodex/api/`](docs/ecodex/api/) — plugin API contracts (`hooks.md`, `skills.md`, `mcp.md`, `plugin-statusline.md`, `plugin-writable-roots.md`, `integration-tests.md`)
- [`codex-rs/codex-empirica-plugin/README.md`](codex-rs/codex-empirica-plugin/README.md) — plugin architecture, hook-by-hook status
- [`codex-rs/codex-empirica-translator/README.md`](codex-rs/codex-empirica-translator/README.md) — translator design, CIF, adapter map

## Relationship to upstream codex

ecodex is a **product fork**, not a derivative. Upstream improvements flow into us via `main` rebase; our hardening fixes go back via PRs against `openai/codex`. The agent runtime, sandbox, RPC protocol, plugin host, and hook system all come from upstream — unchanged.

What ecodex adds is *additive*: new crates (`codex-empirica-plugin`, `codex-empirica-translator`), new manifest fields (`writableRoots`, `statusline`), new provider entries, an install script that handles bundled-plugin layout. We do **not** rename, reorganize, or break upstream APIs. We add layers and curated defaults; we do not divert.

The only special-case patch we maintain inside upstream code is the `ECODEX_AUTO_TRUSTED_PLUGIN_IDS` allowlist in `codex-rs/hooks/src/engine/discovery.rs` — first-party plugin trust on first install, in lieu of upstream's user-trust review flow. When empirica becomes a marketplace plugin, this comes off and we use the upstream-intended flow.

## License

Apache-2.0 (inherited from `openai/codex`). See [`LICENSE`](LICENSE).

ecodex is built by [Empirica](https://github.com/EmpiricaAI/empirica). Upstream codex is built by [OpenAI](https://github.com/openai/codex).
